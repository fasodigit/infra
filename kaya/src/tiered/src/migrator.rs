//! Background migration worker for tiered storage.
//!
//! `TieredMigrator` runs as a Tokio task and periodically evaluates which
//! hot-tier keys should be demoted to the cold backend, using the configured
//! [`MigrationPolicy`].
//!
//! # Location tracking
//!
//! Every key in the system has one of three locations:
//!
//! ```text
//! Hot        — key lives only in the hot RAM shard.
//! HotDirty   — key was written/updated since last migration tick; it will
//!              not be considered for demotion until it is flushed.
//! Cold       — key lives only in the cold backend (fjall / MemBackend).
//! ```
//!
//! On `set`, the location is marked `HotDirty`.
//! On `get` of a cold key, the key is promoted back to `Hot`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::{debug, info, warn};

use crate::backend::ColdBackend;
use crate::error::TieredError;
use crate::policy::MigrationPolicy;

// ---------------------------------------------------------------------------
// Location
// ---------------------------------------------------------------------------

/// Where a key currently resides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// Key is in the hot RAM tier and is clean (no pending write).
    Hot,
    /// Key was recently written and is ineligible for immediate demotion.
    HotDirty,
    /// Key has been demoted to the cold backend.
    Cold,
}

// ---------------------------------------------------------------------------
// AccessRecord
// ---------------------------------------------------------------------------

/// Per-key metadata tracked by the migrator for policy decisions.
#[derive(Debug, Clone)]
pub struct AccessRecord {
    /// Last time this key was read or written.
    pub last_accessed: Instant,
    /// Total number of accesses since insertion.
    pub access_count: u64,
    /// Current tier location.
    pub location: Location,
}

impl AccessRecord {
    pub fn new_hot_dirty() -> Self {
        Self {
            last_accessed: Instant::now(),
            access_count: 1,
            location: Location::HotDirty,
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }

    pub fn idle_duration(&self) -> Duration {
        self.last_accessed.elapsed()
    }
}

// ---------------------------------------------------------------------------
// TieredMigrator
// ---------------------------------------------------------------------------

/// Background Tokio task that migrates keys between hot and cold tiers.
pub struct TieredMigrator {
    /// Location + access metadata per key.
    pub(crate) locations: Arc<DashMap<Vec<u8>, AccessRecord, ahash::RandomState>>,
    cold: Arc<dyn ColdBackend>,
    hot: Arc<kaya_store::Store>,
    policy: Arc<parking_lot::RwLock<MigrationPolicy>>,

    /// Tick interval.
    tick: Duration,

