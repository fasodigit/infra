//! `TieredStore`: unified hot+cold key-value store.
//!
//! `TieredStore` composes:
//! - A **hot tier** (`Arc<kaya_store::Store>`) — sharded in-memory DashMap.
//! - A **cold tier** (`Arc<dyn ColdBackend>`) — fjall LSM-tree on NVMe.
//! - A **migrator** (`TieredMigrator`) — background Tokio task applying the
//!   configured [`MigrationPolicy`].
//!
//! ## Read path
//!
//! 1. Check hot tier. If found, return immediately.
//! 2. Check migrator location table. If key is `Cold`, fetch from cold backend
//!    and promote back to hot (lazy promotion on read).
//! 3. If key is not in the location table at all, check cold backend directly
//!    (handles keys inserted before the migrator was active).
//!
//! ## Write path
//!
//! Always writes to hot tier first, then marks the key `HotDirty` in the
//! location table. The background migrator will eventually demote it if it
//! becomes cold according to the policy.
//!
//! ## Delete path
//!
//! Delete is issued to both tiers; returns total count of deletions.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use parking_lot::RwLock;
use tokio::task::JoinHandle;
use tracing::info;

use crate::backend::ColdBackend;
use crate::error::TieredError;
use crate::migrator::{Location, TieredMigrator};
use crate::policy::MigrationPolicy;

// ---------------------------------------------------------------------------
// TieredStats
// ---------------------------------------------------------------------------

/// Operational statistics snapshot for the tiered store.
#[derive(Debug, Clone, Default)]
pub struct TieredStats {
    /// Number of keys in the hot tier.
    pub hot_keys: u64,
    /// Number of keys in the cold tier.
    pub cold_keys: u64,
    /// Approximate byte size of hot tier.
    pub hot_bytes: u64,
    /// Approximate byte size of cold tier.
    pub cold_bytes: u64,
    /// Total cumulative demotions (hot → cold).
    pub migrations_total: u64,
    /// Total cumulative promotions (cold → hot).
    pub promotions_total: u64,
}

// ---------------------------------------------------------------------------
// TieredStore
// ---------------------------------------------------------------------------

/// The main tiered storage facade.
pub struct TieredStore {
    /// Hot in-memory tier.
    pub(crate) hot: Arc<kaya_store::Store>,
    /// Cold NVMe / disk tier.
    pub(crate) cold: Arc<dyn ColdBackend>,
    /// Background migration task controller.
    pub(crate) migrator: Arc<TieredMigrator>,
    /// Active migration policy (runtime-swappable).
    pub(crate) policy: Arc<RwLock<MigrationPolicy>>,
    /// Background migrator tick interval.
    pub(crate) tick_interval: Duration,
}

impl TieredStore {
    /// Construct a new `TieredStore`.
    ///
    /// Call [`start_migrator`](Self::start_migrator) on an `Arc<Self>` to
    /// launch background migration.
    pub fn new(
        hot: Arc<kaya_store::Store>,
        cold: Arc<dyn ColdBackend>,
        policy: MigrationPolicy,
    ) -> Self {
        Self::with_tick(hot, cold, policy, Duration::from_secs(1))
    }

    /// Construct with a custom tick interval (useful in tests).
    pub fn with_tick(
        hot: Arc<kaya_store::Store>,
        cold: Arc<dyn ColdBackend>,
        policy: MigrationPolicy,
        tick_interval: Duration,
    ) -> Self {
        let policy_lock = Arc::new(RwLock::new(policy));
        let migrator = Arc::new(TieredMigrator::new(
            Arc::clone(&hot),
            Arc::clone(&cold),
            Arc::clone(&policy_lock),
            tick_interval,
        ));
        Self {
            hot,
            cold,
            migrator,
            policy: policy_lock,
            tick_interval,
        }
    }

    // -- Read -----------------------------------------------------------------

    /// GET: retrieve a value. Hot miss falls through to cold with auto-promote.
    pub async fn get(&self, key: &[u8]) -> Result<Option<Bytes>, TieredError> {
        // 1. Try hot tier.
        if let Ok(Some(value)) = self.hot.get(key) {
            // Update access record.
            if let Some(mut rec) = self.migrator.locations.get_mut(key) {
                rec.touch();
                if rec.location == Location::Cold {
                    rec.location = Location::Hot;
                }
            }
            return Ok(Some(value));
        }

        // 2. Consult location table.
        let is_cold = self.migrator.locations
            .get(key)
            .map(|r| r.location == Location::Cold)
            .unwrap_or(false);

        if is_cold {
            // Promote: cold → hot.
            if let Some(raw) = self.cold.get(key).await? {
                let bytes = Bytes::from(raw.clone());
                self.hot.set(key, &raw, None)?;
                self.cold.delete(key).await?;
                if let Some(mut rec) = self.migrator.locations.get_mut(key) {
                    rec.location = Location::Hot;
                    rec.touch();
                }
                self.migrator.promotions_total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(Some(bytes));
            }
            return Ok(None);
        }

        // 3. Key not in location table — check cold backend directly.
        if let Some(raw) = self.cold.get(key).await? {
            let bytes = Bytes::from(raw.clone());
            // Bring it back to hot and register it.
            self.hot.set(key, &raw, None)?;
            self.cold.delete(key).await?;
            self.migrator.on_write(key);
            if let Some(mut rec) = self.migrator.locations.get_mut(key) {
                rec.location = Location::Hot;
            }
            self.migrator.promotions_total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(Some(bytes));
        }

        Ok(None)
    }

