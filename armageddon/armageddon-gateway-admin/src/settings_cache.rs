// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # AdminSettingsCache — hot-path in-memory cache for admin + TERROIR settings
//!
//! ## Purpose
//!
//! Certain security and feature parameters are consulted on **every** request
//! through ARMAGEDDON.  Fetching them from the DB on every request is too
//! expensive.
//!
//! `AdminSettingsCache` holds an `Arc<RwLock<HashMap<String, serde_json::Value>>>`
//! that is populated at startup and updated whenever either:
//!
//! - the Redpanda topic `admin.settings.changed` delivers a message (admin
//!   keys like `otp.*`, `device_trust.*`, `session.*`), or
//! - the Redpanda topic `terroir.settings.changed` delivers a message (keys
//!   prefixed `terroir.*`).
//!
//! Both topics write into the **same** underlying `HashMap` under different
//! key namespaces, keeping the cache structure simple.
//!
//! ## TERROIR settings (P0.H)
//!
//! | Key | Default | Notes |
//! |-----|---------|-------|
//! | `terroir.eudr.cut_off_date` | `"2020-12-31"` | Earliest deforestation-free date per EUDR Art. 2 |
//! | `terroir.eudr.cache_ttl_days` | `30` | Hansen GFC tile cache TTL |
//! | `terroir.ussd.simulator_enabled` | `true` | Enables loopback USSD simulator (P0-P2); set false in P3 |
//! | `terroir.session.agent_offline_max_days` | `14` | Max days before an agent JWT is force-revoked offline |
//!
//! ## Cache invalidation
//!
//! The consumer loop (`start_consumer`) is started with `tokio::spawn` in the
//! background.  On each message it:
//! 1. Deserialises the payload (`AdminSettingsChangedEvent` — same shape for
//!    both topics).
//! 2. Acquires a **write** lock (briefly held — no `.await` across the lock).
//! 3. Updates the affected key.
//! 4. Releases the lock.
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | Redpanda unreachable at startup | Warning logged; cache starts empty; settings fall back to hard-coded defaults |
//! | Redpanda message decode error | Error logged; message skipped (cache unchanged) |
//! | Redpanda partition re-balance | `rdkafka` handles transparently; consumer loop continues |
//! | Write lock poisoned (panic elsewhere) | `unwrap_or_else(|e| e.into_inner())` recovers |
//!
//! ## Lock discipline
//!
//! Following ARMAGEDDON invariants, the write lock is **never held across
//! an `.await` point**.  The Kafka poll itself does not hold the lock.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "rdkafka-consumer")]
use rdkafka::config::ClientConfig;
#[cfg(feature = "rdkafka-consumer")]
use rdkafka::consumer::{Consumer, StreamConsumer};
#[cfg(feature = "rdkafka-consumer")]
use rdkafka::Message;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::metrics::settings_cache_hits_total;

// ── types ─────────────────────────────────────────────────────────────────────

/// Shape of the `admin.settings.changed` Redpanda message value.
///
/// Produced by `AdminSettingsService.java` whenever a setting is persisted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminSettingsChangedEvent {
    /// Setting key, e.g. `"otp.lifetime_seconds"`.
    pub key: String,
    /// New value as a JSON value (matches the `value JSONB` column).
    pub new_value: serde_json::Value,
    /// Previous value (for audit; not used by the cache).
    #[serde(default)]
    pub old_value: Option<serde_json::Value>,
    /// Actor who made the change (for tracing; not used by the cache).
    #[serde(default)]
    pub actor_id: Option<String>,
    /// Trace ID for correlation with Jaeger.
    #[serde(default)]
    pub trace_id: Option<String>,
}

