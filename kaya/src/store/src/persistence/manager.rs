//! PersistenceManager: orchestrates WAL + snapshots across all shards.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, instrument, warn};

use super::config::PersistenceConfig;
use super::recovery::{recover_shard, RecoveryReport};
use super::snapshot::{
    latest_snapshot, list_snapshots, prune_snapshots, snapshot_path, take_snapshot,
};
use super::wal::{list_segments, WalRecord, WalWriter};
use super::{PersistenceError, PersistenceResult};
use crate::Store;

/// Orchestrates WAL appends, snapshots, and recovery for every shard in a
/// KAYA [`Store`]. One [`PersistenceManager`] per store instance.
pub struct PersistenceManager {
    config: PersistenceConfig,
    writers: Vec<WalWriter>,
    /// Monotonic logical timestamp per shard.
    logical_ts: Vec<AtomicU64>,
    /// Next snapshot sequence number per shard.
    snap_seq: Vec<AtomicU64>,
    /// Snapshot last-applied-index watermark per shard.
    last_applied: Vec<AtomicU64>,
}

impl PersistenceManager {
    /// Construct a new manager bound to `num_shards` shards. Creates (or
    /// opens) all WAL segments.
    #[instrument(skip(config))]
    pub async fn new(
        config: PersistenceConfig,
        num_shards: usize,
    ) -> PersistenceResult<Arc<Self>> {
        if !config.enabled {
            return Err(PersistenceError::Disabled);
        }
        tokio::fs::create_dir_all(config.wal_dir()).await?;
        tokio::fs::create_dir_all(config.snapshots_dir()).await?;

        let mut writers = Vec::with_capacity(num_shards);
        let mut logical_ts = Vec::with_capacity(num_shards);
        let mut snap_seq = Vec::with_capacity(num_shards);
        let mut last_applied = Vec::with_capacity(num_shards);

        for shard_id in 0..num_shards {
            let w = WalWriter::open(shard_id as u32, &config).await?;
            writers.push(w);
            logical_ts.push(AtomicU64::new(0));
            // Probe latest snapshot sequence to continue numbering.
            let seq0 = latest_snapshot(&config.snapshots_dir(), shard_id as u32)?
                .map(|(s, _)| s + 1)
                .unwrap_or(0);
            snap_seq.push(AtomicU64::new(seq0));
            last_applied.push(AtomicU64::new(0));
        }

        Ok(Arc::new(Self {
            config,
            writers,
            logical_ts,
            snap_seq,
            last_applied,
        }))
    }

    /// Return the effective configuration.
    pub fn config(&self) -> &PersistenceConfig {
        &self.config
    }

    /// Assign the next logical timestamp for `shard_id` and append the
    /// record to its WAL.
    #[instrument(skip(self, record), fields(shard_id = shard_id))]
    pub async fn log(&self, shard_id: usize, mut record: WalRecord) -> PersistenceResult<u64> {
        let writer = self
            .writers
            .get(shard_id)
            .ok_or_else(|| PersistenceError::Internal(format!("no writer for shard {shard_id}")))?;
        let ts = self.logical_ts[shard_id].fetch_add(1, Ordering::AcqRel) + 1;
        record.logical_ts = ts;
        record.shard_id = shard_id as u32;
        writer.append(record).await?;
        Ok(ts)
    }

    /// Force fsync on every shard's WAL.
    #[instrument(skip(self))]
    pub async fn sync_all(&self) -> PersistenceResult<()> {
        for w in &self.writers {
            w.sync_now().await?;
        }
        Ok(())
    }

