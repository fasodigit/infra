//! KAYA persistence layer: Write-Ahead-Log, snapshots, and crash recovery.
//!
//! This module provides durability for the KAYA in-memory store via an
//! append-only WAL per shard plus periodic Zstd-compressed snapshots. On
//! restart, the recovery pipeline replays the WAL on top of the latest
//! valid snapshot to reconstruct each shard's state.
//!
//! Architecture
//! ------------
//! * [`wal`] — per-shard append-only segmented log with CRC-xxh3 records and
//!   configurable fsync policy.
//! * [`snapshot`] — streaming writer/reader, Zstd-compressed, atomic rename.
//! * [`recovery`] — snapshot-then-WAL replay, corruption detection, metrics.
//! * [`config`] — serde-friendly configuration types.
//! * [`manager`] — [`PersistenceManager`], the public orchestrator used by
//!   [`crate::Store`].

pub mod config;
pub mod manager;
pub mod recovery;
pub mod snapshot;
pub mod wal;

use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

pub use config::{CompressionAlgo, FsyncPolicy, PersistenceConfig};
pub use manager::PersistenceManager;
pub use recovery::RecoveryReport;
pub use snapshot::{SnapshotReader, SnapshotWriter};
pub use wal::{WalOp, WalRecord};

use crate::Store;

/// Errors produced by the KAYA persistence layer.
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// Underlying IO error.
    #[error("persistence I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A WAL segment or snapshot file has a bad magic / version header.
    #[error("bad magic or version header in {0}")]
    BadMagic(String),

    /// A WAL record or snapshot chunk failed CRC/checksum validation.
    #[error("corrupt segment: {0}")]
    CorruptSegment(String),

    /// Zstd/LZ4 (de)compression failure.
    #[error("compression error: {0}")]
    Compression(String),

    /// Encoding/decoding error (serde, varint, etc.).
    #[error("encoding error: {0}")]
    Encoding(String),

    /// Persistence is not enabled but an operation was attempted.
    #[error("persistence disabled")]
    Disabled,

    /// Generic internal error.
    #[error("persistence internal error: {0}")]
    Internal(String),
}

/// Convenience alias.
pub type PersistenceResult<T> = Result<T, PersistenceError>;

// ---------------------------------------------------------------------------
// Store integration helpers
// ---------------------------------------------------------------------------

/// Persistence-related methods attached directly to [`Store`].
///
/// These functions are intentionally kept in the `persistence` module rather
/// than `lib.rs` so that the WAL / snapshot surface can grow independently
/// without bloating the core store file.
impl Store {
    /// Write a full snapshot of every shard into `dir`.
    ///
    /// Each shard is serialized to `dir/snap-{shard_id:05}-{seq:010}.snap`
    /// using the supplied [`CompressionAlgo`]. The caller is responsible for
    /// choosing a unique `seq` number. This call is safe to issue while the
    /// store is serving live traffic: shard data is materialized cheaply
    /// (reference-counted [`bytes::Bytes`] clones) before any async I/O.
    pub async fn save_snapshot(
        &self,
        dir: &Path,
        compression: CompressionAlgo,
        zstd_level: i32,
        seq: u64,
    ) -> PersistenceResult<Vec<std::path::PathBuf>> {
        tokio::fs::create_dir_all(dir).await?;
        let mut paths = Vec::with_capacity(self.num_shards());
        for shard_id in 0..self.num_shards() {
            let shard = self.shard_at(shard_id);
            let target = snapshot::snapshot_path(dir, shard_id as u32, seq);
            let path = snapshot::take_snapshot(shard, &target, compression, zstd_level, seq)
                .await?;
            paths.push(path);
        }
        Ok(paths)
    }

    /// Restore a single shard from its snapshot file at `path`.
    ///
    /// After applying the snapshot the shard's data is replaced by the
    /// snapshot contents. Returns the `last_applied_index` embedded in the
    /// snapshot header.
    pub async fn load_snapshot(&self, path: &Path) -> PersistenceResult<RecoveryReport> {
        let last_applied_index = snapshot::apply_snapshot(
            // Shard id is encoded inside the snapshot; we derive it from the
            // header after opening, so we pass shard 0 as a placeholder and
            // allow apply_snapshot to route via the header's shard_id field.
            // In practice callers use recover() on PersistenceManager for full
            // multi-shard recovery.
            self.shard_at(0),
            path,
            PersistenceConfig::default().max_decompressed_size,
        )
        .await?;
        let mut report = RecoveryReport::default();
        report.snapshot_loaded = Some(last_applied_index);
        Ok(report)
    }

    /// Build and run a full persistence recovery (snapshot + WAL replay) for
    /// every shard, using the supplied manager. Returns one report per shard.
    pub async fn recover_with(
        &self,
        manager: &Arc<PersistenceManager>,
    ) -> PersistenceResult<Vec<RecoveryReport>> {
        manager.recover(self).await
    }
}
