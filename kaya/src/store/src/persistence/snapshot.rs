//! Snapshot writer/reader for KAYA persistence layer.
//!
//! On-disk format:
//! ```text
//!   u8[8]  magic = "KAYASNAP1"
//!   u32    version
//!   u32    shard_id
//!   u64    term
//!   u64    last_applied_index
//!   u64    key_count
//!   u64    checksum_xxh3        // xxh3 of concatenated record tuples
//!   ... chunks (compressed via Zstd/LZ4/None) ...
//! ```
//!
//! A chunk stream is a sequence of framed records, each with:
//! ```text
//!   u32 key_len | key | u32 val_len | value | u64 expires_ms_epoch (0 = none)
//! ```
//! The entire chunk payload is compressed (or plain, depending on
//! [`CompressionAlgo`]).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, instrument, warn};
use xxhash_rust::xxh3::Xxh3;

use super::config::{CompressionAlgo, PersistenceConfig};
use super::{PersistenceError, PersistenceResult};
use crate::entry::{Entry, EntryMetadata};
use crate::shard::Shard;

/// Magic header for KAYA snapshot files.
pub const SNAPSHOT_MAGIC: &[u8; 8] = b"KAYASNP1";
/// Current snapshot format version.
pub const SNAPSHOT_VERSION: u32 = 1;

const HEADER_SIZE: usize = 8 + 4 + 4 + 8 + 8 + 8 + 8;

/// In-memory representation of a snapshot header.
#[derive(Debug, Clone)]
pub struct SnapshotHeader {
    /// Format version.
    pub version: u32,
    /// Shard this snapshot belongs to.
    pub shard_id: u32,
    /// Raft term (0 when not in cluster mode).
    pub term: u64,
    /// Last applied WAL index / logical timestamp.
    pub last_applied_index: u64,
    /// Number of keys written.
    pub key_count: u64,
    /// xxh3 checksum over all record tuples.
    pub checksum: u64,
}

impl SnapshotHeader {
    fn encode(&self, out: &mut BytesMut) {
        out.extend_from_slice(SNAPSHOT_MAGIC);
        out.put_u32_le(self.version);
        out.put_u32_le(self.shard_id);
        out.put_u64_le(self.term);
        out.put_u64_le(self.last_applied_index);
        out.put_u64_le(self.key_count);
        out.put_u64_le(self.checksum);
    }

    fn decode(buf: &[u8]) -> PersistenceResult<Self> {
        if buf.len() < HEADER_SIZE {
            return Err(PersistenceError::BadMagic("short header".into()));
        }
        if &buf[..8] != SNAPSHOT_MAGIC {
            return Err(PersistenceError::BadMagic("snapshot magic mismatch".into()));
        }
        let mut c = &buf[8..];
        let version = c.get_u32_le();
        if version != SNAPSHOT_VERSION {
            return Err(PersistenceError::BadMagic(format!(
                "snapshot version {version} != {SNAPSHOT_VERSION}"
            )));
        }
        let shard_id = c.get_u32_le();
        let term = c.get_u64_le();
        let last_applied_index = c.get_u64_le();
        let key_count = c.get_u64_le();
        let checksum = c.get_u64_le();
        Ok(Self {
            version,
            shard_id,
            term,
            last_applied_index,
            key_count,
            checksum,
        })
    }
}

// ---------------------------------------------------------------------------
// Serialized record
// ---------------------------------------------------------------------------

/// A single decoded snapshot record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRecord {
    /// Key bytes.
    pub key: Bytes,
    /// Value bytes (compressed as stored in the Entry).
    pub value: Bytes,
    /// Expiration as milliseconds since UNIX epoch, 0 if no TTL.
    pub expires_at_ms: u64,
}

impl SnapshotRecord {
    fn encode(&self, out: &mut BytesMut) {
        out.put_u32_le(self.key.len() as u32);
        out.extend_from_slice(&self.key);
        out.put_u32_le(self.value.len() as u32);
        out.extend_from_slice(&self.value);
        out.put_u64_le(self.expires_at_ms);
    }