/// The shared admin settings cache.
///
/// Clone-safe (inner `Arc`); pass an `Arc<AdminSettingsCache>` to every filter
/// that needs to read settings on the hot path.
#[derive(Clone, Debug)]
pub struct AdminSettingsCache {
    inner: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl Default for AdminSettingsCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminSettingsCache {
    /// Create a new empty cache.
    ///
    /// Call `seed` to pre-populate with known defaults before the Redpanda
    /// consumer catches up, so the OTP rate-limit filter always has a value.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Pre-populate the cache with default values so the hot path never falls
    /// back to an unconfigured state between startup and the first Redpanda
    /// message.
    ///
    /// These match the defaults defined in GAP-ANALYSIS §2 V7 seed data.
    pub async fn seed_defaults(&self) {
        let mut map = self.inner.write().await;
        // OTP policy defaults
        map.insert("otp.lifetime_seconds".to_string(), serde_json::json!(300));
        map.insert("otp.max_attempts".to_string(), serde_json::json!(3));
        map.insert(
            "otp.lock_duration_seconds".to_string(),
            serde_json::json!(900),
        );
        map.insert(
            "otp.rate_limit_per_user_5min".to_string(),
            serde_json::json!(3),
        );
        map.insert("otp.length".to_string(), serde_json::json!(8));
        // Device trust defaults
        map.insert("device_trust.ttl_days".to_string(), serde_json::json!(30));
        map.insert(
            "device_trust.max_per_user".to_string(),
            serde_json::json!(5),
        );
        // Session defaults
        map.insert(
            "session.max_concurrent_per_user".to_string(),
            serde_json::json!(3),
        );
        map.insert(
            "session.idle_timeout_minutes".to_string(),
            serde_json::json!(480),
        );
    }

    /// Pre-populate the cache with TERROIR-specific default values.
    ///
    /// Call this alongside [`seed_defaults`] at gateway startup so the filters
    /// always see a configured value even before the `terroir.settings.changed`
    /// topic is consumed.
    ///
    /// | Key | Default | Source |
    /// |-----|---------|--------|
    /// | `terroir.eudr.cut_off_date` | `"2020-12-31"` | EUDR Art. 2 regulation |
    /// | `terroir.eudr.cache_ttl_days` | `30` | Ops decision (tile freshness) |
    /// | `terroir.ussd.simulator_enabled` | `true` | ADR-003 (P0-P2 loopback sim) |
    /// | `terroir.session.agent_offline_max_days` | `14` | Q2 / Kratos JWT sliding window |
    pub async fn seed_terroir_defaults(&self) {
        let mut map = self.inner.write().await;
        // EUDR cut-off and cache TTL
        map.insert(
            "terroir.eudr.cut_off_date".to_string(),
            serde_json::json!("2020-12-31"),
        );
        map.insert(
            "terroir.eudr.cache_ttl_days".to_string(),
            serde_json::json!(30),
        );
        // USSD simulator flag (true in P0-P2, switched to false in P3)
        map.insert(
            "terroir.ussd.simulator_enabled".to_string(),
            serde_json::json!(true),
        );
        // Agent offline session max duration
        map.insert(
            "terroir.session.agent_offline_max_days".to_string(),
            serde_json::json!(14),
        );
    }

    /// Convenience: read a `bool` setting with a fallback.
    pub async fn get_bool(&self, key: &str, default: bool) -> bool {
        self.get(key)
            .await
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    /// Convenience: read a `String` setting with a fallback.
    pub async fn get_str(&self, key: &str, default: &str) -> String {
        self.get(key)
            .await
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| default.to_owned())
    }

    /// Read a setting value, returning `None` if the key is absent.
    ///
    /// Increments `armageddon_admin_settings_cache_hits_total` on every call
    /// (regardless of hit/miss) to let operators distinguish "cache empty" from
    /// "key unknown".
    pub async fn get(&self, key: &str) -> Option<serde_json::Value> {
        settings_cache_hits_total().inc();
        let map = self.inner.read().await;
        map.get(key).cloned()
    }

    /// Convenience: read an `i64` setting with a fallback.
    pub async fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.get(key)
            .await
            .and_then(|v| v.as_i64())
            .unwrap_or(default)
    }

    /// Update a single key.  Called by the Redpanda consumer loop.
    ///
    /// Lock is acquired and released synchronously (no `.await` while held).
    pub async fn update(&self, key: String, value: serde_json::Value) {
        let mut map = self.inner.write().await;
        map.insert(key, value);
        // Lock released here — before any .await.
    }
}

// ── Redpanda consumer ─────────────────────────────────────────────────────────

/// Configuration for the Redpanda settings-change consumer.
///
/// Always available (not feature-gated) so the main binary can construct and
/// pass this config without conditional compilation.  The consumer itself is
/// only available when the `"rdkafka-consumer"` feature is enabled.
#[derive(Debug, Clone)]
pub struct SettingsConsumerConfig {
    /// Comma-separated broker addresses, e.g. `"redpanda:9092"`.
    pub brokers: String,
    /// Kafka consumer group ID.  Use a stable, service-specific ID so the
    /// consumer resumes from its last committed offset after restarts.
    pub group_id: String,
    /// Topic to consume.  Default: `"admin.settings.changed"`.
    pub topic: String,
    /// Reconnect back-off when the broker is temporarily unavailable.
    pub reconnect_backoff: Duration,
}