    /// Running count of demotions performed.
    pub(crate) migrations_total: Arc<std::sync::atomic::AtomicU64>,
    /// Running count of promotions performed.
    pub(crate) promotions_total: Arc<std::sync::atomic::AtomicU64>,
}

impl TieredMigrator {
    pub fn new(
        hot: Arc<kaya_store::Store>,
        cold: Arc<dyn ColdBackend>,
        policy: Arc<parking_lot::RwLock<MigrationPolicy>>,
        tick: Duration,
    ) -> Self {
        Self {
            locations: Arc::new(DashMap::with_hasher(
                ahash::RandomState::with_seeds(42, 43, 44, 45),
            )),
            cold,
            hot,
            policy,
            tick,
            migrations_total: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            promotions_total: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Mark a key as `HotDirty` after a write. Called by `TieredStore::set`.
    pub fn on_write(&self, key: &[u8]) {
        let mut record = self.locations
            .entry(key.to_vec())
            .or_insert_with(AccessRecord::new_hot_dirty);
        record.location = Location::HotDirty;
        record.touch();
    }

    /// Record a read access. If the key was Cold, promote it.
    /// Returns `true` if a promotion was triggered (caller must load from cold).
    pub async fn on_read(&self, key: &[u8]) -> Result<bool, TieredError> {
        if let Some(mut record) = self.locations.get_mut(key) {
            record.touch();
            if record.location == Location::Cold {
                // Promote: read from cold, write to hot, update location.
                drop(record); // release the dashmap lock before await
                self.promote(key).await?;
                return Ok(true);
            }
            return Ok(false);
        }
        // Unknown key: might be brand new or not tracked yet.
        Ok(false)
    }

    /// Promote a cold key back to hot tier.
    pub async fn promote(&self, key: &[u8]) -> Result<(), TieredError> {
        if let Some(value) = self.cold.get(key).await? {
            self.hot.set(key, &value, None)?;
            self.cold.delete(key).await?;
            if let Some(mut record) = self.locations.get_mut(key) {
                record.location = Location::Hot;
                record.touch();
            }
            self.promotions_total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            debug!("promoted cold key ({} bytes) to hot tier", key.len());
        }
        Ok(())
    }

    /// Demote a hot key to cold tier.
    pub async fn demote(&self, key: &[u8]) -> Result<(), TieredError> {
        if let Ok(Some(value)) = self.hot.get(key) {
            self.cold.set(key, &value).await?;
            self.hot.del(&[key]);
            if let Some(mut record) = self.locations.get_mut(key) {
                record.location = Location::Cold;
            }
            self.migrations_total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            debug!("demoted hot key ({} bytes) to cold tier", key.len());
        }
        Ok(())
    }

    /// One migration tick: evaluate candidates and demote according to policy.
    ///
    /// At the start of each tick, keys marked `HotDirty` are transitioned to
    /// `Hot`. This represents one tick's "grace period" after a write before a
    /// key becomes eligible for demotion.
    pub async fn tick(&self) -> Result<usize, TieredError> {
        // Transition HotDirty → Hot so they are eligible for demotion.
        for mut rec in self.locations.iter_mut() {
            if rec.location == Location::HotDirty {
                rec.location = Location::Hot;
            }
        }

        let policy = self.policy.read().clone();
        let demoted = match &policy {
            MigrationPolicy::Manual => 0,
            MigrationPolicy::LfuCold { min_idle_secs, max_hot_mem_bytes, migrate_ratio } => {
                self.tick_lfu(*min_idle_secs, *max_hot_mem_bytes, *migrate_ratio).await?
            }
            MigrationPolicy::TtlCold { hot_ttl_secs } => {
                self.tick_ttl(*hot_ttl_secs).await?
            }
        };
        if demoted > 0 {
            info!("migration tick: demoted {} keys to cold tier", demoted);
        }
        Ok(demoted)
    }

    async fn tick_lfu(
        &self,
        min_idle_secs: u64,
        max_hot_mem_bytes: u64,
        migrate_ratio: f32,
    ) -> Result<usize, TieredError> {
        let idle_threshold = Duration::from_secs(min_idle_secs);

        // Collect hot candidates sorted by (access_count ASC, idle DESC) —
        // i.e., coldest first.
        let mut candidates: Vec<(Vec<u8>, u64, Duration)> = self
            .locations
            .iter()
            .filter(|r| {
                r.location == Location::Hot && r.idle_duration() >= idle_threshold
            })
            .map(|r| (r.key().clone(), r.access_count, r.idle_duration()))
            .collect();

        if candidates.is_empty() {
            return Ok(0);
        }

        // Sort: least frequent first; break ties by most idle.
        candidates.sort_unstable_by(|a, b| {
            a.1.cmp(&b.1).then(b.2.cmp(&a.2))
        });

        // Approximate hot-tier byte usage via key count * avg entry size heuristic.
        let hot_key_count = candidates.len() as u64;
        // Only migrate if we are over the memory threshold OR have eligible keys.
        let estimated_hot_bytes = self.hot.key_count() as u64 * 256; // rough estimate

        if estimated_hot_bytes < max_hot_mem_bytes && hot_key_count == 0 {
            return Ok(0);
        }

        let count = ((candidates.len() as f32 * migrate_ratio).ceil() as usize).max(1);
        let to_demote: Vec<Vec<u8>> = candidates.into_iter().take(count).map(|(k, _, _)| k).collect();

        let mut demoted = 0;
        for key in &to_demote {
            match self.demote(key).await {
                Ok(()) => demoted += 1,
                Err(e) => warn!("failed to demote key: {}", e),
            }
        }
        Ok(demoted)
    }

    async fn tick_ttl(&self, hot_ttl_secs: u64) -> Result<usize, TieredError> {
        let threshold = Duration::from_secs(hot_ttl_secs);

        let candidates: Vec<Vec<u8>> = self
            .locations
            .iter()
            .filter(|r| r.location == Location::Hot && r.idle_duration() >= threshold)
            .map(|r| r.key().clone())
            .collect();

        let mut demoted = 0;
        for key in &candidates {
            match self.demote(key).await {
                Ok(()) => demoted += 1,
                Err(e) => warn!("failed to demote key during TTL tick: {}", e),
            }
        }
        Ok(demoted)
    }

    /// Count of keys currently in the cold tier.
    pub fn cold_key_count(&self) -> u64 {
        self.locations
            .iter()
            .filter(|r| r.location == Location::Cold)
            .count() as u64
    }

    /// Count of keys currently in the hot tier (Hot or HotDirty).
    pub fn hot_key_count(&self) -> u64 {
        self.locations
            .iter()
            .filter(|r| r.location != Location::Cold)
            .count() as u64
    }

    /// Run the background migrator loop indefinitely.
    pub async fn run_loop(self: Arc<Self>) {
        info!("tiered migrator started (tick={:?})", self.tick);
        loop {
            tokio::time::sleep(self.tick).await;
            if let Err(e) = self.tick().await {
                warn!("migration tick error: {}", e);
            }
        }
    }
}
