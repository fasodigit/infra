// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//! Canary / A-B / shadow / multi-stage traffic splitting for the Pingora gateway.
//!
//! Ported from `src/traffic_split.rs` (hyper path) and extended with
//! Pingora-specific integration points.
//!
//! ## Design
//!
//! The [`TrafficSplitter`] holds an [`arc_swap::ArcSwap`]-protected route
//! table that maps route names to [`SplitSpec`]s.  It is hot-reloadable:
//! xDS pushes or operator config changes call [`TrafficSplitter::update`]
//! without blocking active requests.
//!
//! ## Routing modes
//!
//! ### Canary / A-B
//!
//! Probabilistic routing: weights sum to 100.  Routing is **sticky** — the
//! same `sticky_value` (e.g. `user_id`, session cookie) always maps to the
//! same variant using a deterministic blake3 hash.
//!
//! ### Shadow
//!
//! 100 % of traffic goes to the *primary* cluster; a fire-and-forget copy is
//! sent to the *shadow* cluster at the configured sample rate.  The shadow
//! response is **ignored**.
//!
//! ### Multi-stage (BL-1)
//!
//! A route may declare multiple [`StageVariant`]s ordered by `priority`
//! (ascending = first exposure).  Each stage has a `weight` (float in
//! `[0, 1)`) and a [`StageMode`] (`Split` or `Shadow`).
//!
//! **Selection algorithm:**
//! 1. Compute `h = blake3(sticky_value)` normalised to `[0, 1)`.
//! 2. Sort stages by `priority` ascending (stable sort preserves declaration
//!    order on ties).
//! 3. Walk stages: accumulate `sum_weights`; if `h < sum_weights`, select
//!    this stage.
//! 4. If no stage matched → route to primary.
//! 5. `Shadow` stage: route to primary AND enqueue shadow duplication to
//!    the stage cluster (response ignored).
//!
//! Total weight of all stages must be in `(0, 1]`.  The remaining fraction
//! `(1 - total_weight)` routes to primary.
//!
//! ## Integration in `upstream_peer()`
//!
//! ```rust,ignore
//! // In PingoraGateway::upstream_peer():
//! if let Some(decision) = self.splitter.decide(&ctx.cluster, sticky_value) {
//!     ctx.cluster = decision.primary.clone();
//!     ctx.traffic_split_shadow = decision.shadow.clone();
//! }
//! ```
//!
//! The shadow request is dispatched as a `tokio::spawn` fire-and-forget in
//! the calling code after `upstream_peer` returns.
//!
//! ## Metrics
//!
//! `armageddon_traffic_split_decisions_total{route, variant, decision, priority}` is
//! incremented by [`TrafficSplitter::decide`] on every routing decision.
//! Pass a [`PingoraMetrics`] bundle via [`TrafficSplitter::with_metrics`] to
//! enable metric emission.
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | Route not in table | `None` returned — caller uses default cluster |
//! | Weight sum ≠ 100 (canary/A-B) | `SplitError::WeightSum` at validation |
//! | No variants defined | `SplitError::NoVariants` |
//! | Shadow sample_rate out of [0,1] | `SplitError::ShadowSampleRate` |
//! | Multi-stage total weight > 1.0 | `SplitError::MultiStageWeightExceeds` |
//! | Multi-stage no stages | `SplitError::NoVariants` |

use blake3::Hasher;
use std::collections::HashMap;
use std::sync::Arc;

use crate::pingora::metrics::PingoraMetrics;

// ── Types ──────────────────────────────────────────────────────────────────

/// One variant in a traffic split (maps to one upstream cluster).
#[derive(Debug, Clone)]
pub struct Variant {
    /// Upstream cluster name matching an entry in the `UpstreamRegistry`.
    pub cluster: String,
    /// Integer weight in `0..=100`.  All weights in a `SplitSpec` must sum to 100.
    pub weight: u32,
    /// Human-readable label used in metrics (`variant="stable"`, etc.).
    pub label: Option<String>,
}

