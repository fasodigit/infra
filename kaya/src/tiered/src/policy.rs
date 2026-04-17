//! Migration policies that govern when keys are moved from hot to cold tier.
//!
//! # Policies
//!
//! | Policy      | Trigger                                                    |
//! |-------------|-------------------------------------------------------------|
//! | `LfuCold`   | Memory pressure OR LFU idle time exceeds `min_idle_secs`   |
//! | `TtlCold`   | Key has not been accessed for longer than `hot_ttl_secs`   |
//! | `Manual`    | No automatic migration; operator-driven only               |

use std::time::Duration;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MigrationPolicy
// ---------------------------------------------------------------------------

/// Strategy used by [`TieredMigrator`](crate::migrator::TieredMigrator) to
/// decide which keys to demote to cold storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MigrationPolicy {
    /// Least-Frequently-Used cold demotion.
    ///
    /// Keys whose last access was more than `min_idle_secs` ago are eligible
    /// for demotion. When hot-tier memory exceeds `max_hot_mem_bytes`, the
    /// coldest `migrate_ratio` fraction of eligible keys is demoted.
    LfuCold {
        /// Minimum idle time before a key is a demotion candidate (seconds).
        min_idle_secs: u64,
        /// Hot-tier memory threshold that triggers migration (bytes).
        max_hot_mem_bytes: u64,
        /// Fraction of eligible keys to demote per tick (0.0–1.0).
        migrate_ratio: f32,
    },

    /// TTL-based cold demotion.
    ///
    /// Keys not accessed within the last `hot_ttl_secs` are candidates for
    /// demotion regardless of memory pressure.
    TtlCold {
        /// Keys older than this threshold (seconds) are moved cold.
        hot_ttl_secs: u64,
    },

    /// No automatic migration. Keys are demoted only via explicit
    /// `KAYA.TIERED.DEMOTE` or [`TieredStore::force_demote`](crate::store::TieredStore).
    Manual,
}

impl MigrationPolicy {
    /// Returns `true` if this policy performs automatic background migration.
    pub fn is_auto(&self) -> bool {
        !matches!(self, MigrationPolicy::Manual)
    }

    /// Returns the minimum idle duration for LFU policy, or `None`.
    pub fn lfu_idle_duration(&self) -> Option<Duration> {
        match self {
            MigrationPolicy::LfuCold { min_idle_secs, .. } => {
                Some(Duration::from_secs(*min_idle_secs))
            }
            _ => None,
        }
    }

    /// Returns the hot TTL duration for TTL policy, or `None`.
    pub fn ttl_hot_duration(&self) -> Option<Duration> {
        match self {
            MigrationPolicy::TtlCold { hot_ttl_secs } => {
                Some(Duration::from_secs(*hot_ttl_secs))
            }
            _ => None,
        }
    }

    /// Returns the max hot-tier memory bytes threshold (LFU policy only).
    pub fn max_hot_mem_bytes(&self) -> Option<u64> {
        match self {
            MigrationPolicy::LfuCold { max_hot_mem_bytes, .. } => Some(*max_hot_mem_bytes),
            _ => None,
        }
    }

    /// Returns the migration ratio (LFU policy only).
    pub fn migrate_ratio(&self) -> f32 {
        match self {
            MigrationPolicy::LfuCold { migrate_ratio, .. } => *migrate_ratio,
            _ => 1.0,
        }
    }
}

impl Default for MigrationPolicy {
    fn default() -> Self {
        MigrationPolicy::LfuCold {
            min_idle_secs: 60,
            max_hot_mem_bytes: 512 * 1024 * 1024, // 512 MB
            migrate_ratio: 0.1,
        }
    }
}