impl Default for SettingsConsumerConfig {
    fn default() -> Self {
        Self {
            brokers: "redpanda:9092".to_string(),
            group_id: "armageddon-settings-consumer".to_string(),
            topic: "admin.settings.changed".to_string(),
            reconnect_backoff: Duration::from_secs(5),
        }
    }
}

/// Spawn the Redpanda consumer loop as a background `tokio::task`.
#[cfg(feature = "rdkafka-consumer")]
///
/// The task runs for the lifetime of the process and updates `cache` whenever
/// it receives an `AdminSettingsChangedEvent`.
///
/// # Cancellation
///
/// The task checks `shutdown_rx` via `tokio::select!` on each loop iteration
/// so it exits cleanly when the gateway shuts down.
pub fn start_consumer(
    cfg: SettingsConsumerConfig,
    cache: Arc<AdminSettingsCache>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    tokio::spawn(async move {
        loop {
            // Re-create the consumer on every (re)connection attempt.
            // This ensures we pick up new metadata after a broker restart.
            let consumer: StreamConsumer = match build_consumer(&cfg) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        topic = %cfg.topic,
                        err = %e,
                        "admin settings consumer: failed to create rdkafka consumer, retrying in {:?}",
                        cfg.reconnect_backoff
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(cfg.reconnect_backoff) => continue,
                        _ = shutdown_rx.recv() => {
                            tracing::info!("admin settings consumer: shutdown signal received");
                            return;
                        }
                    }
                }
            };

            if let Err(e) = consumer.subscribe(&[cfg.topic.as_str()]) {
                tracing::warn!(topic = %cfg.topic, err = %e, "admin settings consumer: subscribe failed");
                tokio::time::sleep(cfg.reconnect_backoff).await;
                continue;
            }

            tracing::info!(topic = %cfg.topic, group_id = %cfg.group_id, "admin settings consumer: subscribed");

            // Poll loop — one message at a time.
            loop {
                tokio::select! {
                    msg = consumer.recv() => {
                        match msg {
                            Err(e) => {
                                tracing::error!(err = %e, "admin settings consumer: receive error, reconnecting");
                                break; // outer loop will recreate the consumer
                            }
                            Ok(m) => {
                                process_message(&m, &cache).await;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("admin settings consumer: shutdown signal received");
                        return;
                    }
                }
            }

            // Back-off before reconnect.
            tokio::select! {
                _ = tokio::time::sleep(cfg.reconnect_backoff) => {}
                _ = shutdown_rx.recv() => {
                    tracing::info!("admin settings consumer: shutdown signal received during backoff");
                    return;
                }
            }
        }
    });
}

/// Process a single Kafka message: decode and update the cache.
#[cfg(feature = "rdkafka-consumer")]
async fn process_message<M>(msg: &M, cache: &AdminSettingsCache)
where
    M: rdkafka::Message,
{
    let payload = match msg.payload() {
        Some(p) => p,
        None => {
            tracing::warn!("admin settings consumer: received message with empty payload");
            return;
        }
    };

    match serde_json::from_slice::<AdminSettingsChangedEvent>(payload) {
        Ok(event) => {
            tracing::info!(
                key = %event.key,
                trace_id = ?event.trace_id,
                actor_id = ?event.actor_id,
                "admin settings cache: invalidating key"
            );
            cache.update(event.key, event.new_value).await;
        }
        Err(e) => {
            tracing::error!(
                err = %e,
                "admin settings consumer: failed to decode AdminSettingsChangedEvent, skipping"
            );
        }
    }
}

/// Build a `StreamConsumer` from the consumer config.
#[cfg(feature = "rdkafka-consumer")]
fn build_consumer(cfg: &SettingsConsumerConfig) -> anyhow::Result<StreamConsumer> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.brokers)
        .set("group.id", &cfg.group_id)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        // Reconnect automatically on broker failures.
        .set("reconnect.backoff.ms", "1000")
        .set("reconnect.backoff.max.ms", "10000")
        .set("session.timeout.ms", "30000")
        .create()?;
    Ok(consumer)
}