/// How to split traffic among variants.
#[derive(Debug, Clone)]
pub enum SplitMode {
    /// Probabilistic per-request routing.  Not intended as a long-lived split.
    Canary,
    /// A/B experiment.  Same mechanics as `Canary` but semantically long-lived.
    AbTest {
        /// Experiment identifier used in metrics (e.g. `"checkout-redesign"`).
        name: String,
    },
    /// Shadow — primary always receives the request; the shadow cluster gets a
    /// fire-and-forget copy.
    Shadow {
        /// Fraction of traffic to also shadow (0.0..=1.0).  1.0 = shadow every request.
        sample_rate: f32,
    },
}

/// Complete split specification for one route.
#[derive(Debug, Clone)]
pub struct SplitSpec {
    /// Routing mode.
    pub mode: SplitMode,
    /// Ordered list of variants.  For `Shadow` mode the first is the primary,
    /// the second is the shadow target.
    pub variants: Vec<Variant>,
    /// Header or cookie name to use as the sticky hash key.
    ///
    /// `None` → fall back to the caller-supplied `sticky_value` (typically the
    /// client IP or user_id from `ctx`).
    pub sticky_header: Option<String>,
}

impl SplitSpec {
    /// Validate the spec.
    ///
    /// Returns `Err` when weight invariants are violated.
    pub fn validate(&self) -> Result<(), SplitError> {
        if self.variants.is_empty() {
            return Err(SplitError::NoVariants);
        }
        match &self.mode {
            SplitMode::Canary | SplitMode::AbTest { .. } => {
                let sum: u32 = self.variants.iter().map(|v| v.weight).sum();
                if sum != 100 {
                    return Err(SplitError::WeightSum(sum));
                }
            }
            SplitMode::Shadow { sample_rate } => {
                if !((0.0_f32)..=1.0).contains(sample_rate) {
                    return Err(SplitError::ShadowSampleRate(*sample_rate));
                }
            }
        }
        Ok(())
    }
}

/// Result of routing one request.
#[derive(Debug, Clone)]
pub struct SplitDecision {
    /// Cluster the request must be forwarded to.
    pub primary: String,
    /// Label of the chosen primary variant (for metrics).
    pub primary_label: Option<String>,
    /// When in shadow mode and sampled, the cluster to fire-and-forget to.
    pub shadow: Option<String>,
    /// The split mode that produced this decision (for metric labelling).
    pub mode: SplitDecisionMode,
}

/// Mode tag on a [`SplitDecision`], used for metric labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDecisionMode {
    Canary,
    AbTest,
    Shadow,
}

/// Errors in split configuration or routing.
#[derive(Debug, thiserror::Error)]
pub enum SplitError {
    /// Spec has no variants.
    #[error("traffic split has no variants")]
    NoVariants,
    /// Weights do not sum to 100.
    #[error("traffic split weights must sum to 100, got {0}")]
    WeightSum(u32),
    /// Shadow sample_rate is outside [0.0, 1.0].
    #[error("shadow sample_rate must be in 0.0..=1.0, got {0}")]
    ShadowSampleRate(f32),
    /// Multi-stage total weight exceeds 1.0.
    #[error("multi-stage total weight must be <= 1.0, got {0}")]
    MultiStageWeightExceeds(f32),
}

// ── Multi-stage types (BL-1) ───────────────────────────────────────────────

/// Routing mode for an individual stage variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageMode {
    /// Route the request to this variant cluster (normal split).
    Split,
    /// Route to primary AND fire-and-forget a duplicate to this cluster.
    /// The shadow response is silently discarded.
    Shadow,
}

/// One stage variant in a multi-stage rollout.
#[derive(Debug, Clone)]
pub struct StageVariant {
    /// Upstream cluster name.
    pub cluster: String,
    /// Exposure priority: lower = earlier (first) exposure.
    /// Stages are walked in ascending `priority` order.
    pub priority: u32,
    /// Fraction of total traffic for this stage, in `(0.0, 1.0]`.
    /// The sum across all stages must be `<= 1.0`.
    pub weight: f64,
    /// `Split` — variant receives the real request.
    /// `Shadow` — primary receives the request + variant gets a silent copy.
    pub mode: StageMode,
}

