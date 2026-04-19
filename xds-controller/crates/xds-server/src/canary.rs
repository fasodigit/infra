// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Canary / progressive-rollout orchestrator.
//!
//! # State machine
//!
//! ```text
//! Stage1Pct ──(1 h SLO OK)──► Stage10Pct ──(1 h SLO OK)──► Stage50Pct ──(1 h SLO OK)──► Promoted(100 %)
//!     │                            │                              │
//!     └──────────────── SLO breach ≥ 3 consecutive ticks ────────┴──► RolledBack(0 %)
//!
//! Any state → Paused   (via PauseCanary RPC)
//! Paused    → stage it was paused at (via PromoteCanary / restart tick)
//! Any state → RolledBack (via AbortCanary RPC, or SLO breach)
//! ```
//!
//! # Tick
//!
//! A background task calls [`CanaryOrchestrator::tick`] every 30 s.
//! On each tick the orchestrator:
//! 1. Queries Prometheus for `rate(http_requests_total{cluster=canary}[5m])` and
//!    `histogram_quantile(0.99, rate(http_request_duration_seconds_bucket{cluster=canary}[5m]))`.
//! 2. Evaluates SLO compliance.
//! 3. Auto-advances when `stage_elapsed >= min_stage_duration` and the last tick was OK.
//! 4. Rollback after 3 consecutive SLO-breach ticks.
//! 5. On any weight change the orchestrator mutates the `ConfigStore` (CDS + RDS push).
//!
//! # Failure modes
//!
//! * **Prometheus unreachable**: treat as SLO unknown (not a breach). Log warn.
//! * **Canary cluster missing from store**: tick is a no-op; log error.
//! * **Concurrent mutations** to the same canary: the `DashMap` entry is updated
//!   atomically; the tick loop holds no lock across `.await`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::time;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::prometheus_client::{PrometheusClient, SloMetrics};
use xds_store::model::{
    ClusterEntry, DiscoveryType, LbPolicy, RouteEntry, RouteRuleEntry, PathMatch,
    VirtualHostEntry, WeightedClusterEntry,
};
use xds_store::ConfigStore;

// ---------------------------------------------------------------------------
// Prometheus metric IDs
// ---------------------------------------------------------------------------

/// Prometheus metric labels for the canary cluster.
pub const PROM_ERROR_RATE_QUERY: &str =
    "sum(rate(http_requests_total{cluster=\"{cluster}\",status=~\"5..\"}[5m])) / sum(rate(http_requests_total{cluster=\"{cluster}\"}[5m]))";

pub const PROM_LATENCY_P99_QUERY: &str =
    "histogram_quantile(0.99, sum(rate(http_request_duration_seconds_bucket{cluster=\"{cluster}\"}[5m])) by (le)) * 1000";

// ---------------------------------------------------------------------------
// Stage
// ---------------------------------------------------------------------------

/// Traffic weight stages for a progressive rollout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// 1 % canary weight.
    Stage1Pct,
    /// 10 % canary weight.
    Stage10Pct,
    /// 50 % canary weight.
    Stage50Pct,
    /// 100 % canary — fully promoted, stable cluster removed from route.
    Promoted,
    /// 0 % canary — rolled back due to SLO breach or manual abort.
    RolledBack,
    /// Paused at current weight; no tick advancement.
    Paused,
}

impl Stage {
    /// Canary traffic weight in percent [0..100].
    pub fn weight_pct(self) -> u32 {
        match self {
            Stage::Stage1Pct => 1,
            Stage::Stage10Pct => 10,
            Stage::Stage50Pct => 50,
            Stage::Promoted => 100,
            Stage::RolledBack => 0,
            Stage::Paused => 0, // weight unchanged externally; placeholder
        }
    }

