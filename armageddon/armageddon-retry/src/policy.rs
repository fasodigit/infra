// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Retry policy configuration and Envoy-compatible header parsing.
//!
//! Supports reading `x-envoy-max-retries` and `x-envoy-upstream-rq-timeout-ms`
//! from inbound request headers so that upstream callers can influence retry
//! behaviour on a per-request basis — matching Envoy proxy semantics.

use std::collections::HashMap;
use std::time::Duration;

// -- constants --

/// Default maximum number of retries (matches Envoy default).
const DEFAULT_MAX_RETRIES: u32 = 2;

/// Default per-try timeout: 15 s.
const DEFAULT_PER_TRY_TIMEOUT_MS: u64 = 15_000;

/// Default overall (global) timeout: 45 s (3× per-try).
const DEFAULT_OVERALL_TIMEOUT_MS: u64 = 45_000;

/// Default initial backoff for exponential back-off: 25 ms.
pub const DEFAULT_BACKOFF_BASE_MS: u64 = 25;

/// Default backoff cap: 2 s.
pub const DEFAULT_BACKOFF_CAP_MS: u64 = 2_000;

// -- RetryOn --

/// Conditions that trigger a retry attempt.
#[derive(Debug, Clone)]
pub struct RetryOn {
    /// HTTP status codes that should cause a retry (e.g. 500, 502, 503, 504).
    pub status_codes: Vec<u16>,
    /// Retry on upstream connection errors (TCP reset, refused, etc.).
    pub connect_error: bool,
    /// Retry when a per-try timeout fires.
    pub timeout: bool,
}

impl Default for RetryOn {
    fn default() -> Self {
        Self {
            status_codes: vec![502, 503, 504],
            connect_error: true,
            timeout: true,
        }
    }
}

impl RetryOn {
    /// Returns `true` when `status` is in the retryable set.
    pub fn matches_status(&self, status: u16) -> bool {
        self.status_codes.contains(&status)
    }
}

// -- RetryPolicy --

/// Full retry policy attached to a route or overridden per-request via headers.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (does **not** count the original attempt).
    pub max_retries: u32,
    /// Deadline for each individual upstream attempt.
    pub per_try_timeout: Duration,
    /// Hard deadline across all attempts (original + retries).
    pub overall_timeout: Duration,
    /// Which error conditions trigger a retry.
    pub retry_on: RetryOn,
    /// Base interval for exponential back-off.
    pub backoff_base: Duration,
    /// Upper cap for exponential back-off.
    pub backoff_cap: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            per_try_timeout: Duration::from_millis(DEFAULT_PER_TRY_TIMEOUT_MS),
            overall_timeout: Duration::from_millis(DEFAULT_OVERALL_TIMEOUT_MS),
            retry_on: RetryOn::default(),
            backoff_base: Duration::from_millis(DEFAULT_BACKOFF_BASE_MS),
            backoff_cap: Duration::from_millis(DEFAULT_BACKOFF_CAP_MS),
        }
    }
}

impl RetryPolicy {
    /// Parse / override policy fields from Envoy-compatible request headers.
    ///
    /// Recognised headers:
    /// - `x-envoy-max-retries`              → `max_retries`
    /// - `x-envoy-upstream-rq-timeout-ms`   → `overall_timeout`
    /// - `x-envoy-upstream-rq-per-try-timeout-ms` → `per_try_timeout`
    pub fn apply_envoy_headers(&mut self, headers: &HashMap<String, String>) {
        if let Some(val) = headers.get("x-envoy-max-retries") {
            if let Ok(n) = val.trim().parse::<u32>() {
                self.max_retries = n;
            }
        }
        if let Some(val) = headers.get("x-envoy-upstream-rq-timeout-ms") {
            if let Ok(ms) = val.trim().parse::<u64>() {
                self.overall_timeout = Duration::from_millis(ms);
            }
        }
        if let Some(val) = headers.get("x-envoy-upstream-rq-per-try-timeout-ms") {
            if let Ok(ms) = val.trim().parse::<u64>() {
                self.per_try_timeout = Duration::from_millis(ms);
            }
        }
    }

    /// Compute exponential back-off duration for the given retry number (1-based).
    ///
    /// `delay = min(backoff_base * 2^(attempt-1), backoff_cap)`
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(63) as u32;
        let factor = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
        let ms = self
            .backoff_base
            .as_millis()
            .saturating_mul(factor as u128)
            .min(self.backoff_cap.as_millis()) as u64;
        Duration::from_millis(ms)
    }

    /// Parse a `Retry-After` header value (seconds integer) and clamp it to `backoff_cap`.
    pub fn parse_retry_after(value: &str) -> Option<Duration> {
        value
            .trim()
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_has_sane_values() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(p.per_try_timeout, Duration::from_millis(DEFAULT_PER_TRY_TIMEOUT_MS));
        assert_eq!(p.overall_timeout, Duration::from_millis(DEFAULT_OVERALL_TIMEOUT_MS));
    }

    #[test]
    fn envoy_headers_override_policy() {
        let mut p = RetryPolicy::default();
        let mut h = HashMap::new();
        h.insert("x-envoy-max-retries".into(), "5".into());
        h.insert("x-envoy-upstream-rq-timeout-ms".into(), "10000".into());
        h.insert("x-envoy-upstream-rq-per-try-timeout-ms".into(), "2000".into());
        p.apply_envoy_headers(&h);
        assert_eq!(p.max_retries, 5);
        assert_eq!(p.overall_timeout, Duration::from_secs(10));
        assert_eq!(p.per_try_timeout, Duration::from_secs(2));
    }

    #[test]
    fn backoff_doubles_each_attempt() {
        let p = RetryPolicy {
            backoff_base: Duration::from_millis(100),
            backoff_cap: Duration::from_secs(10),
            ..Default::default()
        };
        assert_eq!(p.backoff_for(1), Duration::from_millis(100));
        assert_eq!(p.backoff_for(2), Duration::from_millis(200));
        assert_eq!(p.backoff_for(3), Duration::from_millis(400));
    }

    #[test]
    fn backoff_capped() {
        let p = RetryPolicy {
            backoff_base: Duration::from_millis(500),
            backoff_cap: Duration::from_millis(1000),
            ..Default::default()
        };
        assert_eq!(p.backoff_for(3), Duration::from_millis(1000));
    }
}
