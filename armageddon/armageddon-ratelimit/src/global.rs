// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Distributed sliding-window rate limiter backed by KAYA (Redis-compatible).
//!
//! # Algorithm
//!
//! Uses KAYA's `INCR` + `EXPIRE` pipeline, which provides an atomic increment
//! with TTL refresh.  The key is:
//!
//! ```text
//! armageddon:ratelimit:<descriptor>:<window_epoch>
//! ```
//!
//! where `window_epoch = now_secs / window_secs` forms a fixed tumbling window.
//! This is **not** a true sliding window (that would require a sorted set), but
//! it is consistent with how Envoy's global rate limit service behaves under the
//! default `RATE_LIMIT` action — requests within the same window share a counter.
//!
//! A true GCRA sliding window can be layered on top later using the same trait.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | KAYA unreachable | `check()` returns `Err(BackendUnavailable)` |
//! | KAYA latency spike | `check()` times out; caller applies fallback policy |
//! | Counter exceeds limit | Returns `Ok(RateLimitResult::Denied { remaining_secs })` |
//!
//! # Trait design
//!
//! `RateLimitBackend` is the seam for unit tests and future alternative
//! backends (e.g. a local in-memory counter for single-instance deployments).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use thiserror::Error;

use armageddon_nexus::kaya::KayaClient;

// ── errors ────────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("KAYA backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("KAYA command error: {0}")]
    CommandError(String),
}

// ── result ────────────────────────────────────────────────────────────────────

/// Outcome of a single rate limit check against the global backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request is within limits; `count` is the current window count.
    Allowed { count: u64 },
    /// Request exceeds limit; `retry_after_secs` is the window reset time.
    Denied { retry_after_secs: u64 },
}

// ── trait ─────────────────────────────────────────────────────────────────────

/// Abstraction over the distributed counter backend.
///
/// The real implementation wraps `KayaClient`; tests use `MockRateLimitBackend`.
#[async_trait]
pub trait RateLimitBackend: Send + Sync {
    /// Increment the counter for `descriptor` and return the new count.
    ///
    /// `window_secs` is the TTL for the tumbling window key.
    async fn increment(&self, descriptor: &str, window_secs: u64)
        -> Result<u64, BackendError>;
}

// ── KAYA implementation ───────────────────────────────────────────────────────

/// Production backend using `KayaClient::incr_rate_limit`.
pub struct KayaRateLimitBackend {
    client: Arc<KayaClient>,
}

impl KayaRateLimitBackend {
    pub fn new(client: Arc<KayaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl RateLimitBackend for KayaRateLimitBackend {
    async fn increment(
        &self,
        descriptor: &str,
        window_secs: u64,
    ) -> Result<u64, BackendError> {
        // Build a window-scoped key: descriptor + current tumbling window epoch.
        let window_epoch = now_secs() / window_secs.max(1);
        let scoped_key = format!("{}:{}", descriptor, window_epoch);

        self.client
            .incr_rate_limit(&scoped_key, window_secs)
            .await
            .map_err(|e| BackendError::CommandError(e.to_string()))
    }
}

// ── mock backend (for tests) ──────────────────────────────────────────────────

/// In-memory mock backend for unit tests — no KAYA required.
///
/// Each descriptor maps to a simple u64 counter.  There is no TTL logic;
/// tests reset state by constructing a new instance.
#[derive(Default)]
pub struct MockRateLimitBackend {
    counters: parking_lot::Mutex<HashMap<String, u64>>,
    /// If `Some(err_msg)`, every `increment` call returns `Err`.
    pub inject_error: parking_lot::Mutex<Option<String>>,
}

impl MockRateLimitBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-seed a counter so tests can start from a specific state.
    pub fn set_count(&self, descriptor: &str, count: u64) {
        self.counters.lock().insert(descriptor.to_string(), count);
    }

    /// Get current count (test helper).
    pub fn get_count(&self, descriptor: &str) -> u64 {
        *self.counters.lock().get(descriptor).unwrap_or(&0)
    }
}

#[async_trait]
impl RateLimitBackend for MockRateLimitBackend {
    async fn increment(
        &self,
        descriptor: &str,
        _window_secs: u64,
    ) -> Result<u64, BackendError> {
        if let Some(ref msg) = *self.inject_error.lock() {
            return Err(BackendError::BackendUnavailable(msg.clone()));
        }
        let mut map = self.counters.lock();
        let entry = map.entry(descriptor.to_string()).or_insert(0);
        *entry += 1;
        Ok(*entry)
    }
}

// ── GlobalRateLimiter ─────────────────────────────────────────────────────────

/// Rule for a single descriptor.
#[derive(Debug, Clone)]
pub struct GlobalRuleConfig {
    /// Maximum number of requests per `window_secs`.
    pub requests_per_window: u64,
    /// Window size in seconds.
    pub window_secs: u64,
}

/// Distributed rate limiter using a pluggable `RateLimitBackend`.
///
/// # Example
/// ```rust,ignore
/// use armageddon_ratelimit::global::{GlobalRateLimiter, GlobalRuleConfig, MockRateLimitBackend, RateLimitResult};
/// use std::sync::Arc;
///
/// let backend = Arc::new(MockRateLimitBackend::new());
/// let mut limiter = GlobalRateLimiter::new(backend);
/// limiter.add_rule("tenant:acme", GlobalRuleConfig { requests_per_window: 100, window_secs: 60 });
///
/// // tokio runtime required for check()
/// ```
pub struct GlobalRateLimiter {
    backend: Arc<dyn RateLimitBackend>,
    rules: HashMap<String, GlobalRuleConfig>,
    /// When `true`, unknown descriptors are denied (fail-closed).  Default
    /// `false` for backwards compatibility.  Strict mode closes the
    /// descriptor-spoofing bypass: a crafted `X-Tenant-Id` that yields a key
    /// with no matching rule no longer defaults to Allowed.
    strict_mode: bool,
}

impl GlobalRateLimiter {
    pub fn new(backend: Arc<dyn RateLimitBackend>) -> Self {
        Self {
            backend,
            rules: HashMap::new(),
            strict_mode: false,
        }
    }