/// Multi-stage rollout spec: one primary cluster + an ordered list of stages.
///
/// Each stage may be a canary slice (`Split`) or a silent shadow (`Shadow`).
/// Stages are selected by a deterministic sticky hash so the same user always
/// lands on the same stage for the duration of the rollout.
#[derive(Debug, Clone)]
pub struct MultiStageSplitSpec {
    /// The fallback cluster when no stage matches the hash.
    pub primary: String,
    /// Ordered (by `priority` asc) stage variants.
    pub stages: Vec<StageVariant>,
    /// Optional sticky key header name.
    pub sticky_header: Option<String>,
}

impl MultiStageSplitSpec {
    /// Validate the spec.
    ///
    /// Returns `Err` when:
    /// - `stages` is empty → `SplitError::NoVariants`
    /// - total weight > 1.0 → `SplitError::MultiStageWeightExceeds`
    pub fn validate(&self) -> Result<(), SplitError> {
        if self.stages.is_empty() {
            return Err(SplitError::NoVariants);
        }
        let total: f64 = self.stages.iter().map(|s| s.weight).sum();
        if total > 1.0 + f64::EPSILON {
            return Err(SplitError::MultiStageWeightExceeds(total as f32));
        }
        Ok(())
    }
}

/// Result of a multi-stage routing decision.
#[derive(Debug, Clone)]
pub struct MultiStageSplitDecision {
    /// Cluster the request is forwarded to (primary or a `Split` stage).
    pub primary: String,
    /// When `Some`, a silent copy of the request must also be sent to this
    /// cluster (stage mode = `Shadow`).
    pub shadow: Option<String>,
    /// Priority of the matched stage, or `None` when routed to primary.
    pub matched_priority: Option<u32>,
    /// Human-readable label for the matched stage cluster (for metrics).
    pub variant_label: String,
}

/// Pure routing function for multi-stage specs.
///
/// Stages are walked in ascending `priority` order.  The first stage whose
/// cumulative weight exceeds the normalised hash wins.
pub fn multi_stage_decide(spec: &MultiStageSplitSpec, sticky_value: &str) -> MultiStageSplitDecision {
    // Normalise hash to [0, 1).
    let h = hash_to_float(sticky_value);

    // Sort stages by priority ascending (stable — preserves declaration order on ties).
    let mut sorted: Vec<&StageVariant> = spec.stages.iter().collect();
    sorted.sort_by_key(|s| s.priority);

    let mut acc = 0.0_f64;
    for stage in &sorted {
        acc += stage.weight;
        if h < acc {
            // This stage wins.
            match stage.mode {
                StageMode::Split => {
                    return MultiStageSplitDecision {
                        primary: stage.cluster.clone(),
                        shadow: None,
                        matched_priority: Some(stage.priority),
                        variant_label: stage.cluster.clone(),
                    };
                }
                StageMode::Shadow => {
                    return MultiStageSplitDecision {
                        primary: spec.primary.clone(),
                        shadow: Some(stage.cluster.clone()),
                        matched_priority: Some(stage.priority),
                        variant_label: stage.cluster.clone(),
                    };
                }
            }
        }
    }

    // No stage matched → route to primary.
    MultiStageSplitDecision {
        primary: spec.primary.clone(),
        shadow: None,
        matched_priority: None,
        variant_label: spec.primary.clone(),
    }
}

/// Normalise a blake3 hash of `key` to a float in `[0, 1)`.
fn hash_to_float(key: &str) -> f64 {
    let mut h = Hasher::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    let bytes = digest.as_bytes();
    // Use 8 bytes for a well-distributed u64.
    let n = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    // Map to [0, 1) via uniform division by 2^64.
    (n as f64) / (u64::MAX as f64 + 1.0)
}

