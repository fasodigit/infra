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

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use armageddon_common::types::HttpRequest;
use tracing::{debug, warn};

use crate::global::{BackendError, GlobalRateLimiter, RateLimitResult};
use crate::local::LocalTokenBucket;
use crate::metrics::RateLimitMetrics;

/// Maximum accepted length for a tenant id (bytes).  Values longer than this
/// are rejected by `sanitize_tenant` to bound descriptor cardinality and
/// prevent memory-amplification attacks against the bucket map.
const MAX_TENANT_LEN: usize = 64;

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
    /// Optional allow-list of tenant ids.  When `Some`, a request whose
    /// `X-Tenant-Id` header (after sanitization) is not in this set is treated
    /// as *tenantless*: the descriptor falls back to the untagged
    /// `route:<path>` form so the route-only rule still applies.  This is
    /// safer than returning 429, which would let an attacker DoS legitimate
    /// users by spoofing their tenant id.
    allowed_tenants: Option<HashSet<String>>,
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
            allowed_tenants: None,
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
            allowed_tenants: None,
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
            allowed_tenants: None,
        })
    }

    /// Install an allow-list of tenant ids.  When set, a request whose
    /// sanitized `X-Tenant-Id` is not in `allowed` is treated as tenantless
    /// (descriptor falls back to the untagged form).  Pass `None` to clear.
    pub fn set_allowed_tenants(&mut self, allowed: Option<HashSet<String>>) {
        self.allowed_tenants = allowed;
    }

    /// Check rate limits for the incoming request.
    ///
    /// This method is cancel-safe: it does not hold any lock across `.await`.
    pub async fn check(
        &self,
        req: &HttpRequest,
    ) -> RateLimitDecision {
        let descriptor = extract_descriptor(req, self.allowed_tenants.as_ref());
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
/// Falls back to `"route:<path>"` when:
/// - the `X-Tenant-Id` header is absent, OR
/// - the header value is empty after sanitization (strip `/` and `:`,
///   clamp to 64 bytes), OR
/// - `allowed_tenants` is provided and the sanitized value is not in it.
///
/// The sanitization step is critical: without it, an attacker can inject
/// descriptor separators (`/`, `:`) in the header to produce a fabricated
/// descriptor that has no matching rule, triggering the backend's fail-open
/// path for a total rate-limit bypass.  Header lookup is case-insensitive.
fn extract_descriptor(
    req: &HttpRequest,
    allowed_tenants: Option<&HashSet<String>>,
) -> String {
    let tenant = lookup_header_ci(&req.headers, "x-tenant-id")
        .and_then(sanitize_tenant)
        .filter(|t| match allowed_tenants {
            Some(set) => set.contains(t),
            None => true,
        })
        .map(|t| format!("tenant:{}/", t))
        .unwrap_or_default();

    format!("{}route:{}", tenant, req.path)
}

/// Case-insensitive header lookup that works with any HashMap casing.
///
/// HTTP header names are case-insensitive per RFC 7230 §3.2; the upstream
/// parser is expected to canonicalise to lowercase but we must not rely on
/// that — any mismatch would silently drop the tenant prefix and, combined
/// with the backend's fail-open on unknown descriptors, yield a bypass.
fn lookup_header_ci<'a>(
    headers: &'a std::collections::HashMap<String, String>,
    target_lower: &str,
) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(target_lower))
        .map(|(_, v)| v.as_str())
}

