//! Configuration for the KAYA persistence layer.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Controls how aggressively the WAL is flushed to stable storage.
///
/// * [`FsyncPolicy::Always`] — fsync after every record (strongest durability,
///   slowest throughput, but amortized via group-commit in the writer).
/// * [`FsyncPolicy::EverySec`] — fsync at most once per second (recommended
///   default, bounded data loss window of <=1s).
/// * [`FsyncPolicy::No`] — never explicitly fsync; rely on the OS page cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsyncPolicy {
    /// fsync after every appended record (group-committed).
    Always,
    /// fsync at most once per second.
    EverySec,
    /// Never fsync explicitly.
    No,
}

impl Default for FsyncPolicy {
    fn default() -> Self {
        Self::EverySec
    }
}

/// Algorithm used to compress snapshot chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgo {
    /// Zstandard compression (recommended).
    Zstd,
    /// LZ4 block compression (lower ratio, faster).
    Lz4,
    /// No compression.
    None,
}

impl Default for CompressionAlgo {
    fn default() -> Self {
        Self::Zstd
    }
}

/// Configuration for the [`crate::persistence::PersistenceManager`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Whether the persistence layer is active.
    pub enabled: bool,

    /// Root data directory (WAL + snapshots live inside subdirs here).
    pub data_dir: PathBuf,

    /// Fsync policy for the WAL.
    pub fsync_policy: FsyncPolicy,

    /// Max WAL segment size in bytes before rolling to the next segment.
    pub segment_size_bytes: u64,

    /// Interval in seconds between periodic snapshots (0 disables).
    pub snapshot_interval_secs: u64,

    /// Number of snapshots to keep per shard (older ones are deleted).
    pub snapshot_retention: usize,

    /// Compression algorithm for snapshot chunks.
    pub compression: CompressionAlgo,

    /// Zstd compression level (1..=22). Ignored for non-Zstd.
    pub zstd_level: i32,

    /// Maximum allowed decompressed size per snapshot chunk in bytes.
    ///
    /// WHY: prevents zip-bomb payloads from exhausting process memory when
    /// loading a snapshot (SecFinding-SNAPSHOT-ZIPBOMB). Default: 256 MiB.
    #[serde(default = "PersistenceConfig::default_max_decompressed_size")]
    pub max_decompressed_size: usize,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            data_dir: PathBuf::from("./data/kaya"),
            fsync_policy: FsyncPolicy::EverySec,
            segment_size_bytes: 64 * 1024 * 1024,
            snapshot_interval_secs: 3600,
            snapshot_retention: 7,
            compression: CompressionAlgo::Zstd,
            zstd_level: 3,
            max_decompressed_size: Self::default_max_decompressed_size(),
        }
    }
}

impl PersistenceConfig {
    fn default_max_decompressed_size() -> usize {
        256 * 1024 * 1024 // 256 MiB
    }

    /// Absolute path to the WAL directory.
    pub fn wal_dir(&self) -> PathBuf {
        self.data_dir.join("wal")
    }

    /// Absolute path to the snapshots directory.
    pub fn snapshots_dir(&self) -> PathBuf {
        self.data_dir.join("snapshots")
    }
}