// ── TERROIR settings consumer ─────────────────────────────────────────────────

/// Configuration for the TERROIR Redpanda settings-change consumer.
///
/// Subscribes to `terroir.settings.changed` and writes `terroir.*` keys into
/// the **same** `AdminSettingsCache` used for admin settings.  This avoids
/// introducing a second cache struct while keeping the topic boundary clean.
///
/// If stream P0.E has not yet created the `terroir.settings.changed` topic,
/// operators can point this consumer at `admin.settings.changed` with a key
/// prefix filter — the `process_message` function already handles `terroir.*`
/// keys transparently since it writes the key verbatim.
#[derive(Debug, Clone)]
pub struct TerroirSettingsConsumerConfig {
    /// Comma-separated broker addresses, e.g. `"redpanda:9092"`.
    pub brokers: String,
    /// Kafka consumer group ID.  Must be distinct from the admin consumer group.
    pub group_id: String,
    /// Topic to consume.  Default: `"terroir.settings.changed"`.
    pub topic: String,
    /// Reconnect back-off when the broker is temporarily unavailable.
    pub reconnect_backoff: Duration,
}

impl Default for TerroirSettingsConsumerConfig {
    fn default() -> Self {
        Self {
            brokers: "redpanda:9092".to_string(),
            group_id: "armageddon-terroir-settings-consumer".to_string(),
            topic: "terroir.settings.changed".to_string(),
            reconnect_backoff: Duration::from_secs(5),
        }
    }
}

