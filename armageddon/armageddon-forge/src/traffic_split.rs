// SPDX-License-Identifier: AGPL-3.0-or-later
//! Traffic splitting: canary / A-B / shadow.
//!
//! This module decides, for each request, which *upstream cluster* variant to
//! forward it to, given a set of weighted variants. It is the host-side
//! companion to `armageddon-xds::WeightedCluster` configuration — when xDS
//! pushes a cluster with multiple weighted endpoints, we consume that weight
//! map here and apply it deterministically per request.
//!
//! # Strategies
//!
//! * **Canary / A-B** — probabilistic routing, weights sum to 100 (or 1.0).
//!   Deterministic by sticky key (user-id / session / cookie) so that a given
//!   client sees a consistent variant across requests during a canary window.
//! * **Shadow** — the request is dispatched to the *primary* and a background
//!   copy is fired-and-forgotten to the shadow variant, responses of the
//!   shadow are discarded. Used to exercise a new version on live traffic
//!   without user-visible impact.
//!
//! Shadow dispatch is a plain `tokio::spawn` with a bounded retry count;
//! never blocks the client path.

use blake3::Hasher;
use std::collections::HashMap;
use std::sync::Arc;

/// One variant of a traffic split (= one upstream cluster in xDS terms).
#[derive(Debug, Clone)]
pub struct Variant {
    /// Upstream cluster name (matches an `armageddon-lb` backend pool).
    pub cluster: String,
    /// Integer weight in [0..=100]. All weights of a `SplitSpec` must sum to 100.
    pub weight: u32,
    /// Optional label — reported via metrics (`variant="v1.3.0"`).
    pub label: Option<String>,
}

/// How to split traffic among variants.
#[derive(Debug, Clone)]
pub enum SplitKind {
    /// Probabilistic per-request. Requires a hash key (client IP by default).
    Canary,
    /// A/B — same as `Canary` but the split is treated as long-lived (not a
    /// progressing rollout). Helps metric labelling.
    AbTest {
        /// Human-readable experiment name (e.g., `"checkout-redesign"`).
        name: String,
    },
    /// Shadow — primary always receives the request; shadow variants receive a
    /// fire-and-forget copy.
    Shadow {
        /// Fraction of traffic to also shadow (0.0..=1.0). 1.0 = shadow every
        /// request.
        sample_rate: f32,
    },
}

/// A complete split specification for one route.
#[derive(Debug, Clone)]
pub struct SplitSpec {
    pub kind: SplitKind,
    pub variants: Vec<Variant>,
    /// HTTP header / cookie name to derive the sticky hash from. If `None`,
    /// falls back to the client IP.
    pub sticky_header: Option<String>,
}

impl SplitSpec {
    /// Sanity check: weights sum to 100 for `Canary` / `AbTest`.
    pub fn validate(&self) -> Result<(), SplitError> {
        if self.variants.is_empty() {
            return Err(SplitError::NoVariants);
        }
        match self.kind {
            SplitKind::Canary | SplitKind::AbTest { .. } => {
                let sum: u32 = self.variants.iter().map(|v| v.weight).sum();
                if sum != 100 {
                    return Err(SplitError::WeightSum(sum));
                }
            }
            SplitKind::Shadow { sample_rate } => {
                if !(0.0..=1.0).contains(&sample_rate) {
                    return Err(SplitError::ShadowSampleRate(sample_rate));
                }
            }
        }
        Ok(())
    }
}

/// Decision of routing one request.
#[derive(Debug, Clone)]
pub struct SplitDecision {
    /// The cluster the request must be forwarded to.
    pub primary: String,
    /// Optional label of the primary — used for metrics.
    pub primary_label: Option<String>,
    /// When `Shadow` is active and sampled, this field holds the additional
    /// cluster to fire-and-forget to.
    pub shadow: Option<String>,
}

/// Errors in configuration or routing.
#[derive(Debug, thiserror::Error)]
pub enum SplitError {
    #[error("traffic split has no variants")]
    NoVariants,
    #[error("traffic split weights must sum to 100, got {0}")]
    WeightSum(u32),
    #[error("shadow sample_rate must be in 0.0..=1.0, got {0}")]
    ShadowSampleRate(f32),
}

/// Thread-safe registry of per-route splits. Hot-reloaded by xDS consumer
/// when it receives weighted-cluster updates.
#[derive(Debug, Default)]
pub struct TrafficSplitter {
    /// route-name -> spec
    routes: arc_swap::ArcSwap<HashMap<String, Arc<SplitSpec>>>,
}

impl TrafficSplitter {
    pub fn new() -> Self {
        Self {
            routes: arc_swap::ArcSwap::from_pointee(HashMap::new()),
        }
    }

    /// Replace the route table atomically.
    pub fn update(&self, routes: HashMap<String, Arc<SplitSpec>>) {
        self.routes.store(Arc::new(routes));
    }

    /// Return a clone of the current map (test helper).
    pub fn snapshot(&self) -> Arc<HashMap<String, Arc<SplitSpec>>> {
        self.routes.load_full()
    }