    /// Take a snapshot of every shard in `store`, advancing the
    /// `last_applied_index` watermark and pruning older snapshots.
    #[instrument(skip(self, store))]
    pub async fn checkpoint_all(&self, store: &Store) -> PersistenceResult<Vec<PathBuf>> {
        let dir = self.config.snapshots_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let mut out = Vec::with_capacity(self.writers.len());
        for shard_id in 0..store.num_shards() {
            let shard = store.shard_at(shard_id);
            let seq = self.snap_seq[shard_id].fetch_add(1, Ordering::AcqRel);
            let target = snapshot_path(&dir, shard_id as u32, seq);
            let last_idx = self.logical_ts[shard_id].load(Ordering::Acquire);
            match take_snapshot(
                shard,
                &target,
                self.config.compression,
                self.config.zstd_level,
                last_idx,
            )
            .await
            {
                Ok(path) => {
                    self.last_applied[shard_id].store(last_idx, Ordering::Release);
                    out.push(path);
                    // Retention
                    let _ = prune_snapshots(&self.config, shard_id as u32, self.config.snapshot_retention)
                        .await;
                    // Truncate WAL segments older than the one containing
                    // last_applied (keep the current live segment).
                    if let Err(e) = self
                        .truncate_old_wal_segments(shard_id as u32)
                        .await
                    {
                        warn!(shard_id, error = %e, "failed to truncate old WAL");
                    }
                }
                Err(e) => {
                    warn!(shard_id, error = %e, "snapshot failed");
                }
            }
        }
        Ok(out)
    }

    /// Replay persistence state into `store` at startup. Returns one
    /// [`RecoveryReport`] per shard.
    #[instrument(skip(self, store))]
    pub async fn recover(&self, store: &Store) -> PersistenceResult<Vec<RecoveryReport>> {
        let mut reports = Vec::with_capacity(self.writers.len());
        for shard_id in 0..store.num_shards() {
            let shard = store.shard_at(shard_id);
            let report = recover_shard(shard, &self.config).await?;
            // Re-align logical_ts so future appends continue monotonically.
            let ts = report
                .last_logical_ts
                .max(report.snapshot_loaded.unwrap_or(0));
            self.logical_ts[shard_id].store(ts, Ordering::Release);
            if let Some(last) = report.snapshot_loaded {
                self.last_applied[shard_id].store(last, Ordering::Release);
            }
            reports.push(report);
        }
        Ok(reports)
    }

