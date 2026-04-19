// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! HTTP filter integrating local and global rate limiting into the FORGE pipeline.
//!
//! # Integration point
//!
//! Call `RateLimitFilter::check()` at the start of request handling, before
//! upstream forwarding.  The method is `async` and cancel-safe (no locks held
//! across `.await`).
//!
//! # Decision precedence (hybrid mode)
//!
//! 1. Local bucket checked first (no network hop).
//! 2. If local allows, global KAYA counter checked.
//! 3. If KAYA returns error, apply `fallback` policy.
//!
//! # Descriptor extraction
//!
//! Descriptors are built from the `HttpRequest` using the configured dimensions:
//! - `tenant`  — from `X-Tenant-Id` header
//! - `route`   — from `request.path`
//! - `ip`      — from `connection.client_ip`
//!
//! Multiple dimensions are joined: `"tenant:acme/route:/api/v1/poulets"`.

use std::sync::Arc;
use std::time::Instant;

use armageddon_common::types::HttpRequest;
use tracing::{debug, warn};

use crate::global::{BackendError, GlobalRateLimiter, RateLimitResult};
use crate::local::LocalTokenBucket;
use crate::metrics::RateLimitMetrics;

// ── decision ──────────────────────────────────────────────────────────────────

/// Decision returned by `RateLimitFilter::check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitDecision {
    /// Request is within all limits — proceed to upstream.
    Allow,
    /// Request exceeds limit — return 429 with `Retry-After: <n>` header.
    Deny { retry_after_secs: u64 },
    /// Shadow mode — over-limit but forward anyway (dry-run / canary).
    Shadow { retry_after_secs: u64 },
}

// ── mode ──────────────────────────────────────────────────────────────────────

/// Operating mode for the rate limit filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitMode {
    /// Local token bucket only — no KAYA call.
    Local,
    /// Global KAYA counter only — no local bucket.
    Global,
    /// Local checked first; if allowed, global checked second.
    Hybrid,
}

// ── fallback ──────────────────────────────────────────────────────────────────

/// What to do when KAYA is unreachable (global / hybrid modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackPolicy {
    /// Allow the request (fail-open — prefer availability).
    FailOpen,
    /// Deny the request (fail-closed — prefer safety).
    FailClosed,
}

// ── filter ────────────────────────────────────────────────────────────────────

/// Rate limit filter for the FORGE pipeline.
///
/// # Example (local mode)
/// ```rust,ignore
/// use armageddon_ratelimit::filter::{RateLimitFilter, RateLimitMode, FallbackPolicy};
/// use armageddon_ratelimit::LocalTokenBucket;
/// use armageddon_common::types::{RateLimitConfig, RateLimitDescriptorDimension};
/// use prometheus::Registry;
/// use std::sync::Arc;
///
/// let bucket = LocalTokenBucket::new();
/// bucket.add_rule("tenant:acme", 100, 200);
///
/// let filter = RateLimitFilter::new_local(Arc::new(bucket), &Registry::new()).unwrap();
/// // filter.check(&http_request).await  → RateLimitDecision::Allow
/// ```
pub struct RateLimitFilter {
    mode: RateLimitMode,
    local: Option<Arc<LocalTokenBucket>>,
    global: Option<Arc<GlobalRateLimiter>>,
    fallback: FallbackPolicy,
    shadow: bool,
    metrics: RateLimitMetrics,
}

impl RateLimitFilter {
    /// Construct a local-only filter.
    pub fn new_local(
        bucket: Arc<LocalTokenBucket>,
        registry: &prometheus::Registry,
    ) -> Result<Self, prometheus::Error> {
        Ok(Self {
            mode: RateLimitMode::Local,
            local: Some(bucket),
            global: None,
            fallback: FallbackPolicy::FailOpen,
            shadow: false,
            metrics: RateLimitMetrics::new(registry)?,
        })
    }

    /// Construct a global-only filter.
    pub fn new_global(
        limiter: Arc<GlobalRateLimiter>,
        fallback: FallbackPolicy,
        registry: &prometheus::Registry,
    ) -> Result<Self, prometheus::Error> {
        Ok(Self {
            mode: RateLimitMode::Global,
            local: None,
            global: Some(limiter),
            fallback,
            shadow: false,
            metrics: RateLimitMetrics::new(registry)?,
        })
    }

    /// Construct a hybrid filter (local first, then global).
    pub fn new_hybrid(
        bucket: Arc<LocalTokenBucket>,
        limiter: Arc<GlobalRateLimiter>,
        fallback: FallbackPolicy,
        shadow: bool,
        registry: &prometheus::Registry,
    ) -> Result<Self, prometheus::Error> {
        Ok(Self {
            mode: RateLimitMode::Hybrid,
            local: Some(bucket),
            global: Some(limiter),
            fallback,
            shadow,
            metrics: RateLimitMetrics::new(registry)?,
        })
    }

