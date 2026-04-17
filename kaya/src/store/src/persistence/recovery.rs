//! Crash recovery: load latest snapshot, then replay WAL tail.
//!
//! Execution order per shard:
//! 1. Locate the most recent valid snapshot and apply it.
//! 2. Enumerate WAL segments, skipping records with `logical_ts` <= the
//!    snapshot's `last_applied_index`, and apply the remainder in order.
//! 3. On the final segment, a truncated tail record is tolerated (crash
//!    during write). Any CRC error before EOF aborts recovery for that
//!    segment but other shards are unaffected.

use std::path::PathBuf;
use std::time::Instant;

use bytes::Bytes;
use once_cell::sync::Lazy;
use prometheus::{Histogram, HistogramOpts, IntCounter};
use tracing::{debug, instrument, warn};

use super::config::PersistenceConfig;
use super::snapshot::{apply_snapshot, latest_snapshot};
use super::wal::{list_segments, WalOp, WalReader, WalRecord};
use super::{PersistenceError, PersistenceResult};
use crate::entry::{Entry, EntryMetadata};
use crate::shard::Shard;

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

static RECOVERY_DURATION: Lazy<Histogram> = Lazy::new(|| {
    Histogram::with_opts(
        HistogramOpts::new(
            "kaya_recovery_duration_ms",
            "Duration of per-shard crash recovery in milliseconds.",
        )
        .buckets(vec![10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 30000.0]),
    )
    .expect("build histogram")
});

static RECOVERY_RECORDS: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "kaya_recovery_records_replayed_total",
        "Total WAL records replayed during KAYA recovery.",
    )
    .expect("build counter")
});