    /// Spawn a background task that takes a full-store checkpoint every
    /// `snapshot_interval_secs`.
    pub fn spawn_periodic_snapshots(self: &Arc<Self>, store: Arc<Store>) {
        let interval_secs = self.config.snapshot_interval_secs;
        if interval_secs == 0 {
            return;
        }
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            // Skip the immediate tick.
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(e) = this.checkpoint_all(&store).await {
                    warn!(error = %e, "periodic checkpoint failed");
                } else {
                    debug!("periodic checkpoint ok");
                }
            }
        });
    }

    /// Delete WAL segments entirely below the current live segment for
    /// `shard_id`. Called after a successful checkpoint.
    async fn truncate_old_wal_segments(&self, shard_id: u32) -> PersistenceResult<()> {
        let segments = list_segments(&self.config.wal_dir(), shard_id)?;
        if segments.len() <= 1 {
            return Ok(());
        }
        // Keep the last (live) segment, drop all older.
        for (_, path) in &segments[..segments.len() - 1] {
            if let Err(e) = tokio::fs::remove_file(path).await {
                warn!(path = %path.display(), error = %e, "failed to delete old WAL segment");
            }
        }
        Ok(())
    }

    /// For testing / inspection: list snapshot sequences for a shard.
    pub fn list_snapshots_for(&self, shard_id: u32) -> PersistenceResult<Vec<(u64, PathBuf)>> {
        list_snapshots(&self.config.snapshots_dir(), shard_id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::config::CompressionAlgo;
    use crate::persistence::{FsyncPolicy, WalOp, WalRecord};
    use crate::{Store, StoreConfig, EvictionPolicyKind};
    use tempfile::TempDir;

    fn make_config(dir: &std::path::Path) -> PersistenceConfig {
        PersistenceConfig {
            enabled: true,
            data_dir: dir.to_path_buf(),
            fsync_policy: FsyncPolicy::No,
            segment_size_bytes: 64 * 1024 * 1024,
            snapshot_interval_secs: 0,
            snapshot_retention: 7,
            compression: CompressionAlgo::None,
            zstd_level: 1,
            max_decompressed_size: 256 * 1024 * 1024,
        }
    }

    fn small_store() -> Store {
        Store::new(
            StoreConfig {
                num_shards: 4,
                eviction_policy: EvictionPolicyKind::None,
                ..StoreConfig::default()
            },
            kaya_compress::CompressConfig::default(),
        )
    }

    // --- manager construction + WAL log + sync_all --------------------------

    #[tokio::test]
    async fn manager_log_and_sync() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        let mgr = PersistenceManager::new(config, 4).await.unwrap();

        let rec = WalRecord::new(
            WalOp::Set,
            0,
            0,
            bytes::Bytes::from_static(b"mgr-key"),
            bytes::Bytes::from_static(b"mgr-val"),
        );
        let ts = mgr.log(0, rec).await.unwrap();
        assert_eq!(ts, 1, "first record must get logical_ts=1");

        mgr.sync_all().await.unwrap();
    }

    // --- checkpoint_all produces one snapshot file per shard ----------------

    #[tokio::test]
    async fn manager_checkpoint_all() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        let num_shards = 4;
        let mgr = PersistenceManager::new(config.clone(), num_shards)
            .await
            .unwrap();

        let store = Arc::new(small_store());
        // Insert a key into shard 0.
        store.set(b"chk-key", b"chk-val", None).unwrap();

        let paths = mgr.checkpoint_all(&store).await.unwrap();
        assert_eq!(paths.len(), num_shards);
        for p in &paths {
            assert!(p.exists(), "snapshot file must exist: {}", p.display());
        }
        // Each shard must have exactly one snapshot listed.
        for shard_id in 0..num_shards as u32 {
            let snaps = mgr.list_snapshots_for(shard_id).unwrap();
            assert!(!snaps.is_empty(), "shard {shard_id} must have a snapshot");
        }
    }

    // --- recover after WAL append: logical_ts re-aligned --------------------

    #[tokio::test]
    async fn manager_recover_realigns_logical_ts() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        let store = Arc::new(small_store());

        // First session: log 3 records to shard 0.
        {
            let mgr = PersistenceManager::new(config.clone(), 4).await.unwrap();
            for i in 1u64..=3 {
                let k = format!("k{i}");
                let rec = WalRecord::new(
                    WalOp::Set,
                    0,
                    0,
                    bytes::Bytes::from(k.into_bytes()),
                    bytes::Bytes::from_static(b"v"),
                );
                mgr.log(0, rec).await.unwrap();
            }
            mgr.sync_all().await.unwrap();
        }

        // Second session: recover and verify logical_ts >= 3.
        let mgr2 = PersistenceManager::new(config, 4).await.unwrap();
        let reports = mgr2.recover(&store).await.unwrap();
        assert_eq!(reports.len(), 4);
        let r0 = &reports[0];
        assert_eq!(r0.wal_records_replayed, 3);
        // Logical ts must be at least 3 so new appends continue from 4+.
        assert!(r0.last_logical_ts >= 3);
    }

    // --- checkpoint + recovery round-trip -----------------------------------

    #[tokio::test]
    async fn manager_checkpoint_then_recover_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = make_config(dir.path());
        let store = Arc::new(small_store());
        store.set(b"persist-key", b"persist-val", None).unwrap();

        // Checkpoint.
        let mgr = PersistenceManager::new(config.clone(), 4).await.unwrap();
        mgr.checkpoint_all(&store).await.unwrap();

        // Recover into a fresh store.
        let store2 = Arc::new(small_store());
        let mgr2 = PersistenceManager::new(config, 4).await.unwrap();
        let reports = mgr2.recover(&store2).await.unwrap();

        // At least one shard must report a loaded snapshot.
        let any_snap = reports.iter().any(|r| r.snapshot_loaded.is_some());
        assert!(any_snap, "some shard must have loaded the checkpoint snapshot");

        // The key must be visible in the restored store.
        let got = store2.get(b"persist-key").unwrap();
        assert_eq!(got.as_deref(), Some(b"persist-val".as_ref()));
    }
}