    // -- Write ----------------------------------------------------------------

    /// SET: always writes to hot tier; marks key as `HotDirty`.
    pub async fn set(&self, key: &[u8], value: &[u8], ttl: Option<u64>) -> Result<(), TieredError> {
        // If key was cold, delete from cold first.
        let was_cold = self.migrator.locations
            .get(key)
            .map(|r| r.location == Location::Cold)
            .unwrap_or(false);

        if was_cold {
            self.cold.delete(key).await?;
        }

        self.hot.set(key, value, ttl)?;
        self.migrator.on_write(key);
        Ok(())
    }

    // -- Delete ---------------------------------------------------------------

    /// DEL: delete from both tiers. Returns total keys deleted.
    pub async fn del(&self, key: &[u8]) -> u64 {
        let hot_deleted = self.hot.del(&[key]);

        let cold_deleted = if let Some(rec) = self.migrator.locations.get(key) {
            if rec.location == Location::Cold {
                drop(rec);
                self.cold.delete(key).await.unwrap_or(false) as u64
            } else {
                drop(rec);
                0
            }
        } else {
            // Try cold unconditionally if we have no location record.
            self.cold.delete(key).await.unwrap_or(false) as u64
        };

        self.migrator.locations.remove(key);
        hot_deleted + cold_deleted
    }

    // -- Force operations -----------------------------------------------------

    /// Force-promote a key from cold to hot tier.
    pub async fn force_promote(&self, key: &[u8]) -> Result<(), TieredError> {
        // First try via location table.
        let is_cold = self.migrator.locations
            .get(key)
            .map(|r| r.location == Location::Cold)
            .unwrap_or(false);

        if is_cold || self.cold.exists(key).await? {
            self.migrator.promote(key).await?;
        }
        Ok(())
    }

    /// Force-demote a key from hot to cold tier.
    pub async fn force_demote(&self, key: &[u8]) -> Result<(), TieredError> {
        self.migrator.demote(key).await
    }

    // -- Stats ----------------------------------------------------------------

    /// Return a statistics snapshot.
    pub async fn stats(&self) -> TieredStats {
        let hot_keys = self.migrator.hot_key_count();
        let cold_keys = self.migrator.cold_key_count();
        let hot_bytes = (self.hot.key_count() as u64).saturating_mul(256); // heuristic
        let cold_bytes = self.cold.size_bytes().await;
        let migrations_total = self.migrator.migrations_total
            .load(std::sync::atomic::Ordering::Relaxed);
        let promotions_total = self.migrator.promotions_total
            .load(std::sync::atomic::Ordering::Relaxed);

        TieredStats {
            hot_keys,
            cold_keys,
            hot_bytes,
            cold_bytes,
            migrations_total,
            promotions_total,
        }
    }

    // -- Policy ---------------------------------------------------------------

    /// Get a clone of the current policy.
    pub fn get_policy(&self) -> MigrationPolicy {
        self.policy.read().clone()
    }

    /// Replace the active migration policy at runtime.
    pub fn set_policy(&self, new_policy: MigrationPolicy) {
        *self.policy.write() = new_policy;
    }

    // -- Location query -------------------------------------------------------

    /// Query the current tier location of a key.
    pub fn location(&self, key: &[u8]) -> Option<Location> {
        self.migrator.locations.get(key).map(|r| r.location)
    }

    /// LFU access count for a key.
    pub fn access_count(&self, key: &[u8]) -> Option<u64> {
        self.migrator.locations.get(key).map(|r| r.access_count)
    }

    /// Idle time (seconds since last access) for a key.
    pub fn idle_secs(&self, key: &[u8]) -> Option<u64> {
        self.migrator.locations.get(key).map(|r| r.idle_duration().as_secs())
    }

    // -- Background task ------------------------------------------------------

    /// Spawn the background migration loop as a Tokio task.
    ///
    /// Returns the `JoinHandle`; dropping it cancels the background loop.
    pub fn start_migrator(self: Arc<Self>) -> JoinHandle<()> {
        let migrator = Arc::clone(&self.migrator);
        info!("starting tiered migrator background task");
        tokio::spawn(migrator.run_loop())
    }

    /// Run a single migration tick synchronously (useful in tests and benchmarks).
    pub async fn tick_once(&self) -> Result<usize, TieredError> {
        self.migrator.tick().await
    }

    /// Access the underlying hot-tier store (for diagnostics / tests).
    pub fn hot_store(&self) -> &kaya_store::Store {
        &self.hot
    }
}
