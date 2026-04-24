// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Shadow-mode runtime for Pingora ↔ hyper parity validation.
//!
//! # Overview
//!
//! When shadow mode is enabled, a fraction (`sample_rate`) of inbound
//! requests are duplicated and sent to a secondary (shadow) backend.  The
//! primary response is **always** returned to the downstream client; the
//! shadow response is compared asynchronously and the result is logged and
//! published as Prometheus metrics.
//!
//! This is gate M5-3 of the Pingora migration.  It implements the runtime
//! described in [`SHADOW-MODE.md`].
//!
//! # Topology
//!
//! ```text
//! inbound request
//!       │
//!       ▼
//!  primary handler (hyper :8080)
//!       │
//!       ├──── 10% sample ──────► shadow handler (pingora :8081) [fire-and-forget]
//!       │                              │
//!       │                         diff-queue (async, non-blocking)
//!       │
//!       ▼
//!  response to client  (ground truth — always primary)
//! ```
//!
//! # Sampling
//!
//! Uses a deterministic blake3 hash of `request_id` so the same request ID
//! always makes the same sampling decision.  Rate is controlled by an
//! `AtomicU32` (`shadow_sample_percent`) so it can be adjusted at runtime
//! without redeploy.
//!
//! Setting `shadow_sample_percent = 0` is the atomic rollback mechanism.
//!
//! # Multi-process note
//!
//! Pingora runs its workers in a separate process via `pingora::Server::run_forever`.
//! Running both hyper and Pingora in the **same process** on different ports
//! would require `tokio::spawn` for the hyper listener alongside Pingora's
//! own scheduler — which is straightforward with the forge bridge runtime.
//!
//! **Alternative (chosen for M5)**: the shadow comparison is implemented
//! fully within the hyper path as a `ForgeFilter`-like middleware that sends
//! a mirrored request to the Pingora listener port via a lightweight HTTP
//! client.  This avoids embedding Pingora's scheduler inside hyper's runtime.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | Shadow target unreachable | `timeout_on_pingora` bucket; primary response unaffected |
//! | Shadow response timeout | Same as above |
//! | Diff queue full (> 4096) | Shadow event dropped; `shadow_diff_dropped_total` incremented |
//! | `sample_percent = 0` | No shadow requests; feature atomically disabled |

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::warn;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Top-level shadow-mode configuration.
///
/// Set in `forge.toml` under `[shadow_mode]`.
#[derive(Debug, Clone)]
pub struct ShadowModeConfig {
    /// Whether shadow mode is active.
    pub enabled: bool,
    /// Port the primary (hyper) listener is bound to.
    pub hyper_port: u16,
    /// Port the shadow (Pingora) listener is bound to.
    pub pingora_port: u16,
    /// Fraction of requests to mirror, expressed as an integer percentage
    /// 0–100.  The live value is stored in an `AtomicU32` for runtime flip.
    pub sample_rate_percent: u32,
    /// Timeout for shadow requests.
    pub shadow_timeout_ms: u64,
}

impl Default for ShadowModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hyper_port: 8080,
            pingora_port: 8081,
            sample_rate_percent: 10,
            shadow_timeout_ms: 5_000,
        }
    }
}

// ---------------------------------------------------------------------------
// MirroredResponse — result of one completed request side
// ---------------------------------------------------------------------------

/// Captured metadata from one completed HTTP response (primary or shadow).
#[derive(Debug, Clone)]
pub struct MirroredResponse {
    /// HTTP status code.
    pub status: u16,
    /// Sorted, lowercased header names and values (infrastructure-stripped).
    pub headers: Vec<(String, Vec<u8>)>,
    /// blake3 hash of the response body.
    pub body_hash: [u8; 32],
    /// Body byte length.
    pub body_len: usize,
    /// When this side finished.
    pub finished_at: Instant,
}

// ---------------------------------------------------------------------------
// DiffBucket — classification of a (primary, shadow) pair
// ---------------------------------------------------------------------------

/// Classification of a shadow comparison pair.
///
/// Ordered: first matching rule wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffBucket {
    /// Pingora slot did not arrive before the 30 s TTL.
    TimeoutOnPingora,
    /// Hyper slot did not arrive (request failed on primary path).
    TimeoutOnHyper,
    /// HTTP status codes differ.
    StatusDiffer,
    /// Response bodies differ (hash or length mismatch).
    BodyDiffer,
    /// Headers differ after normalisation.
    HeaderDiffer,
    /// Streaming response — not comparable.
    Streaming,
    /// Primary and shadow responses are bit-exact (after normalisation).
    Identical,
}