/// Register recovery metrics into `reg`.
pub fn register_metrics(reg: &prometheus::Registry) -> prometheus::Result<()> {
    reg.register(Box::new(RECOVERY_DURATION.clone()))?;
    reg.register(Box::new(RECOVERY_RECORDS.clone()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Outcome of a per-shard recovery pass.
#[derive(Debug, Clone, Default)]
pub struct RecoveryReport {
    /// `last_applied_index` from the snapshot that was loaded (if any).
    pub snapshot_loaded: Option<u64>,
    /// Number of WAL records successfully replayed.
    pub wal_records_replayed: u64,
    /// Segments that had to be skipped due to corruption mid-file (NOT the
    /// normal tail-truncation case).
    pub corrupt_segments: Vec<PathBuf>,
    /// Wall-clock elapsed milliseconds.
    pub elapsed_ms: u64,
    /// Highest logical_ts observed during replay.
    pub last_logical_ts: u64,
}

// ---------------------------------------------------------------------------
// recover_shard
// ---------------------------------------------------------------------------

/// Recover a single shard in place. Returns a [`RecoveryReport`] detailing
/// what happened.
#[instrument(skip(shard, config), fields(shard_id = shard.id))]
pub async fn recover_shard(
    shard: &Shard,
    config: &PersistenceConfig,
) -> PersistenceResult<RecoveryReport> {
    let start = Instant::now();
    let mut report = RecoveryReport::default();
    let shard_id = shard.id as u32;

    // --- Step 1: snapshot ------------------------------------------------
    let snaps_dir = config.snapshots_dir();
    let mut cutoff: u64 = 0;
    if snaps_dir.exists() {
        if let Some((seq, path)) = latest_snapshot(&snaps_dir, shard_id)? {
            let max_dc = config.max_decompressed_size;
            match apply_snapshot(shard, &path, max_dc).await {
                Ok(last) => {
                    report.snapshot_loaded = Some(last);
                    cutoff = last;
                    debug!(
                        shard_id,
                        seq,
                        last_applied_index = last,
                        "loaded snapshot"
                    );
                }
                Err(e) => {
                    warn!(
                        shard_id,
                        path = %path.display(),
                        error = %e,
                        "failed to apply snapshot; trying older"
                    );
                    // Best effort: try any older snapshot.
                    let mut all =
                        super::snapshot::list_snapshots(&snaps_dir, shard_id)?;
                    all.sort_by_key(|(s, _)| *s);
                    all.pop(); // already tried the latest
                    while let Some((s, p)) = all.pop() {
                        match apply_snapshot(shard, &p, max_dc).await {
                            Ok(last) => {
                                report.snapshot_loaded = Some(last);
                                cutoff = last;
                                debug!(shard_id, seq = s, "fell back to older snapshot");
                                break;
                            }
                            Err(e2) => {
                                warn!(
                                    shard_id,
                                    path = %p.display(),
                                    error = %e2,
                                    "snapshot also invalid"
                                );
                                report.corrupt_segments.push(p);
                            }
                        }
                    }
                }
            }
        }
    }

    // --- Step 2: WAL ----------------------------------------------------
    let wal_dir = config.wal_dir();
    let segments = list_segments(&wal_dir, shard_id)?;
    let num_segments = segments.len();
    for (idx, (_seg_num, path)) in segments.iter().enumerate() {
        let is_last = idx + 1 == num_segments;
        match WalReader::open(path).await {
            Ok(mut reader) => loop {
                match reader.next().await {
                    Ok(Some(rec)) => {
                        if rec.logical_ts <= cutoff {
                            continue;
                        }
                        apply_record(shard, &rec);
                        report.wal_records_replayed += 1;
                        report.last_logical_ts = report.last_logical_ts.max(rec.logical_ts);
                        RECOVERY_RECORDS.inc();
                    }
                    Ok(None) => break,
                    Err(PersistenceError::CorruptSegment(msg)) => {
                        if is_last {
                            warn!(
                                shard_id,
                                path = %path.display(),
                                msg = %msg,
                                "tolerating corruption in final WAL segment"
                            );
                        } else {
                            warn!(
                                shard_id,
                                path = %path.display(),
                                msg = %msg,
                                "WAL segment corruption detected"
                            );
                            report.corrupt_segments.push(path.clone());
                        }
                        break;
                    }
                    Err(e) => {
                        warn!(shard_id, error = %e, "WAL read error");
                        report.corrupt_segments.push(path.clone());
                        break;
                    }
                }
            },
            Err(e) => {
                warn!(shard_id, path = %path.display(), error = %e, "cannot open WAL segment");
                report.corrupt_segments.push(path.clone());
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    RECOVERY_DURATION.observe(elapsed);
    report.elapsed_ms = elapsed as u64;
    debug!(
        shard_id,
        records = report.wal_records_replayed,
        elapsed_ms = report.elapsed_ms,
        "recovery complete"
    );
    Ok(report)
}

// ---------------------------------------------------------------------------
// Apply a single WAL record
// ---------------------------------------------------------------------------

fn apply_record(shard: &Shard, rec: &WalRecord) {
    let now = Instant::now();
    match rec.op {
        WalOp::Set => {
            let expires_at = if rec.extra == 0 {
                None
            } else {
                Some(now + std::time::Duration::from_secs(rec.extra))
            };
            let entry = Entry {
                value: rec.value.clone(),
                metadata: EntryMetadata {
                    created_at: now,
                    last_accessed: now,
                    expires_at,
                    access_count: 0,
                    size_bytes: rec.value.len(),
                },
            };
            shard.insert(&rec.key, entry);
        }
        WalOp::Del => {
            shard.remove(&rec.key);
        }
        WalOp::Expire => {
            shard.set_expiry(&rec.key, std::time::Duration::from_secs(rec.extra));
        }
        WalOp::SAdd | WalOp::SetExAdd => {
            let member: &[u8] = &rec.value;
            let _ = shard.sadd(&rec.key, &[member]);
        }
        WalOp::SRem => {
            let member: &[u8] = &rec.value;
            let _ = shard.srem(&rec.key, &[member]);
        }
        WalOp::ZAdd => {
            let score = f64::from_bits(rec.extra);
            let member: &[u8] = &rec.value;
            let _ = shard.zadd(&rec.key, &[(score, member)]);
        }
        WalOp::ZRem => {
            let member: &[u8] = &rec.value;
            let _ = shard.zrem(&rec.key, &[member]);
        }
        WalOp::Incr => {
            // Replay stores the new absolute value as the key's value.
            // We simply write it back as the record's value field.
            let new_entry = Entry {
                value: rec.value.clone(),
                metadata: EntryMetadata {
                    created_at: now,
                    last_accessed: now,
                    expires_at: None,
                    access_count: 0,
                    size_bytes: rec.value.len(),
                },
            };
            shard.insert(&rec.key, new_entry);
        }
        WalOp::Flush => {
            shard.flush();
        }
    }
    let _ = Bytes::new(); // silence unused warning in case
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryMetadata;
    use crate::persistence::config::CompressionAlgo;
    use crate::persistence::snapshot::{snapshot_path, take_snapshot};
    use crate::persistence::wal::{segment_path, WalWriter};
    use crate::shard::Shard;
    use crate::EvictionPolicyKind;
    use std::time::Instant;
    use tempfile::TempDir;

    fn make_shard(id: usize) -> Shard {
        Shard::new(id, EvictionPolicyKind::None)
    }

    fn insert_str(shard: &Shard, key: &[u8], val: &[u8]) {
        shard.insert(
            key,
            crate::entry::Entry {
                value: bytes::Bytes::copy_from_slice(val),
                metadata: EntryMetadata {
                    created_at: Instant::now(),
                    last_accessed: Instant::now(),
                    expires_at: None,
                    access_count: 0,
                    size_bytes: val.len(),
                },
            },
        );
    }

    fn make_config(dir: &std::path::Path) -> PersistenceConfig {
        PersistenceConfig {
            enabled: true,
            data_dir: dir.to_path_buf(),
            fsync_policy: crate::persistence::FsyncPolicy::No,
            segment_size_bytes: 64 * 1024 * 1024,
            snapshot_interval_secs: 0,
            snapshot_retention: 7,
            compression: CompressionAlgo::None,
            zstd_level: 1,
            max_decompressed_size: 256 * 1024 * 1024,
        }
    }

    // --- WAL replay from scratch (no snapshot) ------------------------------

    #[tokio::test]
    async fn recovery_wal_only_replay() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        tokio::fs::create_dir_all(config.wal_dir()).await.unwrap();

        // Write two SET records and one DEL into the WAL for shard 0.
        let writer = WalWriter::open(0, &config).await.unwrap();
        let mut rec_set1 = crate::persistence::WalRecord::new(
            crate::persistence::WalOp::Set,
            0,
            1,
            bytes::Bytes::from_static(b"wal-key1"),
            bytes::Bytes::from_static(b"wal-val1"),
        );
        let mut rec_set2 = crate::persistence::WalRecord::new(
            crate::persistence::WalOp::Set,
            0,
            2,
            bytes::Bytes::from_static(b"wal-key2"),
            bytes::Bytes::from_static(b"wal-val2"),
        );
        let mut rec_del = crate::persistence::WalRecord::new(
            crate::persistence::WalOp::Del,
            0,
            3,
            bytes::Bytes::from_static(b"wal-key1"),
            bytes::Bytes::new(),
        );
        rec_set1.logical_ts = 1;
        rec_set2.logical_ts = 2;
        rec_del.logical_ts = 3;
        writer.append(rec_set1).await.unwrap();
        writer.append(rec_set2).await.unwrap();
        writer.append(rec_del).await.unwrap();
        writer.sync_now().await.unwrap();
        drop(writer);

        // Recover.
        let shard = make_shard(0);
        let report = recover_shard(&shard, &config).await.unwrap();

        assert_eq!(report.wal_records_replayed, 3);
        assert!(report.snapshot_loaded.is_none());
        // key1 deleted, key2 present
        assert!(!shard.contains(b"wal-key1"));
        assert!(shard.contains(b"wal-key2"));
    }

    // --- snapshot + WAL replay (combined recovery) --------------------------

    #[tokio::test]
    async fn recovery_snapshot_then_wal() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        tokio::fs::create_dir_all(config.wal_dir()).await.unwrap();
        tokio::fs::create_dir_all(config.snapshots_dir()).await.unwrap();

        // Write a snapshot containing "snap-key".
        let shard_src = make_shard(0);
        insert_str(&shard_src, b"snap-key", b"snap-val");
        let snap_path = snapshot_path(&config.snapshots_dir(), 0, 0);
        take_snapshot(&shard_src, &snap_path, CompressionAlgo::None, 0, 1)
            .await
            .unwrap();

        // Write a WAL SET for "wal-key" with logical_ts=2 (after snapshot boundary 1).
        let writer = WalWriter::open(0, &config).await.unwrap();
        let mut rec = crate::persistence::WalRecord::new(
            crate::persistence::WalOp::Set,
            0,
            2,
            bytes::Bytes::from_static(b"wal-key"),
            bytes::Bytes::from_static(b"wal-val"),
        );
        rec.logical_ts = 2;
        writer.append(rec).await.unwrap();
        writer.sync_now().await.unwrap();
        drop(writer);

        // Recover into a fresh shard.
        let shard = make_shard(0);
        let report = recover_shard(&shard, &config).await.unwrap();

        assert!(report.snapshot_loaded.is_some(), "snapshot must be loaded");
        // The WAL record has ts=2 > cutoff=1, so it should be replayed.
        assert_eq!(report.wal_records_replayed, 1);
        assert!(shard.contains(b"snap-key"), "snapshot content restored");
        assert!(shard.contains(b"wal-key"), "WAL content replayed");
    }

    // --- mid-WAL crash: truncated tail record is tolerated ------------------

    #[tokio::test]
    async fn recovery_tolerates_truncated_tail_record() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        tokio::fs::create_dir_all(config.wal_dir()).await.unwrap();

        // Write a valid record then append garbage to simulate a crash.
        let writer = WalWriter::open(0, &config).await.unwrap();
        let mut rec = crate::persistence::WalRecord::new(
            crate::persistence::WalOp::Set,
            0,
            1,
            bytes::Bytes::from_static(b"truncated-key"),
            bytes::Bytes::from_static(b"truncated-val"),
        );
        rec.logical_ts = 1;
        writer.append(rec).await.unwrap();
        writer.sync_now().await.unwrap();
        drop(writer);

        // Append a few raw garbage bytes after the valid record.
        let seg = segment_path(&config.wal_dir(), 0, 0);
        let mut raw = tokio::fs::read(&seg).await.unwrap();
        raw.extend_from_slice(b"\xDE\xAD\xBE\xEF\x00\x01"); // partial/corrupt tail
        tokio::fs::write(&seg, &raw).await.unwrap();

        let shard = make_shard(0);
        // Should NOT return an Err; truncated tail is tolerated.
        let report = recover_shard(&shard, &config).await.unwrap();
        // The valid record must have been replayed.
        assert!(shard.contains(b"truncated-key"));
        assert_eq!(report.wal_records_replayed, 1);
    }
}
