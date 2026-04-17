//! Write-Ahead-Log: append-only, segmented, per-shard.
//!
//! Record wire format (little-endian unless noted):
//! ```text
//!   u8   op_type           // see [`WalOp`]
//!   u32  shard_id
//!   u64  logical_ts        // monotonic per shard
//!   u32  key_len
//!   [u8; key_len] key
//!   u32  val_len           // 0 when absent
//!   [u8; val_len] value
//!   u64  extra             // aux field (TTL, score bits, int delta)
//!   u64  crc_xxh3          // xxh3_64 over all previous bytes in the record
//! ```
//!
//! Each segment file starts with an 8-byte magic `KAYAWAL1` followed by a
//! `u32` format-version. Records are written sequentially; the writer rolls
//! to a new segment once the current one exceeds
//! [`crate::persistence::PersistenceConfig::segment_size_bytes`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use prometheus::{Histogram, HistogramOpts, IntCounter};
use serde::{Deserialize, Serialize};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Notify;
use tracing::{debug, instrument, warn};
use xxhash_rust::xxh3::xxh3_64;

use super::config::{FsyncPolicy, PersistenceConfig};
use super::{PersistenceError, PersistenceResult};

/// WAL file magic header.
pub const WAL_MAGIC: &[u8; 8] = b"KAYAWAL1";
/// WAL on-disk format version.
pub const WAL_VERSION: u32 = 1;
/// Group-commit window for [`FsyncPolicy::Always`].
const GROUP_COMMIT_WINDOW: Duration = Duration::from_micros(100);

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

static WAL_FSYNC_LATENCY: Lazy<Histogram> = Lazy::new(|| {
    Histogram::with_opts(
        HistogramOpts::new(
            "kaya_wal_fsync_latency_ms",
            "Latency of WAL fsync operations in milliseconds.",
        )
        .buckets(vec![0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0]),
    )
    .expect("build histogram")
});

static WAL_APPEND_BYTES: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "kaya_wal_append_bytes_total",
        "Total number of bytes appended to KAYA WAL segments.",
    )
    .expect("build counter")
});

static WAL_IO_ERRORS: Lazy<IntCounter> = Lazy::new(|| {
    IntCounter::new(
        "kaya_wal_io_errors_total",
        "Total number of I/O errors encountered by the KAYA WAL.",
    )
    .expect("build counter")
});