impl DiffBucket {
    /// Classify a pair of completed responses.
    ///
    /// Normalisation strips infrastructure-added headers before comparison.
    pub fn classify(primary: &MirroredResponse, shadow: &MirroredResponse) -> Self {
        if primary.status != shadow.status {
            return Self::StatusDiffer;
        }
        if primary.body_hash != shadow.body_hash || primary.body_len != shadow.body_len {
            return Self::BodyDiffer;
        }
        let p_hdrs = normalise_headers(&primary.headers);
        let s_hdrs = normalise_headers(&shadow.headers);
        if p_hdrs != s_hdrs {
            return Self::HeaderDiffer;
        }
        Self::Identical
    }

    /// String label for Prometheus metric.
    pub fn label(&self) -> &'static str {
        match self {
            Self::TimeoutOnPingora => "timeout_on_pingora",
            Self::TimeoutOnHyper => "timeout_on_hyper",
            Self::StatusDiffer => "status_differ",
            Self::BodyDiffer => "body_differ",
            Self::HeaderDiffer => "header_differ",
            Self::Streaming => "streaming",
            Self::Identical => "identical",
        }
    }
}

// ---------------------------------------------------------------------------
// Header normalisation
// ---------------------------------------------------------------------------

/// Headers that are stripped before comparison — infrastructure-added noise.
const INFRA_HEADERS: &[&str] = &[
    "date",
    "server",
    "x-forge-id",
    "x-forge-via",
    "x-request-id",
];

/// Remove infrastructure headers, sort by name, return a stable list.
fn normalise_headers(headers: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = headers
        .iter()
        .filter(|(name, _)| !INFRA_HEADERS.contains(&name.as_str()))
        .cloned()
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------------------------------------------------------------------
// Sampling decision
// ---------------------------------------------------------------------------

/// Decide whether a request with the given `request_id` should be shadowed.
///
/// Uses a deterministic blake3 hash so retries with the same request ID
/// always produce the same decision.
///
/// `sample_percent` must be in `[0, 100]`.
pub fn should_shadow(request_id: &str, sample_percent: u32) -> bool {
    if sample_percent == 0 {
        return false;
    }
    if sample_percent >= 100 {
        return true;
    }
    let digest = blake3::hash(request_id.as_bytes());
    let bucket = u32::from_le_bytes(digest.as_bytes()[0..4].try_into().unwrap()) % 100;
    bucket < sample_percent
}

// ---------------------------------------------------------------------------
// ShadowSampler — runtime-adjustable sampling controller
// ---------------------------------------------------------------------------

/// Runtime-adjustable shadow sampling controller.
///
/// The `sample_percent` field is an `AtomicU32` so it can be flipped to 0
/// for instant rollback without a service restart.
pub struct ShadowSampler {
    /// Live sample rate 0–100 (inclusive).  Write 0 to disable shadow mode
    /// atomically.
    pub sample_percent: AtomicU32,
    /// Port of the shadow (Pingora) listener.
    pub shadow_port: u16,
    /// Per-shadow-request timeout.
    pub timeout: Duration,
}

impl ShadowSampler {
    /// Create a new sampler from config.
    pub fn new(config: &ShadowModeConfig) -> Arc<Self> {
        Arc::new(Self {
            sample_percent: AtomicU32::new(config.sample_rate_percent.min(100)),
            shadow_port: config.pingora_port,
            timeout: Duration::from_millis(config.shadow_timeout_ms),
        })
    }

    /// Test whether this request should be shadowed.
    pub fn should_shadow(&self, request_id: &str) -> bool {
        let pct = self.sample_percent.load(Ordering::Relaxed);
        should_shadow(request_id, pct)
    }

    /// Set the sample rate atomically.  `percent` is clamped to `[0, 100]`.
    pub fn set_sample_percent(&self, percent: u32) {
        self.sample_percent.store(percent.min(100), Ordering::Relaxed);
    }

    /// Disable shadow mode atomically (sets `sample_percent` to 0).
    pub fn disable(&self) {
        self.set_sample_percent(0);
    }
}

// ---------------------------------------------------------------------------
// ShadowEvent — the result pushed to the diff sink
// ---------------------------------------------------------------------------

/// One completed shadow comparison event.
#[derive(Debug, Clone)]
pub struct ShadowEvent {
    /// RFC 3339 timestamp string.
    pub ts: String,
    /// Unique request identifier (uuid v4).
    pub request_id: String,
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub path: String,
    /// Primary (hyper) status code.
    pub hyper_status: u16,
    /// Shadow (Pingora) status code. `None` on timeout.
    pub pingora_status: Option<u16>,
    /// Bucket classification.
    pub bucket: DiffBucket,
    /// Primary latency in microseconds.
    pub latency_us_hyper: u64,
    /// Shadow latency in microseconds. `None` on timeout.
    pub latency_us_pingora: Option<u64>,
    /// Primary body length.
    pub body_len_hyper: usize,
    /// Shadow body length. `None` on timeout.
    pub body_len_pingora: Option<usize>,
}

// ---------------------------------------------------------------------------
// ShadowDiffQueue — the channel between request hooks and the diff analyser
// ---------------------------------------------------------------------------

/// Bounded channel for `ShadowEvent`s.
///
/// The sender is cloned per-request; the receiver is consumed by the diff
/// analyser task.
pub struct ShadowDiffQueue {
    tx: tokio::sync::mpsc::Sender<ShadowEvent>,
    pub rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<ShadowEvent>>,
}

impl ShadowDiffQueue {
    /// Create a bounded queue with `capacity` slots.
    pub fn new(capacity: usize) -> Arc<Self> {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        Arc::new(Self {
            tx,
            rx: tokio::sync::Mutex::new(rx),
        })
    }

    /// Push an event.  Returns `false` (and increments the drop counter) when
    /// the queue is full.
    pub fn push(&self, event: ShadowEvent) -> bool {
        match self.tx.try_send(event) {
            Ok(()) => true,
            Err(e) => {
                let reason = match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => "full",
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => "closed",
                };
                warn!("shadow diff queue {reason} — dropping event");
                increment_dropped_counter();
                false
            }
        }
    }

    /// Depth of the queue (approximate, for metrics).
    pub fn depth(&self) -> usize {
        self.tx.max_capacity().saturating_sub(self.tx.capacity())
    }
}