    /// Check rate limits for the incoming request.
    ///
    /// This method is cancel-safe: it does not hold any lock across `.await`.
    pub async fn check(
        &self,
        req: &HttpRequest,
    ) -> RateLimitDecision {
        let descriptor = extract_descriptor(req);
        let mode_str = mode_label(self.mode);

        match self.mode {
            RateLimitMode::Local => self.check_local(&descriptor, mode_str),
            RateLimitMode::Global => self.check_global(&descriptor, mode_str).await,
            RateLimitMode::Hybrid => {
                // Step 1: local
                if let Some(ref bucket) = self.local {
                    if !bucket.try_acquire(&descriptor, 1) {
                        return self.emit_deny(&descriptor, mode_str, 1);
                    }
                }
                // Step 2: global
                self.check_global(&descriptor, mode_str).await
            }
        }
    }

    // -- private helpers --

    fn check_local(&self, descriptor: &str, mode_str: &str) -> RateLimitDecision {
        let allowed = self
            .local
            .as_ref()
            .map(|b| b.try_acquire(descriptor, 1))
            .unwrap_or(true);

        if allowed {
            self.metrics.record_decision(mode_str, "allow", descriptor);
            debug!(descriptor, "local rate limit: allow");
            RateLimitDecision::Allow
        } else {
            self.emit_deny(descriptor, mode_str, 1)
        }
    }

    async fn check_global(&self, descriptor: &str, mode_str: &str) -> RateLimitDecision {
        let Some(ref global) = self.global else {
            self.metrics.record_decision(mode_str, "allow", descriptor);
            return RateLimitDecision::Allow;
        };

        let t0 = Instant::now();
        let result = global.check(descriptor).await;
        let elapsed = t0.elapsed().as_secs_f64();
        self.metrics.observe_kaya_latency(descriptor, elapsed);

        match result {
            Ok(RateLimitResult::Allowed { .. }) => {
                self.metrics.record_decision(mode_str, "allow", descriptor);
                debug!(descriptor, "global rate limit: allow");
                RateLimitDecision::Allow
            }
            Ok(RateLimitResult::Denied { retry_after_secs }) => {
                self.emit_deny(descriptor, mode_str, retry_after_secs)
            }
            Err(BackendError::BackendUnavailable(ref msg))
            | Err(BackendError::CommandError(ref msg)) => {
                warn!(descriptor, error = %msg, "KAYA rate limit backend error, applying fallback");
                match self.fallback {
                    FallbackPolicy::FailOpen => {
                        self.metrics
                            .record_decision(mode_str, "allow_fallback", descriptor);
                        RateLimitDecision::Allow
                    }
                    FallbackPolicy::FailClosed => {
                        self.emit_deny(descriptor, mode_str, 5)
                    }
                }
            }
        }
    }

    fn emit_deny(&self, descriptor: &str, mode_str: &str, retry_after_secs: u64) -> RateLimitDecision {
        if self.shadow {
            self.metrics.record_decision(mode_str, "shadow", descriptor);
            debug!(descriptor, "rate limit: shadow (over-limit but forwarding)");
            RateLimitDecision::Shadow { retry_after_secs }
        } else {
            self.metrics.record_decision(mode_str, "deny", descriptor);
            debug!(descriptor, retry_after_secs, "rate limit: deny");
            RateLimitDecision::Deny { retry_after_secs }
        }
    }
}

// ── descriptor extraction ─────────────────────────────────────────────────────

/// Build a flat descriptor string from an `HttpRequest`.
///
/// Format: `"tenant:<id>/route:<path>"`.
/// Falls back to `"route:<path>"` when X-Tenant-Id is absent.
fn extract_descriptor(req: &HttpRequest) -> String {
    let tenant = req
        .headers
        .get("x-tenant-id")
        .or_else(|| req.headers.get("X-Tenant-Id"))
        .map(|t| format!("tenant:{}/", t))
        .unwrap_or_default();

    format!("{}route:{}", tenant, req.path)
}