/// Register all WAL metrics into the supplied registry.
pub fn register_metrics(reg: &prometheus::Registry) -> prometheus::Result<()> {
    reg.register(Box::new(WAL_FSYNC_LATENCY.clone()))?;
    reg.register(Box::new(WAL_APPEND_BYTES.clone()))?;
    reg.register(Box::new(WAL_IO_ERRORS.clone()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Record
// ---------------------------------------------------------------------------

/// Logical operation captured in a WAL record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WalOp {
    /// `SET key value` (with optional TTL in `extra`, 0 = no TTL).
    Set = 1,
    /// `DEL key`.
    Del = 2,
    /// `EXPIRE key seconds` (seconds stored in `extra`).
    Expire = 3,
    /// Reserved: set-add-with-expiry (not currently emitted).
    SetExAdd = 4,
    /// `SADD key member`.
    SAdd = 5,
    /// `SREM key member`.
    SRem = 6,
    /// `ZADD key member` (score bits in `extra` as `f64::to_bits`).
    ZAdd = 7,
    /// `ZREM key member`.
    ZRem = 8,
    /// `INCRBY key delta` (delta stored in `extra` as `i64 as u64`).
    Incr = 9,
    /// `FLUSHDB` (no key/value; affects all shards).
    Flush = 10,
}

impl WalOp {
    /// Convert a raw `u8` from disk into a typed [`WalOp`].
    pub fn from_u8(b: u8) -> Option<Self> {
        Some(match b {
            1 => Self::Set,
            2 => Self::Del,
            3 => Self::Expire,
            4 => Self::SetExAdd,
            5 => Self::SAdd,
            6 => Self::SRem,
            7 => Self::ZAdd,
            8 => Self::ZRem,
            9 => Self::Incr,
            10 => Self::Flush,
            _ => return None,
        })
    }
}

/// A single WAL record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalRecord {
    /// Operation type.
    pub op: WalOp,
    /// Shard this record targets.
    pub shard_id: u32,
    /// Monotonic logical timestamp (per-shard sequence).
    pub logical_ts: u64,
    /// Primary key bytes.
    pub key: Bytes,
    /// Value bytes (empty for `DEL`, `Expire`, etc.).
    pub value: Bytes,
    /// Auxiliary integer payload (TTL seconds, score bits, delta).
    pub extra: u64,
}

impl WalRecord {
    /// Create a record with default extra=0.
    pub fn new(op: WalOp, shard_id: u32, logical_ts: u64, key: Bytes, value: Bytes) -> Self {
        Self {
            op,
            shard_id,
            logical_ts,
            key,
            value,
            extra: 0,
        }
    }

    /// Encode the record (including trailing CRC) into `out`.
    pub fn encode(&self, out: &mut BytesMut) {
        let start = out.len();
        out.put_u8(self.op as u8);
        out.put_u32_le(self.shard_id);
        out.put_u64_le(self.logical_ts);
        out.put_u32_le(self.key.len() as u32);
        out.extend_from_slice(&self.key);
        out.put_u32_le(self.value.len() as u32);
        out.extend_from_slice(&self.value);
        out.put_u64_le(self.extra);
        let crc = xxh3_64(&out[start..]);
        out.put_u64_le(crc);
    }

    /// Decode one record from `buf`, returning the number of bytes consumed.
    ///
    /// Returns `Ok(None)` if `buf` is shorter than a complete record (caller
    /// should treat this as EOF/truncation of the last record).
    pub fn decode(buf: &[u8]) -> PersistenceResult<Option<(Self, usize)>> {
        // Minimum size = op(1) + shard(4) + ts(8) + klen(4) + vlen(4) + extra(8) + crc(8)
        const MIN: usize = 1 + 4 + 8 + 4 + 4 + 8 + 8;
        if buf.len() < MIN {
            return Ok(None);
        }
        let mut cur = &buf[..];
        let op_byte = cur.get_u8();
        let op = WalOp::from_u8(op_byte).ok_or_else(|| {
            PersistenceError::CorruptSegment(format!("unknown op byte {op_byte}"))
        })?;
        let shard_id = cur.get_u32_le();
        let logical_ts = cur.get_u64_le();
        let key_len = cur.get_u32_le() as usize;
        if cur.remaining() < key_len + 4 {
            return Ok(None);
        }
        let key = Bytes::copy_from_slice(&cur[..key_len]);
        cur.advance(key_len);
        let val_len = cur.get_u32_le() as usize;
        if cur.remaining() < val_len + 8 + 8 {
            return Ok(None);
        }
        let value = Bytes::copy_from_slice(&cur[..val_len]);
        cur.advance(val_len);
        let extra = cur.get_u64_le();
        let crc_stored = cur.get_u64_le();

        let total = MIN + key_len + val_len;
        let crc_computed = xxh3_64(&buf[..total - 8]);
        if crc_stored != crc_computed {
            return Err(PersistenceError::CorruptSegment(format!(
                "bad CRC (stored={crc_stored:x}, computed={crc_computed:x})"
            )));
        }

        Ok(Some((
            Self {
                op,
                shard_id,
                logical_ts,
                key,
                value,
                extra,
            },
            total,
        )))
    }
}

// ---------------------------------------------------------------------------
// Segment paths
// ---------------------------------------------------------------------------

/// Construct the path for the given (shard, segment) pair.
pub fn segment_path(wal_dir: &Path, shard_id: u32, segment_num: u64) -> PathBuf {
    wal_dir.join(format!("wal-{shard_id:05}-{segment_num:010}.log"))
}

/// List all WAL segments for `shard_id`, sorted ascending by segment number.
pub fn list_segments(wal_dir: &Path, shard_id: u32) -> PersistenceResult<Vec<(u64, PathBuf)>> {
    let mut out = Vec::new();
    if !wal_dir.exists() {
        return Ok(out);
    }
    let prefix = format!("wal-{shard_id:05}-");
    for entry in std::fs::read_dir(wal_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_s = match name.to_str() {
            Some(s) => s,
            None => continue,
        };
        if !name_s.starts_with(&prefix) || !name_s.ends_with(".log") {
            continue;
        }
        let middle = &name_s[prefix.len()..name_s.len() - ".log".len()];
        if let Ok(num) = middle.parse::<u64>() {
            out.push((num, entry.path()));
        }
    }
    out.sort_by_key(|(n, _)| *n);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Append-only writer for a single shard's WAL.
pub struct WalWriter {
    shard_id: u32,
    wal_dir: PathBuf,
    segment_size: u64,
    fsync_policy: FsyncPolicy,
    inner: Arc<Mutex<WriterInner>>,
    /// Wakes the background group-commit task when new records are appended
    /// under [`FsyncPolicy::Always`].
    notify: Arc<Notify>,
}

struct WriterInner {
    segment_num: u64,
    file: Option<File>,
    bytes_in_segment: u64,
    /// True when there is unflushed data waiting for the group-commit task.
    dirty: bool,
    /// Set to true when the writer is dropped so background tasks exit.
    closed: bool,
}

impl WalWriter {
    /// Open (or create) the WAL for `shard_id`, positioning at the tail of
    /// the highest-numbered existing segment.
    #[instrument(skip(config), fields(shard_id = shard_id))]
    pub async fn open(shard_id: u32, config: &PersistenceConfig) -> PersistenceResult<Self> {
        let wal_dir = config.wal_dir();
        tokio::fs::create_dir_all(&wal_dir).await?;

        let segments = list_segments(&wal_dir, shard_id)?;
        let (segment_num, file, bytes) = match segments.last() {
            Some((n, path)) => {
                let file = OpenOptions::new().append(true).read(true).open(path).await?;
                let meta = file.metadata().await?;
                (*n, file, meta.len())
            }
            None => {
                let n = 0u64;
                let path = segment_path(&wal_dir, shard_id, n);
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .read(true)
                    .open(&path)
                    .await?;
                write_segment_header(&mut file).await?;
                let bytes = file.metadata().await?.len();
                (n, file, bytes)
            }
        };

        let writer = Self {
            shard_id,
            wal_dir,
            segment_size: config.segment_size_bytes,
            fsync_policy: config.fsync_policy,
            inner: Arc::new(Mutex::new(WriterInner {
                segment_num,
                file: Some(file),
                bytes_in_segment: bytes,
                dirty: false,
                closed: false,
            })),
            notify: Arc::new(Notify::new()),
        };

        writer.spawn_group_commit();
        Ok(writer)
    }

    /// Append a single record. Depending on [`FsyncPolicy`], the caller may
    /// or may not see durable storage on return.
    #[instrument(skip(self, record), fields(shard_id = self.shard_id, op = ?record.op))]
    pub async fn append(&self, record: WalRecord) -> PersistenceResult<()> {
        let mut buf = BytesMut::with_capacity(64 + record.key.len() + record.value.len());
        record.encode(&mut buf);
        let bytes = buf.freeze();
        let len = bytes.len() as u64;

        // Check rotation first; may take the file out for a moment.
        let file_arc = {
            let g = self.inner.lock();
            if g.bytes_in_segment + len > self.segment_size {
                drop(g);
                self.rotate().await?;
                self.inner.lock()
            } else {
                g
            }
        };
        let mut guard = file_arc;
        let file = guard
            .file
            .as_mut()
            .ok_or_else(|| PersistenceError::Internal("WAL file closed".into()))?;

        if let Err(e) = file.write_all(&bytes).await {
            WAL_IO_ERRORS.inc();
            return Err(PersistenceError::Io(e));
        }
        guard.bytes_in_segment += len;
        guard.dirty = true;
        WAL_APPEND_BYTES.inc_by(len);

        match self.fsync_policy {
            FsyncPolicy::Always => {
                // Trigger group commit; wait until fsync completes.
                drop(guard);
                self.notify.notify_waiters();
                // Fall through and do a sync here too to guarantee durability
                // on return. The group-commit task amortizes across pending
                // writers via the OS buffer merging.
                self.sync_now().await?;
            }
            FsyncPolicy::EverySec | FsyncPolicy::No => {
                // Handled by the background task.
            }
        }

        Ok(())
    }

    /// Force a flush + fsync to disk right now, regardless of policy.
    #[instrument(skip(self), fields(shard_id = self.shard_id))]
    pub async fn sync_now(&self) -> PersistenceResult<()> {
        let start = std::time::Instant::now();

        // We need to sync the current file. Take a cloned std File handle by
        // temporarily removing from the guarded option.
        let mut guard = self.inner.lock();
        let file = match guard.file.as_mut() {
            Some(f) => f,
            None => return Ok(()),
        };
        // Flush buffered data (tokio File uses the underlying std handle
        // which is unbuffered, but call flush for correctness).
        if let Err(e) = file.flush().await {
            WAL_IO_ERRORS.inc();
            return Err(PersistenceError::Io(e));
        }
        if let Err(e) = file.sync_data().await {
            WAL_IO_ERRORS.inc();
            return Err(PersistenceError::Io(e));
        }
        guard.dirty = false;
        drop(guard);

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        WAL_FSYNC_LATENCY.observe(elapsed_ms);
        Ok(())
    }

    /// Rotate to a fresh segment. Called automatically from [`Self::append`]
    /// when the current segment would overflow.
    async fn rotate(&self) -> PersistenceResult<()> {
        // Finalize old file: fsync, close.
        {
            let mut guard = self.inner.lock();
            if let Some(mut f) = guard.file.take() {
                // Best-effort sync; errors are logged but not fatal here.
                drop(f.flush().await);
                drop(f.sync_data().await);
            }
        }
        // Open new segment.
        let new_num = {
            let mut guard = self.inner.lock();
            guard.segment_num = guard.segment_num.saturating_add(1);
            guard.segment_num
        };
        let path = segment_path(&self.wal_dir, self.shard_id, new_num);
        let mut new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)
            .await?;
        write_segment_header(&mut new_file).await?;
        let bytes = new_file.metadata().await?.len();

        let mut guard = self.inner.lock();
        guard.file = Some(new_file);
        guard.bytes_in_segment = bytes;
        Ok(())
    }

    fn spawn_group_commit(&self) {
        let policy = self.fsync_policy;
        if matches!(policy, FsyncPolicy::No) {
            return;
        }
        let inner = Arc::clone(&self.inner);
        let notify = Arc::clone(&self.notify);
        let shard_id = self.shard_id;
        tokio::spawn(async move {
            loop {
                // Exit if writer was dropped.
                if inner.lock().closed {
                    debug!(shard_id, "WAL group-commit task exiting");
                    return;
                }
                match policy {
                    FsyncPolicy::Always => {
                        // Wait up to window, then batch any pending fsyncs.
                        tokio::select! {
                            _ = notify.notified() => {}
                            _ = tokio::time::sleep(GROUP_COMMIT_WINDOW) => {}
                        }
                    }
                    FsyncPolicy::EverySec => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    FsyncPolicy::No => return,
                }
                let start = std::time::Instant::now();
                // Extract file handle while the guard is in a local scope so
                // the non-Send MutexGuard is dropped before any await point.
                let file_opt: Option<File> = {
                    let mut g = inner.lock();
                    if !g.dirty {
                        continue;
                    }
                    g.file.take()
                    // g is dropped here, before any await
                };
                if let Some(mut f) = file_opt {
                    let res = async {
                        f.flush().await?;
                        f.sync_data().await?;
                        Ok::<_, std::io::Error>(())
                    }
                    .await;
                    if let Err(e) = res {
                        WAL_IO_ERRORS.inc();
                        warn!(shard_id, error = %e, "WAL fsync failed");
                    } else {
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        WAL_FSYNC_LATENCY.observe(ms);
                    }
                    let mut g = inner.lock();
                    g.file = Some(f);
                    g.dirty = false;
                }
            }
        });
    }
}

impl Drop for WalWriter {
    fn drop(&mut self) {
        if let Some(mut g) = self.inner.try_lock() {
            g.closed = true;
        }
        self.notify.notify_waiters();
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Streaming reader over a single WAL segment file.
///
/// The reader validates segment magic, then yields records one by one. A
/// truncated final record (e.g. the process crashed mid-write) is reported
/// as a clean EOF rather than an error, allowing recovery to continue.
pub struct WalReader {
    file: File,
    path: PathBuf,
    buf: BytesMut,
    /// Soft-corruption flag: set when we saw a partial tail record.
    pub tail_truncated: bool,
    eof: bool,
}

impl WalReader {
    /// Open a segment file for reading; verifies the header.
    #[instrument(skip_all, fields(path = %path.as_ref().display()))]
    pub async fn open(path: impl AsRef<Path>) -> PersistenceResult<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path).await?;
        let mut hdr = [0u8; 12];
        file.read_exact(&mut hdr).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                PersistenceError::BadMagic(path.display().to_string())
            } else {
                PersistenceError::Io(e)
            }
        })?;
        if &hdr[..8] != WAL_MAGIC {
            return Err(PersistenceError::BadMagic(path.display().to_string()));
        }
        let version = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]);
        if version != WAL_VERSION {
            return Err(PersistenceError::BadMagic(format!(
                "{} (version {} != {})",
                path.display(),
                version,
                WAL_VERSION
            )));
        }
        Ok(Self {
            file,
            path,
            buf: BytesMut::with_capacity(64 * 1024),
            tail_truncated: false,
            eof: false,
        })
    }

    /// Read the next record, returning `Ok(None)` at EOF. A CRC failure
    /// returns a [`PersistenceError::CorruptSegment`] error; the caller may
    /// choose to tolerate it for the very last record (tail truncation).
    pub async fn next(&mut self) -> PersistenceResult<Option<WalRecord>> {
        loop {
            // Try decoding from the current buffer.
            match WalRecord::decode(&self.buf) {
                Ok(Some((rec, consumed))) => {
                    let _ = self.buf.split_to(consumed);
                    return Ok(Some(rec));
                }
                Ok(None) => {
                    if self.eof {
                        if !self.buf.is_empty() {
                            self.tail_truncated = true;
                            warn!(
                                path = %self.path.display(),
                                trailing_bytes = self.buf.len(),
                                "truncating partial WAL record at segment tail"
                            );
                            self.buf.clear();
                        }
                        return Ok(None);
                    }
                    // Need more data.
                    let mut chunk = vec![0u8; 32 * 1024];
                    match self.file.read(&mut chunk).await {
                        Ok(0) => {
                            self.eof = true;
                        }
                        Ok(n) => {
                            self.buf.extend_from_slice(&chunk[..n]);
                        }
                        Err(e) => {
                            WAL_IO_ERRORS.inc();
                            return Err(PersistenceError::Io(e));
                        }
                    }
                }
                Err(PersistenceError::CorruptSegment(msg)) => {
                    // If this is the very last record and we're at EOF,
                    // treat it as truncation rather than corruption.
                    if self.eof && self.buf.len() < 64 * 1024 {
                        self.tail_truncated = true;
                        warn!(
                            path = %self.path.display(),
                            msg = %msg,
                            "tail record failed CRC, treating as truncation"
                        );
                        self.buf.clear();
                        return Ok(None);
                    }
                    return Err(PersistenceError::CorruptSegment(msg));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Seek back to just after the segment header. Useful for re-scanning.
    pub async fn rewind(&mut self) -> PersistenceResult<()> {
        self.file.seek(std::io::SeekFrom::Start(12)).await?;
        self.buf.clear();
        self.eof = false;
        self.tail_truncated = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn write_segment_header(file: &mut File) -> PersistenceResult<()> {
    file.write_all(WAL_MAGIC).await?;
    file.write_all(&WAL_VERSION.to_le_bytes()).await?;
    file.flush().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_roundtrip() {
        let rec = WalRecord {
            op: WalOp::Set,
            shard_id: 7,
            logical_ts: 42,
            key: Bytes::from_static(b"hello"),
            value: Bytes::from_static(b"world"),
            extra: 123,
        };
        let mut buf = BytesMut::new();
        rec.encode(&mut buf);
        let (decoded, n) = WalRecord::decode(&buf).unwrap().unwrap();
        assert_eq!(decoded, rec);
        assert_eq!(n, buf.len());
    }

    #[test]
    fn record_short_buffer_returns_none() {
        let rec = WalRecord::new(
            WalOp::Del,
            0,
            1,
            Bytes::from_static(b"k"),
            Bytes::new(),
        );
        let mut buf = BytesMut::new();
        rec.encode(&mut buf);
        // Truncate any amount
        let truncated = &buf[..buf.len() - 5];
        assert!(WalRecord::decode(truncated).unwrap().is_none());
    }

    #[test]
    fn record_corrupt_crc_returns_err() {
        let rec = WalRecord::new(
            WalOp::Set,
            0,
            1,
            Bytes::from_static(b"k"),
            Bytes::from_static(b"v"),
        );
        let mut buf = BytesMut::new();
        rec.encode(&mut buf);
        let last = buf.len() - 1;
        buf[last] ^= 0xff;
        assert!(matches!(
            WalRecord::decode(&buf),
            Err(PersistenceError::CorruptSegment(_))
        ));
    }
}