/// Spawn the TERROIR Redpanda settings consumer as a background `tokio::task`.
///
/// The task runs for the lifetime of the process and updates the shared
/// `AdminSettingsCache` whenever it receives an `AdminSettingsChangedEvent`
/// (same payload shape as the admin topic).
///
/// Keys are written verbatim (e.g. `terroir.eudr.cut_off_date`), so they
/// co-exist cleanly with admin keys in the same `HashMap`.
///
/// # Cancellation
///
/// The task checks `shutdown_rx` via `tokio::select!` on each loop iteration
/// so it exits cleanly when the gateway shuts down.
#[cfg(feature = "rdkafka-consumer")]
pub fn start_terroir_consumer(
    cfg: TerroirSettingsConsumerConfig,
    cache: Arc<AdminSettingsCache>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    // Adapt TerroirSettingsConsumerConfig → SettingsConsumerConfig so we can
    // reuse the existing build_consumer / process_message helpers.
    let adapted = SettingsConsumerConfig {
        brokers: cfg.brokers,
        group_id: cfg.group_id,
        topic: cfg.topic,
        reconnect_backoff: cfg.reconnect_backoff,
    };

    tokio::spawn(async move {
        loop {
            let consumer: StreamConsumer = match build_consumer(&adapted) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        topic = %adapted.topic,
                        err = %e,
                        "terroir settings consumer: failed to create rdkafka consumer, retrying in {:?}",
                        adapted.reconnect_backoff
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(adapted.reconnect_backoff) => continue,
                        _ = shutdown_rx.recv() => {
                            tracing::info!("terroir settings consumer: shutdown signal received");
                            return;
                        }
                    }
                }
            };

            if let Err(e) = consumer.subscribe(&[adapted.topic.as_str()]) {
                tracing::warn!(
                    topic = %adapted.topic,
                    err = %e,
                    "terroir settings consumer: subscribe failed"
                );
                tokio::time::sleep(adapted.reconnect_backoff).await;
                continue;
            }

            tracing::info!(
                topic = %adapted.topic,
                group_id = %adapted.group_id,
                "terroir settings consumer: subscribed"
            );

            loop {
                tokio::select! {
                    msg = consumer.recv() => {
                        match msg {
                            Err(e) => {
                                tracing::error!(
                                    err = %e,
                                    "terroir settings consumer: receive error, reconnecting"
                                );
                                break;
                            }
                            Ok(m) => {
                                process_message(&m, &cache).await;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("terroir settings consumer: shutdown signal received");
                        return;
                    }
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(adapted.reconnect_backoff) => {}
                _ = shutdown_rx.recv() => {
                    tracing::info!(
                        "terroir settings consumer: shutdown signal received during backoff"
                    );
                    return;
                }
            }
        }
    });
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_defaults_provides_otp_rate_limit() {
        let cache = AdminSettingsCache::new();
        cache.seed_defaults().await;
        let v = cache.get("otp.rate_limit_per_user_5min").await;
        assert_eq!(v.and_then(|x| x.as_i64()), Some(3));
    }

    #[tokio::test]
    async fn update_overwrites_key() {
        let cache = AdminSettingsCache::new();
        cache.seed_defaults().await;
        cache
            .update("otp.lifetime_seconds".to_string(), serde_json::json!(600))
            .await;
        let v = cache.get_i64("otp.lifetime_seconds", 300).await;
        assert_eq!(v, 600);
    }

    #[tokio::test]
    async fn get_i64_returns_default_when_missing() {
        let cache = AdminSettingsCache::new();
        let v = cache.get_i64("nonexistent.key", 42).await;
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn multiple_updates_are_independent() {
        let cache = AdminSettingsCache::new();
        cache.seed_defaults().await;

        cache
            .update("otp.max_attempts".to_string(), serde_json::json!(5))
            .await;
        cache
            .update("otp.lifetime_seconds".to_string(), serde_json::json!(120))
            .await;

        assert_eq!(cache.get_i64("otp.max_attempts", 3).await, 5);
        assert_eq!(cache.get_i64("otp.lifetime_seconds", 300).await, 120);
        // Untouched key must retain seeded value.
        assert_eq!(cache.get_i64("otp.rate_limit_per_user_5min", 3).await, 3);
    }

    // ── TERROIR settings tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn seed_terroir_defaults_provides_eudr_cut_off_date() {
        let cache = AdminSettingsCache::new();
        cache.seed_terroir_defaults().await;
        let v = cache.get_str("terroir.eudr.cut_off_date", "").await;
        assert_eq!(v, "2020-12-31");
    }

    #[tokio::test]
    async fn seed_terroir_defaults_provides_cache_ttl() {
        let cache = AdminSettingsCache::new();
        cache.seed_terroir_defaults().await;
        let v = cache.get_i64("terroir.eudr.cache_ttl_days", 0).await;
        assert_eq!(v, 30);
    }

    #[tokio::test]
    async fn seed_terroir_defaults_ussd_simulator_enabled_true() {
        let cache = AdminSettingsCache::new();
        cache.seed_terroir_defaults().await;
        let v = cache
            .get_bool("terroir.ussd.simulator_enabled", false)
            .await;
        assert!(v, "simulator_enabled must default to true in P0-P2");
    }

    #[tokio::test]
    async fn seed_terroir_defaults_agent_offline_max_days() {
        let cache = AdminSettingsCache::new();
        cache.seed_terroir_defaults().await;
        let v = cache
            .get_i64("terroir.session.agent_offline_max_days", 0)
            .await;
        assert_eq!(v, 14);
    }

    #[tokio::test]
    async fn terroir_settings_do_not_overlap_admin_settings() {
        let cache = AdminSettingsCache::new();
        cache.seed_defaults().await;
        cache.seed_terroir_defaults().await;

        // Admin key still intact.
        let otp = cache.get_i64("otp.lifetime_seconds", 0).await;
        assert_eq!(otp, 300);

        // TERROIR key independently set.
        let ttl = cache.get_i64("terroir.eudr.cache_ttl_days", 0).await;
        assert_eq!(ttl, 30);
    }

    #[tokio::test]
    async fn terroir_setting_can_be_overwritten() {
        let cache = AdminSettingsCache::new();
        cache.seed_terroir_defaults().await;

        // Simulate a message from terroir.settings.changed disabling the simulator.
        cache
            .update(
                "terroir.ussd.simulator_enabled".to_string(),
                serde_json::json!(false),
            )
            .await;

        let v = cache.get_bool("terroir.ussd.simulator_enabled", true).await;
        assert!(!v, "simulator_enabled must be false after update");
    }

    #[tokio::test]
    async fn get_bool_returns_default_when_missing() {
        let cache = AdminSettingsCache::new();
        let v = cache.get_bool("nonexistent.bool", true).await;
        assert!(v);
        let v2 = cache.get_bool("nonexistent.bool", false).await;
        assert!(!v2);
    }

    #[tokio::test]
    async fn get_str_returns_default_when_missing() {
        let cache = AdminSettingsCache::new();
        let v = cache.get_str("nonexistent.str", "fallback").await;
        assert_eq!(v, "fallback");
    }
}