fn mode_label(mode: RateLimitMode) -> &'static str {
    match mode {
        RateLimitMode::Local => "local",
        RateLimitMode::Global => "global",
        RateLimitMode::Hybrid => "hybrid",
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global::{GlobalRuleConfig, MockRateLimitBackend};
    use armageddon_common::types::HttpVersion;
    use prometheus::Registry;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_req(path: &str, tenant: Option<&str>) -> HttpRequest {
        let mut headers = HashMap::new();
        if let Some(t) = tenant {
            headers.insert("x-tenant-id".to_string(), t.to_string());
        }
        HttpRequest {
            method: "GET".to_string(),
            uri: path.to_string(),
            path: path.to_string(),
            query: None,
            headers,
            body: None,
            version: HttpVersion::Http11,
        }
    }

    fn fresh_registry() -> Registry {
        Registry::new()
    }

    // -- local filter --

    #[tokio::test]
    async fn test_local_filter_allows_within_limit() {
        let bucket = Arc::new(LocalTokenBucket::new());
        bucket.add_rule("route:/api/test", 10, 10);
        let filter = RateLimitFilter::new_local(bucket, &fresh_registry()).unwrap();
        let req = make_req("/api/test", None);
        assert_eq!(filter.check(&req).await, RateLimitDecision::Allow);
    }

    #[tokio::test]
    async fn test_local_filter_denies_exhausted() {
        let bucket = Arc::new(LocalTokenBucket::new());
        bucket.add_rule("route:/api/test", 1, 1);
        let filter = RateLimitFilter::new_local(Arc::clone(&bucket), &fresh_registry()).unwrap();
        let req = make_req("/api/test", None);

        // Drain
        let _ = filter.check(&req).await;
        let decision = filter.check(&req).await;
        assert!(matches!(decision, RateLimitDecision::Deny { .. }));
    }

    // -- global filter with mock --

    #[tokio::test]
    async fn test_global_filter_allows_within_limit() {
        let backend = Arc::new(MockRateLimitBackend::new());
        let mut limiter = GlobalRateLimiter::new(Arc::clone(&backend) as Arc<dyn crate::global::RateLimitBackend>);
        limiter.add_rule("route:/api/test", GlobalRuleConfig { requests_per_window: 5, window_secs: 60 });
        let filter = RateLimitFilter::new_global(Arc::new(limiter), FallbackPolicy::FailOpen, &fresh_registry()).unwrap();
        let req = make_req("/api/test", None);
        assert_eq!(filter.check(&req).await, RateLimitDecision::Allow);
    }

    #[tokio::test]
    async fn test_global_filter_fail_open_on_kaya_error() {
        let backend = Arc::new(MockRateLimitBackend::new());
        *backend.inject_error.lock() = Some("kaya down".to_string());
        let mut limiter = GlobalRateLimiter::new(Arc::clone(&backend) as Arc<dyn crate::global::RateLimitBackend>);
        limiter.add_rule("route:/api/test", GlobalRuleConfig { requests_per_window: 5, window_secs: 60 });
        let filter = RateLimitFilter::new_global(Arc::new(limiter), FallbackPolicy::FailOpen, &fresh_registry()).unwrap();
        let req = make_req("/api/test", None);
        // FailOpen: despite backend error, must allow
        assert_eq!(filter.check(&req).await, RateLimitDecision::Allow);
    }

    #[tokio::test]
    async fn test_global_filter_fail_closed_on_kaya_error() {
        let backend = Arc::new(MockRateLimitBackend::new());
        *backend.inject_error.lock() = Some("kaya down".to_string());
        let mut limiter = GlobalRateLimiter::new(Arc::clone(&backend) as Arc<dyn crate::global::RateLimitBackend>);
        limiter.add_rule("route:/api/test", GlobalRuleConfig { requests_per_window: 5, window_secs: 60 });
        let filter = RateLimitFilter::new_global(Arc::new(limiter), FallbackPolicy::FailClosed, &fresh_registry()).unwrap();
        let req = make_req("/api/test", None);
        // FailClosed: backend error → deny
        assert!(matches!(filter.check(&req).await, RateLimitDecision::Deny { .. }));
    }

    // -- shadow mode --

    #[tokio::test]
    async fn test_shadow_mode_returns_shadow_not_deny() {
        let bucket = Arc::new(LocalTokenBucket::new());
        bucket.add_rule("route:/api/test", 1, 1); // burst 1
        let backend = Arc::new(MockRateLimitBackend::new());
        let limiter = GlobalRateLimiter::new(Arc::clone(&backend) as Arc<dyn crate::global::RateLimitBackend>);
        let filter = RateLimitFilter::new_hybrid(
            Arc::clone(&bucket),
            Arc::new(limiter),
            FallbackPolicy::FailOpen,
            true, // shadow = true
            &fresh_registry(),
        ).unwrap();

        let req = make_req("/api/test", None);
        let _ = filter.check(&req).await; // consume burst
        let decision = filter.check(&req).await;
        assert!(matches!(decision, RateLimitDecision::Shadow { .. }), "expected Shadow, got {:?}", decision);
    }

    // -- descriptor extraction --

    #[test]
    fn test_descriptor_with_tenant() {
        let req = make_req("/api/v1/poulets", Some("acme"));
        let desc = extract_descriptor(&req);
        assert_eq!(desc, "tenant:acme/route:/api/v1/poulets");
    }

    #[test]
    fn test_descriptor_without_tenant() {
        let req = make_req("/api/v1/poulets", None);
        let desc = extract_descriptor(&req);
        assert_eq!(desc, "route:/api/v1/poulets");
    }
}