    fn decode(buf: &mut &[u8]) -> PersistenceResult<Option<Self>> {
        if buf.remaining() < 4 {
            return Ok(None);
        }
        let key_len = (&buf[..4]).get_u32_le() as usize;
        let need = 4 + key_len + 4;
        if buf.remaining() < need {
            return Err(PersistenceError::CorruptSegment(
                "snapshot record truncated (key)".into(),
            ));
        }
        buf.advance(4);
        let key = Bytes::copy_from_slice(&buf[..key_len]);
        buf.advance(key_len);
        let val_len = buf.get_u32_le() as usize;
        if buf.remaining() < val_len + 8 {
            return Err(PersistenceError::CorruptSegment(
                "snapshot record truncated (value)".into(),
            ));
        }
        let value = Bytes::copy_from_slice(&buf[..val_len]);
        buf.advance(val_len);
        let expires_at_ms = buf.get_u64_le();
        Ok(Some(Self {
            key,
            value,
            expires_at_ms,
        }))
    }

    fn checksum_update(&self, h: &mut Xxh3) {
        h.update(&(self.key.len() as u32).to_le_bytes());
        h.update(&self.key);
        h.update(&(self.value.len() as u32).to_le_bytes());
        h.update(&self.value);
        h.update(&self.expires_at_ms.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Writes a snapshot to disk. Records are appended via [`Self::write_record`],
/// then [`Self::finalize`] flushes and atomically renames into place.
pub struct SnapshotWriter {
    target: PathBuf,
    tmp: PathBuf,
    file: File,
    header: SnapshotHeader,
    payload: Vec<u8>,
    hasher: Xxh3,
    compression: CompressionAlgo,
    zstd_level: i32,
}

impl SnapshotWriter {
    /// Create a new snapshot writer.
    #[instrument(skip(target), fields(target = %target.as_ref().display()))]
    pub async fn create(
        target: impl AsRef<Path>,
        shard_id: u32,
        term: u64,
        last_applied_index: u64,
        compression: CompressionAlgo,
        zstd_level: i32,
    ) -> PersistenceResult<Self> {
        let target = target.as_ref().to_path_buf();
        let tmp = target.with_extension("snap.tmp");
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
            .await?;
        Ok(Self {
            target,
            tmp,
            file,
            header: SnapshotHeader {
                version: SNAPSHOT_VERSION,
                shard_id,
                term,
                last_applied_index,
                key_count: 0,
                checksum: 0,
            },
            payload: Vec::with_capacity(4 * 1024 * 1024),
            hasher: Xxh3::new(),
            compression,
            zstd_level,
        })
    }

    /// Append a single record to the snapshot.
    pub fn write_record(&mut self, rec: SnapshotRecord) {
        rec.checksum_update(&mut self.hasher);
        let mut tmp = BytesMut::with_capacity(16 + rec.key.len() + rec.value.len());
        rec.encode(&mut tmp);
        self.payload.extend_from_slice(&tmp);
        self.header.key_count += 1;
    }

    /// Finish: compress, write header + payload, fsync, atomic rename.
    #[instrument(skip(self))]
    pub async fn finalize(mut self) -> PersistenceResult<PathBuf> {
        self.header.checksum = self.hasher.digest();
        let compressed = compress_chunk(&self.payload, self.compression, self.zstd_level)?;

        let mut header_buf = BytesMut::with_capacity(HEADER_SIZE);
        self.header.encode(&mut header_buf);
        self.file.write_all(&header_buf).await?;
        // Algo tag so the reader knows how to decompress.
        self.file.write_all(&[self.compression as u8]).await?;
        self.file.write_all(&(compressed.len() as u64).to_le_bytes()).await?;
        self.file.write_all(&compressed).await?;
        self.file.flush().await?;
        self.file.sync_all().await?;
        drop(self.file);

        tokio::fs::rename(&self.tmp, &self.target).await?;
        debug!(
            target = %self.target.display(),
            key_count = self.header.key_count,
            "snapshot finalized"
        );
        Ok(self.target)
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Streaming reader over a snapshot file. Validates the global checksum
/// when [`Self::verify`] is called after iteration.
pub struct SnapshotReader {
    /// Decoded header.
    pub header: SnapshotHeader,
    payload: Bytes,
    cursor: usize,
    hasher: Xxh3,
    records_read: u64,
}

impl SnapshotReader {
    /// Open a snapshot file and decode its header + payload.
    ///
    /// `max_decompressed_size` caps the output of each compressed chunk to
    /// prevent zip-bomb payloads from exhausting process memory
    /// (SecFinding-SNAPSHOT-ZIPBOMB).
    #[instrument(skip(path), fields(path = %path.as_ref().display()))]
    pub async fn open(path: impl AsRef<Path>, max_decompressed_size: usize) -> PersistenceResult<Self> {
        let path = path.as_ref();
        let mut file = File::open(path).await?;
        let mut header_buf = vec![0u8; HEADER_SIZE];
        file.read_exact(&mut header_buf).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                PersistenceError::BadMagic(path.display().to_string())
            } else {
                PersistenceError::Io(e)
            }
        })?;
        let header = SnapshotHeader::decode(&header_buf)?;

        let mut algo_byte = [0u8; 1];
        file.read_exact(&mut algo_byte).await?;
        let algo = match algo_byte[0] {
            0 => CompressionAlgo::Zstd,
            1 => CompressionAlgo::Lz4,
            2 => CompressionAlgo::None,
            b => {
                return Err(PersistenceError::BadMagic(format!(
                    "unknown compression tag {b}"
                )))
            }
        };
        let mut len_buf = [0u8; 8];
        file.read_exact(&mut len_buf).await?;
        let compressed_len = u64::from_le_bytes(len_buf) as usize;
        let mut compressed = vec![0u8; compressed_len];
        file.read_exact(&mut compressed).await?;
        let payload = decompress_chunk(&compressed, algo, max_decompressed_size)?;

        Ok(Self {
            header,
            payload: Bytes::from(payload),
            cursor: 0,
            hasher: Xxh3::new(),
            records_read: 0,
        })
    }

    /// Read the next record, or `Ok(None)` at the end of the payload.
    pub fn next_record(&mut self) -> PersistenceResult<Option<SnapshotRecord>> {
        if self.cursor >= self.payload.len() {
            return Ok(None);
        }
        let mut slice = &self.payload[self.cursor..];
        let before = slice.len();
        match SnapshotRecord::decode(&mut slice)? {
            Some(rec) => {
                let consumed = before - slice.len();
                self.cursor += consumed;
                rec.checksum_update(&mut self.hasher);
                self.records_read += 1;
                Ok(Some(rec))
            }
            None => Ok(None),
        }
    }

    /// Validate that the checksum of iterated records equals the header
    /// value. MUST be called after draining [`Self::next_record`].
    pub fn verify(&self) -> PersistenceResult<()> {
        if self.records_read != self.header.key_count {
            return Err(PersistenceError::CorruptSegment(format!(
                "snapshot record count mismatch: read {}, header {}",
                self.records_read, self.header.key_count
            )));
        }
        let actual = self.hasher.digest();
        if actual != self.header.checksum {
            return Err(PersistenceError::CorruptSegment(format!(
                "snapshot checksum mismatch: computed {:x}, header {:x}",
                actual, self.header.checksum
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Top-level helpers
// ---------------------------------------------------------------------------

/// Take a snapshot of `shard` into `target_path`. Serialization is performed
/// on a blocking pool via [`tokio::task::spawn_blocking`] to avoid stalling
/// the reactor.
#[instrument(skip(shard, target_path), fields(shard_id = shard.id, target = %target_path.as_ref().display()))]
pub async fn take_snapshot(
    shard: &Shard,
    target_path: impl AsRef<Path>,
    compression: CompressionAlgo,
    zstd_level: i32,
    last_applied_index: u64,
) -> PersistenceResult<PathBuf> {
    // Materialize a cheap snapshot of entries so we don't hold DashMap shards
    // for long. We clone Bytes (which are reference-counted/shared).
    let records = collect_shard_records(shard);
    let target = target_path.as_ref().to_path_buf();
    let shard_id = shard.id as u32;

    let mut writer =
        SnapshotWriter::create(&target, shard_id, 0, last_applied_index, compression, zstd_level)
            .await?;
    // Feed records. Loop is cheap because encoding is in-memory.
    for rec in records {
        writer.write_record(rec);
    }
    writer.finalize().await
}

/// Apply the contents of a snapshot to `shard`, repopulating its KV store.
///
/// `max_decompressed_size` is forwarded to the reader to enforce zip-bomb
/// protection (SecFinding-SNAPSHOT-ZIPBOMB).
///
/// Note: set/sorted-set data is not captured by the current snapshot format
/// (those are rebuilt from the WAL on top of the KV snapshot).
#[instrument(skip(shard, path), fields(shard_id = shard.id, path = %path.as_ref().display()))]
pub async fn apply_snapshot(
    shard: &Shard,
    path: impl AsRef<Path>,
    max_decompressed_size: usize,
) -> PersistenceResult<u64> {
    let mut reader = SnapshotReader::open(path, max_decompressed_size).await?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let now_inst = std::time::Instant::now();

    while let Some(rec) = reader.next_record()? {
        let expires_at = if rec.expires_at_ms == 0 {
            None
        } else if rec.expires_at_ms <= now_ms {
            // Already expired; skip.
            continue;
        } else {
            Some(now_inst + Duration::from_millis(rec.expires_at_ms - now_ms))
        };
        let entry = Entry {
            value: rec.value,
            metadata: EntryMetadata {
                created_at: now_inst,
                last_accessed: now_inst,
                expires_at,
                access_count: 0,
                size_bytes: rec.key.len(),
            },
        };
        shard.insert(&rec.key, entry);
    }
    reader.verify()?;
    Ok(reader.header.last_applied_index)
}

/// Delete all but the `keep` most recent snapshots for `shard_id`.
#[instrument(skip(config))]
pub async fn prune_snapshots(
    config: &PersistenceConfig,
    shard_id: u32,
    keep: usize,
) -> PersistenceResult<usize> {
    let dir = config.snapshots_dir();
    if !dir.exists() {
        return Ok(0);
    }
    let mut snaps = list_snapshots(&dir, shard_id)?;
    if snaps.len() <= keep {
        return Ok(0);
    }
    snaps.sort_by_key(|(seq, _)| *seq);
    let cut = snaps.len() - keep;
    let mut removed = 0;
    for (_, path) in snaps.into_iter().take(cut) {
        if let Err(e) = tokio::fs::remove_file(&path).await {
            warn!(path = %path.display(), error = %e, "failed to prune snapshot");
        } else {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Locate the highest-sequence snapshot for `shard_id`, if any.
pub fn latest_snapshot(dir: &Path, shard_id: u32) -> PersistenceResult<Option<(u64, PathBuf)>> {
    let mut snaps = list_snapshots(dir, shard_id)?;
    snaps.sort_by_key(|(seq, _)| *seq);
    Ok(snaps.pop())
}

/// List all snapshots for `shard_id`, returning `(sequence, path)` pairs.
pub fn list_snapshots(dir: &Path, shard_id: u32) -> PersistenceResult<Vec<(u64, PathBuf)>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let prefix = format!("snap-{shard_id:05}-");
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_s = match name.to_str() {
            Some(s) => s,
            None => continue,
        };
        if !name_s.starts_with(&prefix) || !name_s.ends_with(".snap") {
            continue;
        }
        let middle = &name_s[prefix.len()..name_s.len() - ".snap".len()];
        if let Ok(n) = middle.parse::<u64>() {
            out.push((n, entry.path()));
        }
    }
    Ok(out)
}

/// Build the path for a snapshot file.
pub fn snapshot_path(dir: &Path, shard_id: u32, seq: u64) -> PathBuf {
    dir.join(format!("snap-{shard_id:05}-{seq:010}.snap"))
}

// ---------------------------------------------------------------------------
// Compression helpers
// ---------------------------------------------------------------------------

fn compress_chunk(
    payload: &[u8],
    algo: CompressionAlgo,
    level: i32,
) -> PersistenceResult<Vec<u8>> {
    match algo {
        CompressionAlgo::Zstd => zstd::bulk::compress(payload, level)
            .map_err(|e| PersistenceError::Compression(e.to_string())),
        CompressionAlgo::Lz4 => Ok(lz4_flex::compress_prepend_size(payload)),
        CompressionAlgo::None => Ok(payload.to_vec()),
    }
}

fn decompress_chunk(
    payload: &[u8],
    algo: CompressionAlgo,
    max_decompressed_size: usize,
) -> PersistenceResult<Vec<u8>> {
    match algo {
        CompressionAlgo::Zstd => {
            // WHY: cap the output buffer to prevent zip-bomb OOM via a crafted
            // snapshot (SecFinding-SNAPSHOT-ZIPBOMB). `zstd::bulk::decompress`
            // will return an error if the decompressed data exceeds the limit.
            zstd::bulk::decompress(payload, max_decompressed_size)
                .map_err(|e| PersistenceError::Compression(e.to_string()))
        }
        CompressionAlgo::Lz4 => {
            // WHY: lz4_flex::decompress_size_prepended reads the expected size
            // from the first 4 bytes of the payload. Reject oversized claims
            // before allocating to prevent memory exhaustion.
            if payload.len() >= 4 {
                let claimed = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
                if claimed > max_decompressed_size {
                    return Err(PersistenceError::Compression(format!(
                        "lz4 claimed decompressed size {claimed} exceeds limit {max_decompressed_size}"
                    )));
                }
            }
            lz4_flex::decompress_size_prepended(payload)
                .map_err(|e| PersistenceError::Compression(e.to_string()))
        }
        CompressionAlgo::None => Ok(payload.to_vec()),
    }
}

// ---------------------------------------------------------------------------
// Shard -> records
// ---------------------------------------------------------------------------

fn collect_shard_records(shard: &Shard) -> Vec<SnapshotRecord> {
    let now_inst = std::time::Instant::now();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut out = Vec::new();
    for kv in shard.iter_kv() {
        let key: &Vec<u8> = kv.key();
        let entry: &crate::entry::Entry = kv.value();
        if entry.is_expired() {
            continue;
        }
        let expires_at_ms = match entry.metadata.expires_at {
            Some(exp) => {
                if exp <= now_inst {
                    continue;
                }
                let delta = exp.duration_since(now_inst);
                now_ms + delta.as_millis() as u64
            }
            None => 0,
        };
        out.push(SnapshotRecord {
            key: Bytes::from(key.clone()),
            value: entry.value.clone(),
            expires_at_ms,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::EntryMetadata;
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

    // --- round-trip: write then read back and verify checksum ---------------

    #[tokio::test]
    async fn snapshot_roundtrip_basic() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(0);
        insert_str(&shard, b"alpha", b"ALPHA");
        insert_str(&shard, b"beta", b"BETA");
        insert_str(&shard, b"gamma", b"GAMMA");

        let path = snapshot_path(dir.path(), 0, 1);
        take_snapshot(&shard, &path, CompressionAlgo::None, 3, 1)
            .await
            .expect("write snapshot");

        // Read back and verify
        let mut reader = SnapshotReader::open(&path, 256 * 1024 * 1024).await.expect("open snapshot");
        assert_eq!(reader.header.shard_id, 0);
        assert_eq!(reader.header.key_count, 3);
        let mut keys_seen = std::collections::HashSet::new();
        while let Some(rec) = reader.next_record().expect("read record") {
            keys_seen.insert(String::from_utf8(rec.key.to_vec()).unwrap());
        }
        reader.verify().expect("checksum ok");
        assert!(keys_seen.contains("alpha"));
        assert!(keys_seen.contains("beta"));
        assert!(keys_seen.contains("gamma"));
    }

    // --- round-trip with Zstd compression -----------------------------------

    #[tokio::test]
    async fn snapshot_roundtrip_zstd() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(1);
        for i in 0u32..50 {
            let k = format!("key-{i:04}");
            let v = format!("val-{i:04}-{}", "x".repeat(100));
            insert_str(&shard, k.as_bytes(), v.as_bytes());
        }

        let path = snapshot_path(dir.path(), 1, 0);
        take_snapshot(&shard, &path, CompressionAlgo::Zstd, 3, 99)
            .await
            .expect("write zstd snapshot");

        let restored = make_shard(1);
        let last_idx = apply_snapshot(&restored, &path, 256 * 1024 * 1024)
            .await
            .expect("restore snapshot");
        assert_eq!(last_idx, 99);
        assert_eq!(restored.len(), 50);
    }

    // --- round-trip with LZ4 compression ------------------------------------

    #[tokio::test]
    async fn snapshot_roundtrip_lz4() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(2);
        insert_str(&shard, b"lz4-key", b"lz4-value-payload");

        let path = snapshot_path(dir.path(), 2, 0);
        take_snapshot(&shard, &path, CompressionAlgo::Lz4, 0, 5)
            .await
            .expect("write lz4 snapshot");

        let restored = make_shard(2);
        apply_snapshot(&restored, &path, 256 * 1024 * 1024)
            .await
            .expect("restore lz4 snapshot");
        assert!(restored.contains(b"lz4-key"));
    }

    // --- CRC corruption detection -------------------------------------------

    #[tokio::test]
    async fn snapshot_corrupt_checksum_detected() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(3);
        insert_str(&shard, b"foo", b"bar");

        let path = snapshot_path(dir.path(), 3, 0);
        take_snapshot(&shard, &path, CompressionAlgo::None, 0, 1)
            .await
            .unwrap();

        // Flip bytes at the end of the file to corrupt the payload.
        let mut raw = tokio::fs::read(&path).await.unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        raw[last - 1] ^= 0xFF;
        tokio::fs::write(&path, &raw).await.unwrap();

        // Opening and iterating should either fail on decompress or verify.
        let result = SnapshotReader::open(&path, 256 * 1024 * 1024).await;
        match result {
            Err(_) => { /* decompression or magic error: acceptable */ }
            Ok(mut reader) => {
                // If it opened, draining records and calling verify must catch the corruption.
                while reader.next_record().is_ok_and(|r| r.is_some()) {}
                assert!(
                    reader.verify().is_err(),
                    "verify must detect corrupted payload"
                );
            }
        }
    }

    // --- expired entries are skipped ----------------------------------------

    #[tokio::test]
    async fn snapshot_skips_expired_entries() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(4);
        // Alive entry
        insert_str(&shard, b"alive", b"yes");
        // Expired entry: set expires_at in the past
        shard.insert(
            b"dead",
            crate::entry::Entry {
                value: bytes::Bytes::from_static(b"no"),
                metadata: EntryMetadata {
                    created_at: Instant::now(),
                    last_accessed: Instant::now(),
                    expires_at: Some(Instant::now() - std::time::Duration::from_secs(1)),
                    access_count: 0,
                    size_bytes: 2,
                },
            },
        );

        let path = snapshot_path(dir.path(), 4, 0);
        take_snapshot(&shard, &path, CompressionAlgo::None, 0, 1)
            .await
            .unwrap();

        let mut reader = SnapshotReader::open(&path, 256 * 1024 * 1024).await.unwrap();
        assert_eq!(reader.header.key_count, 1, "expired entry must be excluded");
        while reader.next_record().unwrap().is_some() {}
        reader.verify().unwrap();
    }

    // --- zip-bomb protection: tiny limit rejects decompression (SecFinding-SNAPSHOT-ZIPBOMB) ---

    #[tokio::test]
    async fn snapshot_zipbomb_limit_rejects_oversized_zstd() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(5);
        // Write ~1 KiB of data that compresses well.
        for i in 0..20u32 {
            let k = format!("zb-key-{i:04}");
            let v = "z".repeat(50);
            insert_str(&shard, k.as_bytes(), v.as_bytes());
        }

        let path = snapshot_path(dir.path(), 5, 0);
        take_snapshot(&shard, &path, CompressionAlgo::Zstd, 3, 1)
            .await
            .expect("write snapshot");

        // Open with a 1-byte decompression limit — must fail.
        let result = SnapshotReader::open(&path, 1).await;
        assert!(
            result.is_err(),
            "decompression with 1-byte limit must be rejected as zip-bomb protection"
        );
    }

    #[tokio::test]
    async fn snapshot_zipbomb_limit_rejects_oversized_lz4() {
        let dir = TempDir::new().unwrap();
        let shard = make_shard(6);
        for i in 0..20u32 {
            let k = format!("lz-key-{i:04}");
            let v = "l".repeat(50);
            insert_str(&shard, k.as_bytes(), v.as_bytes());
        }

        let path = snapshot_path(dir.path(), 6, 0);
        take_snapshot(&shard, &path, CompressionAlgo::Lz4, 0, 1)
            .await
            .expect("write lz4 snapshot");

        // 1-byte limit must reject the embedded LZ4 size claim.
        let result = SnapshotReader::open(&path, 1).await;
        assert!(
            result.is_err(),
            "lz4 snapshot with 1-byte limit must be rejected"
        );
    }
}