    /// Register a rate limit rule for `descriptor`.
    pub fn add_rule(&mut self, descriptor: &str, rule: GlobalRuleConfig) {
        self.rules.insert(descriptor.to_string(), rule);
    }

    /// Toggle strict mode.  In strict mode, `check()` on an unknown descriptor
    /// returns `Denied` rather than `Allowed`.  Enable once all expected
    /// descriptors have been declared.
    pub fn set_strict_mode(&mut self, strict: bool) {
        self.strict_mode = strict;
    }

    /// Whether the limiter operates in strict (fail-closed) mode.
    pub fn is_strict(&self) -> bool {
        self.strict_mode
    }

    /// Check and increment the counter for `descriptor`.
    ///
    /// Returns:
    /// - `Ok(RateLimitResult::Allowed)` — counter was below limit.
    /// - `Ok(RateLimitResult::Denied)`  — counter exceeded limit, or descriptor
    ///   unknown and strict mode is enabled.
    /// - `Err(BackendError)` — KAYA unreachable; caller applies fallback.
    pub async fn check(
        &self,
        descriptor: &str,
    ) -> Result<RateLimitResult, BackendError> {
        let rule = match self.rules.get(descriptor) {
            None => {
                // Unknown descriptor: strict → deny, otherwise allow (legacy).
                if self.strict_mode {
                    return Ok(RateLimitResult::Denied { retry_after_secs: 1 });
                }
                return Ok(RateLimitResult::Allowed { count: 0 });
            }
            Some(r) => r,
        };

        let count = self.backend.increment(descriptor, rule.window_secs).await?;

        if count <= rule.requests_per_window {
            Ok(RateLimitResult::Allowed { count })
        } else {
            // Retry-after = remaining seconds in current window.
            let window_epoch = now_secs() / rule.window_secs.max(1);
            let window_end = (window_epoch + 1) * rule.window_secs;
            let retry_after = window_end.saturating_sub(now_secs()).max(1);
            Ok(RateLimitResult::Denied { retry_after_secs: retry_after })
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_limiter(rps: u64, window: u64) -> (GlobalRateLimiter, Arc<MockRateLimitBackend>) {
        let backend = Arc::new(MockRateLimitBackend::new());
        let mut limiter = GlobalRateLimiter::new(Arc::clone(&backend) as Arc<dyn RateLimitBackend>);
        limiter.add_rule("desc", GlobalRuleConfig { requests_per_window: rps, window_secs: window });
        (limiter, backend)
    }

    #[tokio::test]
    async fn test_allows_within_limit() {
        let (limiter, _) = make_limiter(3, 60);
        for _ in 0..3 {
            let r = limiter.check("desc").await.unwrap();
            assert!(matches!(r, RateLimitResult::Allowed { .. }));
        }
    }

    #[tokio::test]
    async fn test_denies_over_limit() {
        let (limiter, _) = make_limiter(2, 60);
        let _ = limiter.check("desc").await.unwrap();
        let _ = limiter.check("desc").await.unwrap();
        let r = limiter.check("desc").await.unwrap();
        assert!(matches!(r, RateLimitResult::Denied { .. }));
    }

    #[tokio::test]
    async fn test_unknown_descriptor_allows() {
        let (limiter, _) = make_limiter(5, 60);
        let r = limiter.check("unknown").await.unwrap();
        assert!(matches!(r, RateLimitResult::Allowed { count: 0 }));
    }

    #[tokio::test]
    async fn test_unknown_descriptor_denies_in_strict_mode() {
        let (mut limiter, _) = make_limiter(5, 60);
        limiter.set_strict_mode(true);
        let r = limiter.check("unknown").await.unwrap();
        assert!(
            matches!(r, RateLimitResult::Denied { .. }),
            "strict mode must deny unknown descriptor, got {:?}",
            r
        );
    }

    #[tokio::test]
    async fn test_fallback_on_backend_error() {
        let (limiter, backend) = make_limiter(10, 60);
        *backend.inject_error.lock() = Some("KAYA down".to_string());
        let r = limiter.check("desc").await;
        assert!(r.is_err(), "backend error must propagate so caller can apply fallback");
    }

    #[tokio::test]
    async fn test_pre_seeded_counter_starts_denied() {
        let (limiter, backend) = make_limiter(5, 60);
        // Seed counter at limit so next call exceeds it.
        backend.set_count("desc", 5);
        let r = limiter.check("desc").await.unwrap();
        assert!(matches!(r, RateLimitResult::Denied { .. }));
    }

    #[tokio::test]
    async fn test_retry_after_is_positive() {
        let (limiter, _) = make_limiter(1, 60);
        let _ = limiter.check("desc").await.unwrap(); // count = 1, allowed
        let r = limiter.check("desc").await.unwrap(); // count = 2, denied
        if let RateLimitResult::Denied { retry_after_secs } = r {
            assert!(retry_after_secs >= 1, "retry_after must be at least 1 second");
            assert!(retry_after_secs <= 60, "retry_after must be within the window");
        } else {
            panic!("expected Denied");
        }
    }
}