    /// Route one request to a variant.
    ///
    /// * `route_name` — the matched route (host+path) from the router.
    /// * `sticky_value` — value of the sticky-key (client IP, user-id, cookie).
    ///   When the route has no split registered, the caller must use the
    ///   default cluster from its own router.
    pub fn decide(&self, route_name: &str, sticky_value: &str) -> Option<SplitDecision> {
        let routes = self.routes.load();
        let spec = routes.get(route_name)?;
        Some(decide_with(spec, sticky_value))
    }
}

/// Pure-function split logic — extracted for unit-testing without the Arc.
pub fn decide_with(spec: &SplitSpec, sticky_value: &str) -> SplitDecision {
    match &spec.kind {
        SplitKind::Canary | SplitKind::AbTest { .. } => {
            // Deterministic hash bucket in [0..100).
            let bucket = hash_to_bucket(sticky_value, 100);
            let mut acc = 0u32;
            for v in &spec.variants {
                acc = acc.saturating_add(v.weight);
                if bucket < acc {
                    return SplitDecision {
                        primary: v.cluster.clone(),
                        primary_label: v.label.clone(),
                        shadow: None,
                    };
                }
            }
            // Safety: weights validated to sum=100. If floating-point shenanigans
            // got us here, fall back to last.
            let last = spec.variants.last().unwrap();
            SplitDecision {
                primary: last.cluster.clone(),
                primary_label: last.label.clone(),
                shadow: None,
            }
        }
        SplitKind::Shadow { sample_rate } => {
            let primary = spec.variants.first().expect("validated non-empty");
            let shadow = spec.variants.get(1).map(|v| v.cluster.clone());
            let bucket = hash_to_bucket(sticky_value, 10_000) as f32 / 10_000.0;
            let shadow = if bucket < *sample_rate { shadow } else { None };
            SplitDecision {
                primary: primary.cluster.clone(),
                primary_label: primary.label.clone(),
                shadow,
            }
        }
    }
}

/// Deterministic blake3-based hash → uniform bucket in `0..buckets`.
fn hash_to_bucket(key: &str, buckets: u32) -> u32 {
    let mut h = Hasher::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    // Take first 4 bytes as u32.
    let bytes = digest.as_bytes();
    let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    n % buckets
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_50_50() -> SplitSpec {
        SplitSpec {
            kind: SplitKind::Canary,
            variants: vec![
                Variant { cluster: "v1".into(), weight: 50, label: Some("stable".into()) },
                Variant { cluster: "v2".into(), weight: 50, label: Some("canary".into()) },
            ],
            sticky_header: None,
        }
    }

    #[test]
    fn validates_weight_sum() {
        let mut s = spec_50_50();
        s.variants[0].weight = 60;
        assert!(matches!(s.validate(), Err(SplitError::WeightSum(_))));
    }

    #[test]
    fn validates_empty() {
        let s = SplitSpec {
            kind: SplitKind::Canary,
            variants: vec![],
            sticky_header: None,
        };
        assert!(matches!(s.validate(), Err(SplitError::NoVariants)));
    }

    #[test]
    fn sticky_key_produces_stable_assignment() {
        let s = spec_50_50();
        let d1 = decide_with(&s, "user-42");
        let d2 = decide_with(&s, "user-42");
        assert_eq!(d1.primary, d2.primary);
    }

    #[test]
    fn fifty_fifty_is_reasonably_balanced() {
        let s = spec_50_50();
        let mut v1 = 0u32;
        let mut v2 = 0u32;
        for i in 0..10_000 {
            let d = decide_with(&s, &format!("u{}", i));
            if d.primary == "v1" { v1 += 1; } else { v2 += 1; }
        }
        // Expect ±2% deviation over 10 000 samples.
        assert!((4_800..=5_200).contains(&v1), "v1 got {v1}");
        assert!((4_800..=5_200).contains(&v2), "v2 got {v2}");
    }

    #[test]
    fn shadow_fires_when_sampled() {
        let s = SplitSpec {
            kind: SplitKind::Shadow { sample_rate: 1.0 },
            variants: vec![
                Variant { cluster: "prod".into(), weight: 100, label: None },
                Variant { cluster: "shadow".into(), weight: 0, label: None },
            ],
            sticky_header: None,
        };
        let d = decide_with(&s, "x");
        assert_eq!(d.primary, "prod");
        assert_eq!(d.shadow.as_deref(), Some("shadow"));
    }

    #[test]
    fn shadow_skipped_when_below_sample() {
        let s = SplitSpec {
            kind: SplitKind::Shadow { sample_rate: 0.0 },
            variants: vec![
                Variant { cluster: "prod".into(), weight: 100, label: None },
                Variant { cluster: "shadow".into(), weight: 0, label: None },
            ],
            sticky_header: None,
        };
        let d = decide_with(&s, "x");
        assert!(d.shadow.is_none());
    }

    #[test]
    fn splitter_hot_reload() {
        let sp = TrafficSplitter::new();
        let mut routes = HashMap::new();
        routes.insert("poulets".to_string(), Arc::new(spec_50_50()));
        sp.update(routes);
        let d = sp.decide("poulets", "user-1").unwrap();
        assert!(matches!(d.primary.as_str(), "v1" | "v2"));
    }
}