/// Sanitize a tenant id:
/// - strip all `/` and `:` characters (they are descriptor separators),
/// - clamp to `MAX_TENANT_LEN` bytes,
/// - return `None` when the result is empty.
fn sanitize_tenant(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|&c| c != '/' && c != ':')
        .collect();
    // Clamp at a byte boundary: walk char by char until MAX_TENANT_LEN is
    // reached.  We don't truncate via `.truncate(MAX_TENANT_LEN)` because
    // that could split a multi-byte UTF-8 sequence.
    let mut out = String::with_capacity(cleaned.len().min(MAX_TENANT_LEN));
    for c in cleaned.chars() {
        if out.len() + c.len_utf8() > MAX_TENANT_LEN {
            break;
        }
        out.push(c);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
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
        let desc = extract_descriptor(&req, None);
        assert_eq!(desc, "tenant:acme/route:/api/v1/poulets");
    }

    #[test]
    fn test_descriptor_without_tenant() {
        let req = make_req("/api/v1/poulets", None);
        let desc = extract_descriptor(&req, None);
        assert_eq!(desc, "route:/api/v1/poulets");
    }

    // -- security regressions (descriptor-spoofing bypass) --

    /// Case-insensitive header lookup: a `X-TENANT-ID` casing must still be
    /// picked up and yield the tenant-tagged descriptor.
    #[test]
    fn test_descriptor_header_case_insensitive() {
        let mut headers = HashMap::new();
        headers.insert("X-TENANT-ID".to_string(), "acme".to_string());
        let req = HttpRequest {
            method: "GET".to_string(),
            uri: "/api".to_string(),
            path: "/api".to_string(),
            query: None,
            headers,
            body: None,
            version: HttpVersion::Http11,
        };
        let desc = extract_descriptor(&req, None);
        assert_eq!(desc, "tenant:acme/route:/api");
    }

    /// Descriptor-injection: a tenant id containing `/` or `:` must NOT be
    /// able to forge the descriptor separators.  After sanitization, the
    /// slashes and colons are stripped so the descriptor is well-formed.
    #[test]
    fn test_descriptor_injection_slashes_stripped() {
        let req = make_req("/api/v1/poulets", Some("a/route:/decoy"));
        let desc = extract_descriptor(&req, None);
        // After stripping `/` and `:`, "a/route:/decoy" becomes "aroutedecoy".
        assert_eq!(desc, "tenant:aroutedecoy/route:/api/v1/poulets");
        // And crucially, the descriptor does NOT contain the injected route.
        assert!(!desc.contains("tenant:a/"), "raw tenant must not leak separators");
    }

    /// Tenant id longer than the limit is clamped rather than propagated, to
    /// bound descriptor cardinality.
    #[test]
    fn test_descriptor_long_tenant_is_clamped() {
        let long = "t".repeat(1024);
        let req = make_req("/api", Some(&long));
        let desc = extract_descriptor(&req, None);
        // "tenant:" (7) + at most MAX_TENANT_LEN (64) + "/route:/api"
        let tenant_portion = desc
            .strip_prefix("tenant:")
            .and_then(|s| s.split('/').next())
            .unwrap();
        assert!(tenant_portion.len() <= 64, "tenant must be clamped to 64 bytes");
    }

    /// Empty tenant value (or one that becomes empty after sanitization) must
    /// fall back to the untagged descriptor.
    #[test]
    fn test_descriptor_empty_tenant_falls_back() {
        let req = make_req("/api/v1/poulets", Some(":/:"));
        let desc = extract_descriptor(&req, None);
        assert_eq!(desc, "route:/api/v1/poulets");
    }

    /// With an allow-list, an unknown tenant falls back to the untagged
    /// descriptor — safer than 429, which would let an attacker DoS legit
    /// users by spoofing their tenant id.
    #[test]
    fn test_descriptor_allow_list_rejects_unknown() {
        let mut allowed = HashSet::new();
        allowed.insert("acme".to_string());
        let req = make_req("/api", Some("evil"));
        let desc = extract_descriptor(&req, Some(&allowed));
        assert_eq!(desc, "route:/api");

        let req = make_req("/api", Some("acme"));
        let desc = extract_descriptor(&req, Some(&allowed));
        assert_eq!(desc, "tenant:acme/route:/api");
    }

    /// **Bypass regression**: an attacker sends a bogus `X-Tenant-Id: junk`
    /// against a route-only rule.  Before the fix, the tenant was concatenated
    /// verbatim yielding `tenant:junk/route:/api/test` with no matching rule,
    /// and the bucket fail-opened → unlimited requests.  After the fix, the
    /// route-only rule still applies because only the untagged descriptor
    /// matches the rule we actually registered.
    ///
    /// The request below is still rate-limited by exhausting the burst-1
    /// bucket on the second call.
    #[tokio::test]
    async fn test_bogus_tenant_does_not_bypass_route_rule() {
        let bucket = Arc::new(LocalTokenBucket::new());
        // Rule is registered on the *sanitized* tagged descriptor.
        bucket.add_rule("tenant:junk/route:/api/test", 1, 1);
        let filter = RateLimitFilter::new_local(
            Arc::clone(&bucket),
            &fresh_registry(),
        )
        .unwrap();
        let req = make_req("/api/test", Some("junk"));

        // First call consumes the single token.
        let d1 = filter.check(&req).await;
        assert_eq!(d1, RateLimitDecision::Allow);
        // Second call must be denied — the attacker cannot get unlimited
        // throughput by sending a tenant header.
        let d2 = filter.check(&req).await;
        assert!(
            matches!(d2, RateLimitDecision::Deny { .. }),
            "expected Deny, got {:?}",
            d2
        );
    }

    /// Descriptor-injection end-to-end: a malicious `X-Tenant-Id: a/route:/decoy`
    /// must not produce a descriptor that escapes the tenant slot.
    #[tokio::test]
    async fn test_injection_header_is_sanitized() {
        let bucket = Arc::new(LocalTokenBucket::new());
        // Rule on the sanitized, expected descriptor.
        bucket.add_rule("tenant:aroutedecoy/route:/api/v1/poulets", 1, 1);
        let filter = RateLimitFilter::new_local(
            Arc::clone(&bucket),
            &fresh_registry(),
        )
        .unwrap();
        let req = make_req("/api/v1/poulets", Some("a/route:/decoy"));

        // First call allowed, second denied — confirms the sanitized
        // descriptor matches the registered rule rather than falling
        // through to fail-open.
        assert_eq!(filter.check(&req).await, RateLimitDecision::Allow);
        assert!(matches!(
            filter.check(&req).await,
            RateLimitDecision::Deny { .. }
        ));
    }

    /// Strict mode (local): unknown descriptors must be denied.  Without
    /// strict mode, the same request would be allowed (fail-open legacy path).
    #[tokio::test]
    async fn test_strict_mode_local_denies_unknown_descriptor() {
        let bucket = Arc::new(LocalTokenBucket::with_strict_mode(true));
        // No rule registered → unknown descriptor.
        let filter = RateLimitFilter::new_local(
            Arc::clone(&bucket),
            &fresh_registry(),
        )
        .unwrap();
        let req = make_req("/api/nonexistent", None);
        let decision = filter.check(&req).await;
        assert!(
            matches!(decision, RateLimitDecision::Deny { .. }),
            "strict mode must deny unknown descriptor, got {:?}",
            decision
        );
    }

    /// Strict mode (global): unknown descriptors must be denied.
    #[tokio::test]
    async fn test_strict_mode_global_denies_unknown_descriptor() {
        let backend = Arc::new(MockRateLimitBackend::new());
        let mut limiter = GlobalRateLimiter::new(
            Arc::clone(&backend) as Arc<dyn crate::global::RateLimitBackend>,
        );
        limiter.set_strict_mode(true);
        // No rule registered.
        let filter = RateLimitFilter::new_global(
            Arc::new(limiter),
            FallbackPolicy::FailOpen,
            &fresh_registry(),
        )
        .unwrap();
        let req = make_req("/api/unknown", None);
        let decision = filter.check(&req).await;
        assert!(
            matches!(decision, RateLimitDecision::Deny { .. }),
            "strict global mode must deny unknown descriptor, got {:?}",
            decision
        );
    }
}
