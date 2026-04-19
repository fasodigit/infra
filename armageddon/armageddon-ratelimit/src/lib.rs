// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # armageddon-ratelimit
//!
//! Rate limiting subsystem for ARMAGEDDON, providing feature parity with
//! `envoy.filters.http.local_ratelimit` (token-bucket per instance) and
//! `envoy.filters.http.ratelimit` (global sliding-window counter via KAYA).
//!
//! ## Architecture
//!
//! ```text
//!   HTTP request
//!       │
//!       ▼
//!  RateLimitFilter ──► LocalTokenBucket  (per-process, AtomicU64)
//!       │
//!       └──────────► GlobalRateLimiter  (INCR+EXPIRE on KAYA)
//!                         │
//!                         └── RateLimitBackend (trait, mockable)
//! ```
//!
//! ## Decision enum
//!
//! - `Allow`        — request is within limits.
//! - `Deny(u64)`    — over-limit; the `u64` is `retry_after` in seconds.
//! - `Shadow`       — over-limit but passthrough (dry-run / canary mode).
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | KAYA unreachable | Governed by `RateLimitConfig::fallback`; `FailOpen` ⇒ Allow, `FailClosed` ⇒ Deny |
//! | Local bucket exhausted | Always Deny immediately (no network hop) |
//! | Hybrid mode KAYA lag > threshold | Falls back to local decision |
//!
//! ## Metrics
//!
//! - `armageddon_ratelimit_decisions_total{mode, decision, descriptor}`
//! - `armageddon_ratelimit_kaya_latency_seconds{descriptor}`

pub mod filter;
pub mod global;
pub mod local;
pub mod metrics;

pub use filter::{RateLimitFilter, RateLimitDecision};
pub use global::{GlobalRateLimiter, KayaRateLimitBackend, MockRateLimitBackend, RateLimitBackend};
pub use local::LocalTokenBucket;
pub use metrics::RateLimitMetrics;