// ---------------------------------------------------------------------------
// Metrics (stubs — wired in M6)
// ---------------------------------------------------------------------------

fn increment_dropped_counter() {
    // TODO(M6): wire into Prometheus registry.
    tracing::debug!("armageddon_shadow_diff_dropped_total += 1");
}

/// Increment the `shadow_requests_total{outcome}` counter.
pub fn record_shadow_outcome(bucket: DiffBucket) {
    // TODO(M6): wire into Prometheus registry.
    tracing::debug!(outcome = bucket.label(), "armageddon_shadow_requests_total += 1");
}

/// Increment `shadow_requests_sampled_total`.
pub fn record_shadow_sampled() {
    tracing::debug!("armageddon_shadow_requests_sampled_total += 1");
}

/// Increment `shadow_requests_skipped_total{reason}`.
pub fn record_shadow_skipped(reason: &str) {
    tracing::debug!(
        reason,
        "armageddon_shadow_requests_skipped_total += 1"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── should_shadow ──────────────────────────────────────────────────────

    /// With 0% rate, no request is ever shadowed.
    #[test]
    fn shadow_rate_zero_never_shadows() {
        for i in 0..200u32 {
            assert!(
                !should_shadow(&format!("req-{i}"), 0),
                "0% must never shadow"
            );
        }
    }

    /// With 100% rate, every request is shadowed.
    #[test]
    fn shadow_rate_hundred_always_shadows() {
        for i in 0..200u32 {
            assert!(
                should_shadow(&format!("req-{i}"), 100),
                "100% must always shadow"
            );
        }
    }

    /// With 50% rate, approximately half of a large sample is shadowed.
    #[test]
    fn shadow_rate_fifty_is_approximately_half() {
        let total = 10_000u32;
        let shadowed = (0..total)
            .filter(|i| should_shadow(&format!("req-{i}"), 50))
            .count();
        // 50% ± 5% is acceptable for a hash-based sampler.
        let pct = shadowed as f64 / total as f64 * 100.0;
        assert!(
            (45.0..=55.0).contains(&pct),
            "50% sample rate should produce 45–55% shadows, got {pct:.1}%"
        );
    }

    /// With 10% rate, approximately 10% of a large sample is shadowed.
    #[test]
    fn shadow_rate_ten_percent_respected() {
        let total = 10_000u32;
        let shadowed = (0..total)
            .filter(|i| should_shadow(&format!("uid-{i}"), 10))
            .count();
        let pct = shadowed as f64 / total as f64 * 100.0;
        assert!(
            (7.0..=13.0).contains(&pct),
            "10% sample rate should produce 7–13% shadows, got {pct:.1}%"
        );
    }

    /// Sampling is deterministic: same request ID → same decision.
    #[test]
    fn shadow_sampling_is_deterministic() {
        for i in 0..100u32 {
            let id = format!("deterministic-{i}");
            let d1 = should_shadow(&id, 30);
            let d2 = should_shadow(&id, 30);
            assert_eq!(d1, d2, "same request_id must always produce same decision");
        }
    }

    // ── DiffBucket::classify ───────────────────────────────────────────────

    fn make_resp(status: u16, body: &[u8], headers: Vec<(&str, &[u8])>) -> MirroredResponse {
        let headers = headers
            .into_iter()
            .map(|(k, v)| (k.to_lowercase(), v.to_vec()))
            .collect();
        let body_hash = *blake3::hash(body).as_bytes();
        MirroredResponse {
            status,
            headers,
            body_hash,
            body_len: body.len(),
            finished_at: Instant::now(),
        }
    }

    #[test]
    fn classify_identical_responses() {
        let body = b"hello world";
        let a = make_resp(200, body, vec![("content-type", b"text/plain")]);
        let b = make_resp(200, body, vec![("content-type", b"text/plain")]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::Identical);
    }

    #[test]
    fn classify_status_differ() {
        let body = b"error";
        let a = make_resp(200, body, vec![]);
        let b = make_resp(500, body, vec![]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::StatusDiffer);
    }

    #[test]
    fn classify_body_differ() {
        let a = make_resp(200, b"primary response", vec![]);
        let b = make_resp(200, b"shadow response", vec![]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::BodyDiffer);
    }

    #[test]
    fn classify_header_differ() {
        let body = b"same body";
        let a = make_resp(200, body, vec![("x-custom", b"a")]);
        let b = make_resp(200, body, vec![("x-custom", b"b")]);
        assert_eq!(DiffBucket::classify(&a, &b), DiffBucket::HeaderDiffer);
    }

    /// Infrastructure headers (date, server, x-forge-via) are stripped before
    /// comparison — they must not cause false positives.
    #[test]
    fn classify_infra_headers_stripped() {
        let body = b"same";
        let a = make_resp(
            200,
            body,
            vec![
                ("date", b"Thu, 01 Jan 2026 00:00:00 GMT"),
                ("server", b"armageddon-hyper"),
                ("content-type", b"application/json"),
            ],
        );
        let b = make_resp(
            200,
            body,
            vec![
                ("date", b"Thu, 01 Jan 2026 00:00:01 GMT"),
                ("server", b"armageddon-pingora"),
                ("content-type", b"application/json"),
            ],
        );
        assert_eq!(
            DiffBucket::classify(&a, &b),
            DiffBucket::Identical,
            "infra headers (date, server) must be stripped before comparison"
        );
    }

    // ── ShadowSampler ──────────────────────────────────────────────────────

    #[test]
    fn sampler_disable_stops_mirroring() {
        let cfg = ShadowModeConfig {
            enabled: true,
            sample_rate_percent: 100,
            ..Default::default()
        };
        let sampler = ShadowSampler::new(&cfg);
        assert!(sampler.should_shadow("any-id"), "100% rate must shadow");
        sampler.disable();
        assert!(
            !sampler.should_shadow("any-id"),
            "after disable(), no request must be shadowed"
        );
    }

    #[test]
    fn sampler_runtime_rate_change() {
        let cfg = ShadowModeConfig {
            enabled: true,
            sample_rate_percent: 0,
            ..Default::default()
        };
        let sampler = ShadowSampler::new(&cfg);
        assert!(!sampler.should_shadow("req-1"), "0% must not shadow");
        sampler.set_sample_percent(100);
        assert!(sampler.should_shadow("req-1"), "100% must shadow");
    }

    // ── DiffBucket::label ──────────────────────────────────────────────────

    #[test]
    fn diff_bucket_labels_are_stable() {
        assert_eq!(DiffBucket::Identical.label(), "identical");
        assert_eq!(DiffBucket::StatusDiffer.label(), "status_differ");
        assert_eq!(DiffBucket::BodyDiffer.label(), "body_differ");
        assert_eq!(DiffBucket::HeaderDiffer.label(), "header_differ");
        assert_eq!(DiffBucket::TimeoutOnPingora.label(), "timeout_on_pingora");
        assert_eq!(DiffBucket::TimeoutOnHyper.label(), "timeout_on_hyper");
        assert_eq!(DiffBucket::Streaming.label(), "streaming");
    }

    // ── ShadowDiffQueue ────────────────────────────────────────────────────

    #[tokio::test]
    async fn diff_queue_push_and_depth() {
        let q = ShadowDiffQueue::new(16);
        let event = ShadowEvent {
            ts: "2026-01-01T00:00:00Z".to_string(),
            request_id: "uuid-1".to_string(),
            method: "GET".to_string(),
            path: "/api/v1/test".to_string(),
            hyper_status: 200,
            pingora_status: Some(200),
            bucket: DiffBucket::Identical,
            latency_us_hyper: 1000,
            latency_us_pingora: Some(900),
            body_len_hyper: 42,
            body_len_pingora: Some(42),
        };
        assert!(q.push(event.clone()));
        assert_eq!(q.depth(), 1);
        assert!(q.push(event));
        assert_eq!(q.depth(), 2);
    }
}