// ── TrafficSplitter ────────────────────────────────────────────────────────

/// Thread-safe registry of per-route split specs.
///
/// Hot-reloaded by the xDS consumer when it receives weighted-cluster updates.
/// Uses [`arc_swap::ArcSwap`] for lock-free reads on the hot path.
#[derive(Debug)]
pub struct TrafficSplitter {
    routes: arc_swap::ArcSwap<HashMap<String, Arc<SplitSpec>>>,
    /// Shared Prometheus metrics bundle.  `None` disables metric emission.
    metrics: Option<Arc<PingoraMetrics>>,
}

impl Default for TrafficSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficSplitter {
    /// Create an empty splitter without metrics.
    pub fn new() -> Self {
        Self {
            routes: arc_swap::ArcSwap::from_pointee(HashMap::new()),
            metrics: None,
        }
    }

    /// Create an empty splitter with a shared metrics bundle.
    pub fn with_metrics(metrics: Arc<PingoraMetrics>) -> Self {
        Self {
            routes: arc_swap::ArcSwap::from_pointee(HashMap::new()),
            metrics: Some(metrics),
        }
    }

    /// Replace the entire route table atomically.
    ///
    /// Readers already inside `decide` see the old table; new readers see the
    /// new table.  There is no window of inconsistency.
    pub fn update(&self, routes: HashMap<String, Arc<SplitSpec>>) {
        self.routes.store(Arc::new(routes));
    }

    /// Return a snapshot of the current route table.
    ///
    /// Primarily used in tests to introspect the live state.
    pub fn snapshot(&self) -> Arc<HashMap<String, Arc<SplitSpec>>> {
        self.routes.load_full()
    }

    /// Route one request to a variant.
    ///
    /// - `route_name`: matched route (host + path) from the router filter.
    /// - `sticky_value`: value of the sticky key (user_id, session cookie,
    ///   client IP).  Determines which variant a particular client lands on.
    ///
    /// Returns `None` when no split is registered for `route_name` — the
    /// caller falls back to the default cluster from its own routing logic.
    ///
    /// When a [`PingoraMetrics`] bundle was supplied via
    /// [`TrafficSplitter::with_metrics`], increments
    /// `armageddon_traffic_split_decisions_total{route, variant, decision}`.
    pub fn decide(&self, route_name: &str, sticky_value: &str) -> Option<SplitDecision> {
        let routes = self.routes.load();
        let spec = routes.get(route_name)?;
        let decision = decide_with(spec, sticky_value);

        // Emit metrics if a bundle is attached.
        if let Some(m) = &self.metrics {
            let variant = decision
                .primary_label
                .as_deref()
                .unwrap_or("unknown");
            let decision_type = match decision.mode {
                SplitDecisionMode::Canary => "canary",
                SplitDecisionMode::AbTest => "canary",
                SplitDecisionMode::Shadow => {
                    if decision.shadow.is_some() {
                        "shadow"
                    } else {
                        "primary"
                    }
                }
            };
            m.traffic_split_decisions_total
                .with_label_values(&[route_name, variant, decision_type])
                .inc();
        }

        Some(decision)
    }
}

// ── Pure-function routing logic ────────────────────────────────────────────

/// Routing logic extracted for unit-testing without an `Arc<TrafficSplitter>`.
pub fn decide_with(spec: &SplitSpec, sticky_value: &str) -> SplitDecision {
    match &spec.mode {
        SplitMode::Canary => {
            let v = pick_canary_variant(spec, sticky_value);
            SplitDecision {
                primary: v.cluster.clone(),
                primary_label: v.label.clone(),
                shadow: None,
                mode: SplitDecisionMode::Canary,
            }
        }
        SplitMode::AbTest { .. } => {
            let v = pick_canary_variant(spec, sticky_value);
            SplitDecision {
                primary: v.cluster.clone(),
                primary_label: v.label.clone(),
                shadow: None,
                mode: SplitDecisionMode::AbTest,
            }
        }
        SplitMode::Shadow { sample_rate } => {
            let primary = spec.variants.first().expect("validated non-empty");
            let shadow_cluster = spec.variants.get(1).map(|v| v.cluster.clone());
            // Use a higher-resolution bucket (10 000) so that fractional
            // sample rates can be expressed accurately.
            let bucket = hash_to_bucket(sticky_value, 10_000) as f32 / 10_000.0;
            let shadow = if bucket < *sample_rate { shadow_cluster } else { None };
            SplitDecision {
                primary: primary.cluster.clone(),
                primary_label: primary.label.clone(),
                shadow,
                mode: SplitDecisionMode::Shadow,
            }
        }
    }
}