    /// Returns the next stage in the advancement sequence, or `None` if already terminal.
    pub fn advance(self) -> Option<Stage> {
        match self {
            Stage::Stage1Pct => Some(Stage::Stage10Pct),
            Stage::Stage10Pct => Some(Stage::Stage50Pct),
            Stage::Stage50Pct => Some(Stage::Promoted),
            Stage::Promoted | Stage::RolledBack | Stage::Paused => None,
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Stage::Stage1Pct => "1pct",
            Stage::Stage10Pct => "10pct",
            Stage::Stage50Pct => "50pct",
            Stage::Promoted => "promoted",
            Stage::RolledBack => "rolled_back",
            Stage::Paused => "paused",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// SloConfig
// ---------------------------------------------------------------------------

/// SLO thresholds for a canary deployment.
#[derive(Debug, Clone)]
pub struct SloConfig {
    /// Maximum acceptable error rate (0.0–1.0).
    pub error_rate_max: f64,
    /// Maximum acceptable p99 latency in milliseconds.
    pub latency_p99_max_ms: f64,
    /// Prometheus endpoint to query.
    pub prometheus_endpoint: String,
}

impl Default for SloConfig {
    fn default() -> Self {
        Self {
            error_rate_max: 0.005,
            latency_p99_max_ms: 50.0,
            prometheus_endpoint: "http://prometheus:9090".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// CanaryEntry — per-canary state
// ---------------------------------------------------------------------------

/// Internal mutable state for a single active canary.
#[derive(Debug, Clone)]
pub struct CanaryEntry {
    pub canary_id: String,
    pub service: String,
    /// The stable (baseline) cluster name in the ConfigStore.
    pub stable_cluster: String,
    /// The canary cluster name in the ConfigStore.
    pub canary_cluster: String,
    pub image_tag: String,
    pub current_stage: Stage,
    /// The weight to restore when un-pausing.
    pub pre_pause_stage: Option<Stage>,
    pub slo: SloConfig,
    /// Minimum duration each stage must hold before advancement.
    pub min_stage_duration: Duration,
    pub started_at: DateTime<Utc>,
    pub stage_started_at: DateTime<Utc>,
    /// Time at which this stage's OK window started (reset on breach).
    stage_ok_since: Instant,
    /// Consecutive SLO-breach ticks.
    pub consecutive_breaches: u32,
    /// Most recent SLO compliance snapshot.
    pub last_compliance: Option<SloCompliance>,
    pub rollback_reason: Option<String>,
}

/// SLO compliance snapshot for a single tick.
#[derive(Debug, Clone)]
pub struct SloCompliance {
    pub observed_error_rate: f64,
    pub observed_latency_p99_ms: f64,
    pub within_budget: bool,
    pub measured_at: DateTime<Utc>,
}

impl CanaryEntry {
    /// Effective canary weight for the route (0..100).
    pub fn effective_weight_pct(&self) -> u32 {
        match self.current_stage {
            Stage::Paused => self
                .pre_pause_stage
                .map(|s| s.weight_pct())
                .unwrap_or(1),
            other => other.weight_pct(),
        }
    }

    /// Returns `true` if the canary is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.current_stage,
            Stage::Promoted | Stage::RolledBack
        )
    }
}

// ---------------------------------------------------------------------------
// CanaryOrchestrator
// ---------------------------------------------------------------------------

/// Orchestrates all active canary deployments.
///
/// Each canary is keyed by `canary_id`. The orchestrator's tick loop runs every
/// 30 s and calls `tick_all` which evaluates SLO and advances / rolls back each
/// non-terminal, non-paused canary.
#[derive(Clone)]
pub struct CanaryOrchestrator {
    canaries: Arc<DashMap<String, CanaryEntry>>,
    store: ConfigStore,
}

impl CanaryOrchestrator {
    /// Create a new orchestrator backed by `store`.
    pub fn new(store: ConfigStore) -> Self {
        Self {
            canaries: Arc::new(DashMap::new()),
            store,
        }
    }

    // -----------------------------------------------------------------------
    // RPC entry points
    // -----------------------------------------------------------------------

    /// Start a new canary deployment.  Returns the `canary_id`.
    pub fn start(
        &self,
        service: impl Into<String>,
        image_tag: impl Into<String>,
        slo: SloConfig,
        min_stage_duration: Duration,
    ) -> String {
        let service = service.into();
        let image_tag = image_tag.into();
        let canary_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let stable_cluster = format!("{service}-stable");
        let canary_cluster = format!("{service}-canary");

        // Ensure canary cluster exists in ConfigStore (add if absent).
        if self.store.get_cluster(&canary_cluster).is_none() {
            // Copy stable cluster config if present; otherwise create minimal entry.
            let base = self
                .store
                .get_cluster(&stable_cluster)
                .unwrap_or_else(|| ClusterEntry {
                    name: canary_cluster.clone(),
                    discovery_type: DiscoveryType::Eds,
                    lb_policy: LbPolicy::RoundRobin,
                    connect_timeout_ms: 5000,
                    health_check: None,
                    circuit_breaker: None,
                    spiffe_id: None,
                    metadata: HashMap::new(),
                    updated_at: now,
                });
            let canary_entry = ClusterEntry {
                name: canary_cluster.clone(),
                ..base
            };
            let _ = self.store.set_cluster(canary_entry);
            info!(service = %service, cluster = %canary_cluster, "canary cluster registered in store");
        }

        // Set initial weighted-cluster route (1 % canary).
        self.apply_weights(&service, &stable_cluster, &canary_cluster, Stage::Stage1Pct);

        let entry = CanaryEntry {
            canary_id: canary_id.clone(),
            service,
            stable_cluster,
            canary_cluster,
            image_tag,
            current_stage: Stage::Stage1Pct,
            pre_pause_stage: None,
            slo,
            min_stage_duration,
            started_at: now,
            stage_started_at: now,
            stage_ok_since: Instant::now(),
            consecutive_breaches: 0,
            last_compliance: None,
            rollback_reason: None,
        };

        self.canaries.insert(canary_id.clone(), entry);
        info!(canary_id = %canary_id, stage = "1pct", "canary started");
        canary_id
    }

    /// Pause a canary (halts tick advancement; route weights unchanged).
    pub fn pause(&self, canary_id: &str) -> Result<CanaryEntry, String> {
        let mut entry = self
            .canaries
            .get_mut(canary_id)
            .ok_or_else(|| format!("canary {canary_id} not found"))?;

        if entry.is_terminal() {
            return Err(format!(
                "canary {canary_id} is terminal ({})",
                entry.current_stage
            ));
        }
        if entry.current_stage == Stage::Paused {
            return Ok(entry.clone());
        }

        entry.pre_pause_stage = Some(entry.current_stage);
        entry.current_stage = Stage::Paused;
        info!(canary_id = %canary_id, "canary paused");
        Ok(entry.clone())
    }

    /// Abort a canary — sets weight to 0 % and marks as RolledBack.
    pub fn abort(&self, canary_id: &str, reason: impl Into<String>) -> Result<CanaryEntry, String> {
        let reason = reason.into();

        let (service, stable, canary, snapshot) = {
            let mut entry = self
                .canaries
                .get_mut(canary_id)
                .ok_or_else(|| format!("canary {canary_id} not found"))?;

            entry.rollback_reason = Some(reason.clone());
            entry.current_stage = Stage::RolledBack;
            entry.pre_pause_stage = None;

            let service = entry.service.clone();
            let stable = entry.stable_cluster.clone();
            let canary_cluster = entry.canary_cluster.clone();
            let snapshot = entry.clone();
            (service, stable, canary_cluster, snapshot)
            // DashMap RefMut dropped here
        };

        self.apply_weights(&service, &stable, &canary, Stage::RolledBack);
        info!(canary_id = %canary_id, reason = %reason, "canary aborted");
        Ok(snapshot)
    }

    /// Force-promote a canary to 100 % regardless of SLO.
    pub fn promote(&self, canary_id: &str) -> Result<CanaryEntry, String> {
        let (service, stable, canary, snapshot) = {
            let mut entry = self
                .canaries
                .get_mut(canary_id)
                .ok_or_else(|| format!("canary {canary_id} not found"))?;

            if entry.is_terminal() {
                return Err(format!(
                    "canary {canary_id} already terminal ({})",
                    entry.current_stage
                ));
            }

            entry.current_stage = Stage::Promoted;
            entry.pre_pause_stage = None;
            entry.stage_started_at = Utc::now();

            let service = entry.service.clone();
            let stable = entry.stable_cluster.clone();
            let canary_cluster = entry.canary_cluster.clone();
            let snapshot = entry.clone();
            (service, stable, canary_cluster, snapshot)
            // DashMap RefMut dropped here
        };

        self.apply_weights(&service, &stable, &canary, Stage::Promoted);
        info!(canary_id = %canary_id, "canary force-promoted to 100%");
        Ok(snapshot)
    }

    /// Return a snapshot of a canary's current state.
    pub fn status(&self, canary_id: &str) -> Option<CanaryEntry> {
        self.canaries.get(canary_id).map(|e| e.clone())
    }

    /// Return snapshots of all canaries for a given service.
    pub fn list_for_service(&self, service: &str) -> Vec<CanaryEntry> {
        self.canaries
            .iter()
            .filter(|e| e.service == service)
            .map(|e| e.clone())
            .collect()
    }

    // -----------------------------------------------------------------------
    // Tick loop
    // -----------------------------------------------------------------------

    /// Run the tick loop indefinitely.  Call from a `tokio::spawn`.
    pub async fn run_tick_loop(self) {
        let mut interval = time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            self.tick_all().await;
        }
    }

    /// Evaluate all active canaries for one tick.
    pub async fn tick_all(&self) {
        let ids: Vec<String> = self
            .canaries
            .iter()
            .filter(|e| !e.is_terminal() && e.current_stage != Stage::Paused)
            .map(|e| e.canary_id.clone())
            .collect();

        for id in ids {
            if let Err(e) = self.tick_one(&id).await {
                error!(canary_id = %id, error = %e, "canary tick error");
            }
        }
    }

    async fn tick_one(&self, canary_id: &str) -> Result<(), String> {
        // Read current state (drop the DashMap guard before any .await).
        let snapshot = self
            .canaries
            .get(canary_id)
            .map(|e| e.clone())
            .ok_or_else(|| format!("canary {canary_id} not found during tick"))?;

        if snapshot.is_terminal() || snapshot.current_stage == Stage::Paused {
            return Ok(());
        }

        // Query Prometheus (no lock held).
        let prom = PrometheusClient::new(&snapshot.slo.prometheus_endpoint);
        let metrics = self
            .query_slo_metrics(&prom, &snapshot.canary_cluster)
            .await;

        let compliance = match metrics {
            Some(m) => SloCompliance {
                observed_error_rate: m.error_rate,
                observed_latency_p99_ms: m.latency_p99_ms,
                within_budget: m.error_rate <= snapshot.slo.error_rate_max
                    && m.latency_p99_ms <= snapshot.slo.latency_p99_max_ms,
                measured_at: Utc::now(),
            },
            None => {
                // Prometheus unreachable → not a breach, skip advancement this tick.
                warn!(
                    canary_id = %canary_id,
                    "Prometheus unreachable — skipping SLO evaluation this tick"
                );
                return Ok(());
            }
        };

        // Re-acquire the entry for mutation (still no .await after this point).
        let mut entry = self
            .canaries
            .get_mut(canary_id)
            .ok_or_else(|| format!("canary {canary_id} disappeared mid-tick"))?;

        entry.last_compliance = Some(compliance.clone());

        if !compliance.within_budget {
            entry.consecutive_breaches += 1;
            entry.stage_ok_since = Instant::now(); // reset OK window
            warn!(
                canary_id = %canary_id,
                stage = %entry.current_stage,
                error_rate = compliance.observed_error_rate,
                latency_p99_ms = compliance.observed_latency_p99_ms,
                consecutive_breaches = entry.consecutive_breaches,
                "SLO breach on canary tick"
            );

            if entry.consecutive_breaches >= 3 {
                // Rollback.
                let reason = format!(
                    "SLO breach: error_rate={:.4} (max={:.4}), p99={:.1}ms (max={:.1}ms)",
                    compliance.observed_error_rate,
                    entry.slo.error_rate_max,
                    compliance.observed_latency_p99_ms,
                    entry.slo.latency_p99_max_ms
                );
                entry.rollback_reason = Some(reason.clone());
                entry.current_stage = Stage::RolledBack;
                entry.pre_pause_stage = None;

                let service = entry.service.clone();
                let stable = entry.stable_cluster.clone();
                let canary = entry.canary_cluster.clone();
                drop(entry); // release lock before store write

                self.apply_weights(&service, &stable, &canary, Stage::RolledBack);
                info!(canary_id = %canary_id, reason = %reason, "canary auto-rolled back");
            }
            return Ok(());
        }

        // SLO OK — reset breach counter.
        entry.consecutive_breaches = 0;

        // Check if minimum stage duration has elapsed (wall-clock).
        let stage_wall_elapsed = Utc::now()
            .signed_duration_since(entry.stage_started_at)
            .to_std()
            .unwrap_or_default();

        if stage_wall_elapsed >= entry.min_stage_duration {
            if let Some(next_stage) = entry.current_stage.advance() {
                let prev = entry.current_stage;
                entry.current_stage = next_stage;
                entry.stage_started_at = Utc::now();
                entry.stage_ok_since = Instant::now();
                entry.consecutive_breaches = 0;

                let service = entry.service.clone();
                let stable = entry.stable_cluster.clone();
                let canary = entry.canary_cluster.clone();
                drop(entry); // release DashMap RefMut before store write

                self.apply_weights(&service, &stable, &canary, next_stage);
                info!(
                    canary_id = %canary_id,
                    prev_stage = %prev,
                    next_stage = %next_stage,
                    "canary advanced to next stage"
                );
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // xDS store mutations
    // -----------------------------------------------------------------------

    /// Apply weighted-cluster weights to the ConfigStore route for `service`.
    ///
    /// Builds / replaces the route `<service>-canary-route` with `weighted_clusters`
    /// pointing to stable and canary with the appropriate split.
    fn apply_weights(&self, service: &str, stable: &str, canary: &str, stage: Stage) {
        let weight_canary = stage.weight_pct();
        let weight_stable = 100u32.saturating_sub(weight_canary);

        let route_name = format!("{service}-canary-route");

        // Build weighted clusters for the route rule.
        let weighted_clusters = if weight_canary == 0 {
            // 100 % stable → no split, single cluster.
            None
        } else if weight_canary == 100 {
            // 100 % canary → no split, single cluster.
            None
        } else {
            Some(vec![
                WeightedClusterEntry {
                    name: stable.to_string(),
                    weight: weight_stable,
                },
                WeightedClusterEntry {
                    name: canary.to_string(),
                    weight: weight_canary,
                },
            ])
        };

        // Single cluster target when no split.
        let single_cluster = if weight_canary == 100 {
            canary.to_string()
        } else {
            stable.to_string()
        };

        let rule = RouteRuleEntry {
            name: Some(format!("{service}-route-rule")),
            path_match: PathMatch::Prefix("/".to_string()),
            header_matchers: vec![],
            cluster: single_cluster,
            weighted_clusters,
            timeout_ms: Some(30_000),
            retry_policy: None,
            prefix_rewrite: None,
        };

        let route = RouteEntry {
            name: route_name.clone(),
            virtual_hosts: vec![VirtualHostEntry {
                name: format!("{service}-vhost"),
                domains: vec![format!("{service}.faso.internal")],
                routes: vec![rule],
            }],
            updated_at: Utc::now(),
        };

        if let Err(e) = self.store.set_route(route) {
            error!(
                service = %service,
                route = %route_name,
                error = %e,
                "failed to apply canary weights to ConfigStore"
            );
        } else {
            info!(
                service = %service,
                stable_pct = weight_stable,
                canary_pct = weight_canary,
                stage = %stage,
                "canary weights applied to xDS route"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Prometheus helpers
    // -----------------------------------------------------------------------

    async fn query_slo_metrics(
        &self,
        prom: &PrometheusClient,
        cluster: &str,
    ) -> Option<SloMetrics> {
        let error_q = PROM_ERROR_RATE_QUERY.replace("{cluster}", cluster);
        let latency_q = PROM_LATENCY_P99_QUERY.replace("{cluster}", cluster);

        let (error_result, latency_result) =
            tokio::join!(prom.query_scalar(&error_q), prom.query_scalar(&latency_q));

        match (error_result, latency_result) {
            (Ok(error_rate), Ok(latency_p99_ms)) => Some(SloMetrics {
                error_rate,
                latency_p99_ms,
            }),
            (Err(e), _) | (_, Err(e)) => {
                warn!(cluster = %cluster, error = %e, "Prometheus query failed");
                None
            }
        }
    }
}