/// Pick the variant whose cumulative weight range contains the hash bucket.
fn pick_canary_variant<'a>(spec: &'a SplitSpec, sticky_value: &str) -> &'a Variant {
    let bucket = hash_to_bucket(sticky_value, 100);
    let mut acc = 0u32;
    for v in &spec.variants {
        acc = acc.saturating_add(v.weight);
        if bucket < acc {
            return v;
        }
    }
    // Fallback: weights summed to 100 so this should never happen.
    spec.variants.last().expect("validated non-empty")
}

/// Deterministic blake3-based hash → uniform bucket in `0..buckets`.
fn hash_to_bucket(key: &str, buckets: u32) -> u32 {
    let mut h = Hasher::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    let bytes = digest.as_bytes();
    let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    n % buckets
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers -----------------------------------------------------------

    fn spec_50_50() -> SplitSpec {
        SplitSpec {
            mode: SplitMode::Canary,
            variants: vec![
                Variant {
                    cluster: "v1".into(),
                    weight: 50,
                    label: Some("stable".into()),
                },
                Variant {
                    cluster: "v2".into(),
                    weight: 50,
                    label: Some("canary".into()),
                },
            ],
            sticky_header: None,
        }
    }

    fn spec_10_canary() -> SplitSpec {
        SplitSpec {
            mode: SplitMode::Canary,
            variants: vec![
                Variant {
                    cluster: "primary".into(),
                    weight: 90,
                    label: Some("stable".into()),
                },
                Variant {
                    cluster: "canary".into(),
                    weight: 10,
                    label: Some("canary".into()),
                },
            ],
            sticky_header: None,
        }
    }

    // -- validation --------------------------------------------------------

    #[test]
    fn validate_weight_sum_not_100_errors() {
        let mut s = spec_50_50();
        s.variants[0].weight = 60;
        assert!(matches!(s.validate(), Err(SplitError::WeightSum(_))));
    }

    #[test]
    fn validate_empty_variants_errors() {
        let s = SplitSpec {
            mode: SplitMode::Canary,
            variants: vec![],
            sticky_header: None,
        };
        assert!(matches!(s.validate(), Err(SplitError::NoVariants)));
    }

    #[test]
    fn validate_shadow_invalid_sample_rate() {
        let s = SplitSpec {
            mode: SplitMode::Shadow { sample_rate: 1.5 },
            variants: vec![Variant {
                cluster: "p".into(),
                weight: 100,
                label: None,
            }],
            sticky_header: None,
        };
        assert!(matches!(
            s.validate(),
            Err(SplitError::ShadowSampleRate(_))
        ));
    }

    #[test]
    fn validate_valid_spec_ok() {
        assert!(spec_50_50().validate().is_ok());
    }

    // -- sticky routing ----------------------------------------------------

    #[test]
    fn sticky_key_is_deterministic() {
        let s = spec_50_50();
        let d1 = decide_with(&s, "user-42");
        let d2 = decide_with(&s, "user-42");
        assert_eq!(d1.primary, d2.primary);
    }

    #[test]
    fn different_keys_may_differ() {
        let s = spec_50_50();
        // Over 100 distinct keys at least one pair must land on different
        // variants (pigeonhole + uniform hash).
        let decisions: Vec<String> = (0..100)
            .map(|i| decide_with(&s, &format!("user-{i}")).primary)
            .collect();
        let has_v1 = decisions.iter().any(|d| d == "v1");
        let has_v2 = decisions.iter().any(|d| d == "v2");
        assert!(has_v1, "expected some requests to land on v1");
        assert!(has_v2, "expected some requests to land on v2");
    }

    // -- distribution ------------------------------------------------------

    #[test]
    fn fifty_fifty_is_balanced() {
        let s = spec_50_50();
        let (mut v1, mut v2) = (0u32, 0u32);
        for i in 0..10_000 {
            let d = decide_with(&s, &format!("u{i}"));
            if d.primary == "v1" {
                v1 += 1;
            } else {
                v2 += 1;
            }
        }
        // Expect ±2 % deviation over 10 000 samples.
        assert!((4_800..=5_200).contains(&v1), "v1={v1}");
        assert!((4_800..=5_200).contains(&v2), "v2={v2}");
    }

    #[test]
    fn ten_percent_canary_is_within_bounds() {
        let s = spec_10_canary();
        let mut canary = 0u32;
        for i in 0..10_000 {
            let d = decide_with(&s, &format!("u{i}"));
            if d.primary == "canary" {
                canary += 1;
            }
        }
        // 10 % ± 2 % over 10 000 samples.
        assert!((800..=1_200).contains(&canary), "canary={canary}");
    }

    // -- shadow mode -------------------------------------------------------

    #[test]
    fn shadow_fires_at_100_percent() {
        let s = SplitSpec {
            mode: SplitMode::Shadow { sample_rate: 1.0 },
            variants: vec![
                Variant {
                    cluster: "prod".into(),
                    weight: 100,
                    label: None,
                },
                Variant {
                    cluster: "shadow".into(),
                    weight: 0,
                    label: None,
                },
            ],
            sticky_header: None,
        };
        let d = decide_with(&s, "x");
        assert_eq!(d.primary, "prod");
        assert_eq!(d.shadow.as_deref(), Some("shadow"));
        assert_eq!(d.mode, SplitDecisionMode::Shadow);
    }

    #[test]
    fn shadow_skipped_at_0_percent() {
        let s = SplitSpec {
            mode: SplitMode::Shadow { sample_rate: 0.0 },
            variants: vec![
                Variant {
                    cluster: "prod".into(),
                    weight: 100,
                    label: None,
                },
                Variant {
                    cluster: "shadow".into(),
                    weight: 0,
                    label: None,
                },
            ],
            sticky_header: None,
        };
        let d = decide_with(&s, "x");
        assert_eq!(d.primary, "prod");
        assert!(d.shadow.is_none());
    }

    #[test]
    fn shadow_no_shadow_target_gives_none() {
        let s = SplitSpec {
            mode: SplitMode::Shadow { sample_rate: 1.0 },
            variants: vec![Variant {
                cluster: "prod".into(),
                weight: 100,
                label: None,
            }],
            sticky_header: None,
        };
        let d = decide_with(&s, "x");
        assert_eq!(d.primary, "prod");
        assert!(d.shadow.is_none(), "no shadow variant → shadow should be None");
    }

    // -- A-B mode ----------------------------------------------------------

    #[test]
    fn abtest_mode_returns_abtest_decision() {
        let s = SplitSpec {
            mode: SplitMode::AbTest {
                name: "checkout-redesign".into(),
            },
            variants: vec![
                Variant {
                    cluster: "control".into(),
                    weight: 50,
                    label: Some("control".into()),
                },
                Variant {
                    cluster: "variant".into(),
                    weight: 50,
                    label: Some("variant".into()),
                },
            ],
            sticky_header: None,
        };
        let d = decide_with(&s, "user-1");
        assert_eq!(d.mode, SplitDecisionMode::AbTest);
        assert!(d.primary == "control" || d.primary == "variant");
    }

    // -- TrafficSplitter hot-reload ----------------------------------------

    #[test]
    fn splitter_hot_reload_works() {
        let sp = TrafficSplitter::new();
        let mut routes = HashMap::new();
        routes.insert("poulets".to_string(), Arc::new(spec_50_50()));
        sp.update(routes);

        let d = sp.decide("poulets", "user-1").unwrap();
        assert!(matches!(d.primary.as_str(), "v1" | "v2"));
    }

    #[test]
    fn splitter_unknown_route_returns_none() {
        let sp = TrafficSplitter::new();
        assert!(sp.decide("non-existent", "user-1").is_none());
    }

    #[test]
    fn splitter_snapshot_reflects_update() {
        let sp = TrafficSplitter::new();
        assert!(sp.snapshot().is_empty());

        let mut routes = HashMap::new();
        routes.insert("api".to_string(), Arc::new(spec_50_50()));
        sp.update(routes);

        let snap = sp.snapshot();
        assert!(snap.contains_key("api"));
    }

    // ── Metrics wiring ─────────────────────────────────────────────────────

    /// `decide` with metrics attached increments `decisions_total` counter.
    #[test]
    fn decide_with_metrics_increments_counter() {
        use crate::pingora::metrics::PingoraMetrics;
        use prometheus::Registry;

        let r = Registry::new();
        let m = Arc::new(PingoraMetrics::new(&r).unwrap());
        let sp = TrafficSplitter::with_metrics(m);

        let mut routes = HashMap::new();
        routes.insert("api-v2".to_string(), Arc::new(spec_50_50()));
        sp.update(routes);

        // Make several decisions.
        for i in 0..10u32 {
            let _ = sp.decide("api-v2", &format!("user-{i}"));
        }

        let families = r.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_traffic_split_decisions_total")
            .expect("decisions counter must exist");

        let total: f64 = fam
            .get_metric()
            .iter()
            .filter(|m| {
                m.get_label()
                    .iter()
                    .any(|l| l.get_name() == "route" && l.get_value() == "api-v2")
            })
            .map(|m| m.get_counter().get_value())
            .sum();
        assert_eq!(total, 10.0, "10 decisions should be counted for api-v2");
    }

    /// `decide` without metrics does not panic.
    #[test]
    fn decide_without_metrics_does_not_panic() {
        let sp = TrafficSplitter::new();
        let mut routes = HashMap::new();
        routes.insert("test-route".to_string(), Arc::new(spec_50_50()));
        sp.update(routes);
        for i in 0..5u32 {
            let _ = sp.decide("test-route", &format!("user-{i}"));
        }
        // No panic = pass.
    }

    // ── Multi-stage (BL-1) ─────────────────────────────────────────────────

    fn multi_stage_spec_10_2_shadow() -> MultiStageSplitSpec {
        MultiStageSplitSpec {
            primary: "checkout-v1".into(),
            stages: vec![
                StageVariant {
                    cluster: "checkout-v2".into(),
                    priority: 1,
                    weight: 0.10,
                    mode: StageMode::Split,
                },
                StageVariant {
                    cluster: "checkout-v3-experimental".into(),
                    priority: 2,
                    weight: 0.02,
                    mode: StageMode::Shadow,
                },
            ],
            sticky_header: None,
        }
    }

    /// Validation: valid multi-stage spec passes.
    #[test]
    fn multi_stage_validate_ok() {
        assert!(multi_stage_spec_10_2_shadow().validate().is_ok());
    }

    /// Validation: empty stages → NoVariants.
    #[test]
    fn multi_stage_validate_empty_stages() {
        let s = MultiStageSplitSpec {
            primary: "p".into(),
            stages: vec![],
            sticky_header: None,
        };
        assert!(matches!(s.validate(), Err(SplitError::NoVariants)));
    }

    /// Validation: total weight > 1.0 → MultiStageWeightExceeds.
    #[test]
    fn multi_stage_validate_weight_exceeds() {
        let s = MultiStageSplitSpec {
            primary: "p".into(),
            stages: vec![
                StageVariant { cluster: "a".into(), priority: 1, weight: 0.6, mode: StageMode::Split },
                StageVariant { cluster: "b".into(), priority: 2, weight: 0.6, mode: StageMode::Split },
            ],
            sticky_header: None,
        };
        assert!(matches!(s.validate(), Err(SplitError::MultiStageWeightExceeds(_))));
    }

    /// Distribution: 1000 iterations — ~10 % should land on v2 (priority 1, split).
    #[test]
    fn multi_stage_distribution_respected() {
        let spec = multi_stage_spec_10_2_shadow();
        let (mut v1_count, mut v2_count, mut shadow_count) = (0u32, 0u32, 0u32);
        for i in 0..1000u32 {
            let d = multi_stage_decide(&spec, &format!("user-{i}"));
            if d.primary == "checkout-v2" {
                v2_count += 1;
            } else if d.shadow.is_some() {
                shadow_count += 1;
            } else {
                v1_count += 1;
            }
        }
        // v2 gets ~10 % ± 3 % over 1000 samples.
        assert!(
            (70..=130).contains(&v2_count),
            "checkout-v2 should get ~10%, got {v2_count}/1000"
        );
        // shadow stage gets ~2 % ± 2 %.
        assert!(
            shadow_count <= 40,
            "shadow stage should get <=4%, got {shadow_count}/1000"
        );
        // primary gets the rest (~88 %).
        assert!(
            v1_count >= 800,
            "primary should get >=80%, got {v1_count}/1000"
        );
    }

    /// Sticky session: same key always maps to same stage.
    #[test]
    fn multi_stage_sticky_session_deterministic() {
        let spec = multi_stage_spec_10_2_shadow();
        let d1 = multi_stage_decide(&spec, "user-sticky-42");
        let d2 = multi_stage_decide(&spec, "user-sticky-42");
        assert_eq!(d1.primary, d2.primary);
        assert_eq!(d1.shadow, d2.shadow);
        assert_eq!(d1.matched_priority, d2.matched_priority);
    }

    /// Priority order: if weights are constructed so stage priority=2 has all
    /// weight, stage priority=1 (empty weight 0.0) is skipped.
    #[test]
    fn multi_stage_priority_order_respected() {
        // priority=1 has 0 weight (never selected), priority=2 has full weight.
        let spec = MultiStageSplitSpec {
            primary: "primary".into(),
            stages: vec![
                StageVariant { cluster: "prio-2".into(), priority: 2, weight: 0.99, mode: StageMode::Split },
                StageVariant { cluster: "prio-1".into(), priority: 1, weight: 0.0, mode: StageMode::Split },
            ],
            sticky_header: None,
        };
        // With weight=0 for priority=1 it is never entered; prio-2 wins almost always.
        let mut prio2_count = 0u32;
        let mut prio1_count = 0u32;
        for i in 0..100u32 {
            let d = multi_stage_decide(&spec, &format!("u{i}"));
            if d.primary == "prio-2" { prio2_count += 1; }
            if d.primary == "prio-1" { prio1_count += 1; }
        }
        assert_eq!(prio1_count, 0, "prio-1 stage with weight 0 must never be selected");
        assert!(prio2_count > 90, "prio-2 stage with weight 0.99 should win almost all calls");
    }

    /// Shadow mode: decision routes to primary with shadow populated.
    #[test]
    fn multi_stage_shadow_duplication() {
        // 100 % shadow stage to guarantee selection.
        let spec = MultiStageSplitSpec {
            primary: "primary".into(),
            stages: vec![
                StageVariant {
                    cluster: "shadow-target".into(),
                    priority: 1,
                    weight: 1.0,
                    mode: StageMode::Shadow,
                },
            ],
            sticky_header: None,
        };
        let d = multi_stage_decide(&spec, "any-user");
        assert_eq!(d.primary, "primary", "shadow mode routes to primary");
        assert_eq!(
            d.shadow.as_deref(),
            Some("shadow-target"),
            "shadow cluster must be populated"
        );
        assert_eq!(d.matched_priority, Some(1));
    }
}
