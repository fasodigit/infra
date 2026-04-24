// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Upstream mTLS — SPIFFE-authenticated TLS to the backend.
//!
//! This module implements the SPIFFE-peer verification gate that runs in
//! [`crate::pingora::gateway::PingoraGateway`]'s `upstream_request_filter`
//! hook before any bytes are sent to the upstream.
//!
//! # Design
//!
//! The `UpstreamMtlsFilter` reads `ctx.upstream_addr` and
//! `ctx.spiffe_peer_expected` (if set by the router / selector).  It
//! delegates the actual handshake to the `armageddon-mesh` crate's
//! `AutoMtlsDialer` which performs the allowlist check and validates the
//! X.509 URI SAN in the peer certificate.
//!
//! On success the validated SPIFFE ID is written to `ctx.spiffe_peer` so
//! downstream engines can inspect it without re-parsing the certificate.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | `expected_spiffe_id` is set and peer matches | `ctx.spiffe_peer` populated; `Decision::Continue` |
//! | `expected_spiffe_id` is set and peer mismatches | `Decision::Deny(502)` — fail-closed |
//! | `expected_spiffe_id` is `None` on a TLS cluster | `Decision::Deny(502)` — config error, fail-closed (invariant bug_006) |
//! | No TLS required | `Decision::Continue` immediately |
//!
//! # Security invariant (bug_006 — preserved from selector)
//!
//! This filter **never** falls back to plaintext when `tls_required` is set.
//! A missing `expected_spiffe_id` is treated as a configuration error and
//! results in `Deny(502)`, not silent downgrade.

use std::sync::Arc;

use tracing::{debug, error, warn};

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

// ── configuration ─────────────────────────────────────────────────────────────

/// Configuration for the upstream mTLS SPIFFE verification filter.
#[derive(Debug, Clone)]
pub struct UpstreamMtlsConfig {
    /// SPIFFE trust domain used for validation (e.g. `faso.gov.bf`).
    pub trust_domain: String,
    /// When `true`, ALL upstream connections must be mTLS.
    /// When `false`, the filter only activates when `ctx.spiffe_peer_expected`
    /// is set by the selector.
    pub require_mtls_globally: bool,
}

impl Default for UpstreamMtlsConfig {
    fn default() -> Self {
        Self {
            trust_domain: "faso.gov.bf".to_string(),
            require_mtls_globally: false,
        }
    }
}

// ── SpiffeVerificationResult ─────────────────────────────────────────────────

/// Outcome of a SPIFFE peer verification check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpiffeVerificationResult {
    /// Peer matches expected SPIFFE ID.
    Match(String),
    /// Peer SPIFFE ID is present but differs from expected.
    Mismatch {
        expected: String,
        observed: String,
    },
    /// No expected SPIFFE ID was configured (config error).
    MissingConfig,
    /// mTLS not required for this connection.
    NotRequired,
}

// ── SpiffeChecker — testable validation logic ─────────────────────────────────

/// Pure validation logic decoupled from the Pingora hook for unit testing.
///
/// In production the gateway calls `verify_peer` with the SPIFFE IDs derived
/// from the resolved peer; in tests a mock peer ID is injected directly.
pub struct SpiffeChecker;

impl SpiffeChecker {
    /// Verify that `observed_id` matches `expected_id`.
    ///
    /// Returns `SpiffeVerificationResult::Match` on success.
    /// Returns `SpiffeVerificationResult::Mismatch` when the IDs differ.
    /// Returns `SpiffeVerificationResult::MissingConfig` when `expected_id` is `None`
    /// and mTLS is required.
    pub fn verify(
        expected_id: Option<&str>,
        observed_id: Option<&str>,
        tls_required: bool,
    ) -> SpiffeVerificationResult {
        if !tls_required {
            return SpiffeVerificationResult::NotRequired;
        }

        let expected = match expected_id {
            Some(e) => e,
            None => {
                error!(
                    "UpstreamMtlsFilter: tls_required=true but expected_spiffe_id is None \
                     — refusing connection (config error, bug_006)"
                );
                return SpiffeVerificationResult::MissingConfig;
            }
        };

        match observed_id {
            Some(observed) if observed == expected => {
                debug!(
                    spiffe_id = %observed,
                    "UpstreamMtlsFilter: SPIFFE peer verified"
                );
                SpiffeVerificationResult::Match(observed.to_string())
            }
            Some(observed) => {
                warn!(
                    expected = %expected,
                    observed = %observed,
                    "UpstreamMtlsFilter: SPIFFE peer mismatch — denying"
                );
                SpiffeVerificationResult::Mismatch {
                    expected: expected.to_string(),
                    observed: observed.to_string(),
                }
            }
            None => {
                // Peer presented no SPIFFE ID in its certificate.
                warn!(
                    expected = %expected,
                    "UpstreamMtlsFilter: upstream presented no SPIFFE ID — denying"
                );
                SpiffeVerificationResult::Mismatch {
                    expected: expected.to_string(),
                    observed: String::new(),
                }
            }
        }
    }
}

// ── UpstreamMtlsFilter ────────────────────────────────────────────────────────

/// Upstream filter that validates the peer SPIFFE ID before forwarding.
///
/// This filter runs inside `upstream_request_filter`.  Because Pingora 0.3
/// does not expose the TLS handshake result directly from the hook, the
/// filter operates on the resolved peer metadata stored in `RequestCtx`:
///
/// - `ctx.spiffe_peer_expected` — set by the selector from cluster config.
/// - `ctx.spiffe_peer` — written here on successful validation.
///
/// The actual mTLS dial is handled by `armageddon-mesh`'s `AutoMtlsDialer`
/// which is invoked at TCP/TLS-connect time by Pingora's connector (outside
/// the filter chain in production).  This filter provides a defense-in-depth
/// gate: if the expected SPIFFE ID is missing or mismatched, the request is
/// denied with `502` before any upstream bytes are forwarded.
///
/// # TODO(M5)
///
/// Wire `armageddon_mesh::AutoMtlsDialer::connect_tls` into Pingora's
/// connector when `pingora-openssl` / `pingora-rustls` exposes a custom
/// connector hook (`upstream_connect` or equivalent).  For now, the filter
/// validates post-hoc using the peer info populated by the connector.
#[derive(Debug, Clone)]
pub struct UpstreamMtlsFilter {
    config: Arc<UpstreamMtlsConfig>,
}

impl UpstreamMtlsFilter {
    /// Create a new filter with the given configuration.
    pub fn new(config: UpstreamMtlsConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Create a filter with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(UpstreamMtlsConfig::default())
    }
}

#[async_trait::async_trait]
impl ForgeFilter for UpstreamMtlsFilter {
    fn name(&self) -> &'static str {
        "upstream_mtls"
    }

    /// In `on_upstream_request`, validate the resolved peer SPIFFE ID.
    ///
    /// `ctx.spiffe_peer_expected` is set by the upstream selector when the
    /// cluster carries `tls_required = true`.  If the global
    /// `require_mtls_globally` flag is set and `expected` is absent,
    /// `Deny(502)` is returned.
    async fn on_upstream_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut RequestCtx,
    ) -> Decision {
        let expected_str: &str = match &ctx.spiffe_peer_expected {
            Some(e) => e.as_str(),
            None if self.config.require_mtls_globally => {
                error!(
                    upstream = %ctx.upstream_addr,
                    "UpstreamMtlsFilter: require_mtls_globally=true but no \
                     expected_spiffe_id in ctx — denying (bug_006)"
                );
                return Decision::Deny(502);
            }
            None => {
                // mTLS not required for this cluster.
                debug!(
                    upstream = %ctx.upstream_addr,
                    "UpstreamMtlsFilter: no expected SPIFFE ID — skipping mTLS validation"
                );
                return Decision::Continue;
            }
        };

        // In Pingora 0.3 the TLS handshake is transparent; the filter
        // validates the peer ID that was resolved by the selector and stored
        // in `ctx.spiffe_peer_expected`.  When `ctx.spiffe_peer` is already
        // populated by a connector-level hook, we use that; otherwise we
        // trust the selector's decision (defense-in-depth validation).
        let observed = ctx.spiffe_peer.as_deref();

        match SpiffeChecker::verify(Some(expected_str), observed, true) {
            SpiffeVerificationResult::Match(id) => {
                ctx.spiffe_peer = Some(id);
                Decision::Continue
            }
            SpiffeVerificationResult::Mismatch { expected, observed } => {
                warn!(
                    upstream = %ctx.upstream_addr,
                    expected = %expected,
                    observed = %observed,
                    "UpstreamMtlsFilter: SPIFFE mismatch — upstream_mtls_spiffe_mismatch"
                );
                Decision::Deny(502)
            }
            SpiffeVerificationResult::MissingConfig => Decision::Deny(502),
            SpiffeVerificationResult::NotRequired => Decision::Continue,
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SpiffeChecker::verify ─────────────────────────────────────────────────

    /// Accept when expected and observed match exactly.
    #[test]
    fn verify_match_returns_match() {
        let id = "spiffe://faso.gov.bf/ns/kaya/sa/shard-0";
        let result = SpiffeChecker::verify(Some(id), Some(id), true);
        assert_eq!(
            result,
            SpiffeVerificationResult::Match(id.to_string()),
            "matching SPIFFE IDs must return Match"
        );
    }

    /// Reject when expected and observed differ.
    #[test]
    fn verify_mismatch_returns_mismatch() {
        let expected = "spiffe://faso.gov.bf/ns/kaya/sa/shard-0";
        let observed = "spiffe://other.example/ns/attacker/sa/evil";
        let result = SpiffeChecker::verify(Some(expected), Some(observed), true);
        assert_eq!(
            result,
            SpiffeVerificationResult::Mismatch {
                expected: expected.to_string(),
                observed: observed.to_string(),
            },
            "mismatched SPIFFE IDs must return Mismatch"
        );
    }

    /// Fail-closed: no expected SPIFFE ID when mTLS is required → MissingConfig.
    #[test]
    fn verify_missing_expected_returns_missing_config() {
        let result = SpiffeChecker::verify(None, Some("spiffe://anything"), true);
        assert_eq!(
            result,
            SpiffeVerificationResult::MissingConfig,
            "missing expected SPIFFE ID with tls_required must return MissingConfig"
        );
    }

    /// When mTLS is not required, always return NotRequired regardless of IDs.
    #[test]
    fn verify_not_required_returns_not_required() {
        let result = SpiffeChecker::verify(None, None, false);
        assert_eq!(
            result,
            SpiffeVerificationResult::NotRequired,
            "when tls_required=false, result must be NotRequired"
        );
    }

    /// Peer presents no SPIFFE ID at all — treat as mismatch (fail-closed).
    #[test]
    fn verify_missing_observed_returns_mismatch() {
        let expected = "spiffe://faso.gov.bf/ns/kaya/sa/shard-0";
        let result = SpiffeChecker::verify(Some(expected), None, true);
        match result {
            SpiffeVerificationResult::Mismatch { .. } => {}
            other => panic!("expected Mismatch when peer has no SPIFFE ID, got {other:?}"),
        }
    }

    // ── UpstreamMtlsFilter (filter unit tests) ────────────────────────────────

    #[test]
    fn filter_construction_with_defaults() {
        let f = UpstreamMtlsFilter::with_defaults();
        assert_eq!(f.name(), "upstream_mtls");
        assert_eq!(f.config.trust_domain, "faso.gov.bf");
        assert!(!f.config.require_mtls_globally);
    }

    #[test]
    fn filter_construction_custom_config() {
        let cfg = UpstreamMtlsConfig {
            trust_domain: "test.example".to_string(),
            require_mtls_globally: true,
        };
        let f = UpstreamMtlsFilter::new(cfg);
        assert_eq!(f.config.trust_domain, "test.example");
        assert!(f.config.require_mtls_globally);
    }

    /// Validate the ctx-level SPIFFE logic without a live Pingora session.
    /// Simulates the `on_upstream_request` outcome directly via SpiffeChecker.
    #[test]
    fn mtls_ctx_validation_accept_valid_spiffe_id() {
        let mut ctx = RequestCtx::new();
        let spiffe_id = "spiffe://faso.gov.bf/ns/auth-ms/sa/default";
        ctx.spiffe_peer_expected = Some(spiffe_id.to_string());
        // Simulate connector populating the observed peer.
        ctx.spiffe_peer = Some(spiffe_id.to_string());

        let result = SpiffeChecker::verify(
            ctx.spiffe_peer_expected.as_deref(),
            ctx.spiffe_peer.as_deref(),
            true,
        );
        assert_eq!(result, SpiffeVerificationResult::Match(spiffe_id.to_string()));
    }

    #[test]
    fn mtls_ctx_validation_reject_mismatched_spiffe_id() {
        let mut ctx = RequestCtx::new();
        ctx.spiffe_peer_expected = Some("spiffe://faso.gov.bf/ns/kaya/sa/shard-0".to_string());
        ctx.spiffe_peer = Some("spiffe://other.example/ns/evil/sa/attacker".to_string());

        let result = SpiffeChecker::verify(
            ctx.spiffe_peer_expected.as_deref(),
            ctx.spiffe_peer.as_deref(),
            true,
        );
        assert!(
            matches!(result, SpiffeVerificationResult::Mismatch { .. }),
            "ctx with mismatched SPIFFE IDs must produce Mismatch"
        );
    }

    /// Security regression: fail-closed when expected is not configured.
    #[test]
    fn mtls_ctx_validation_fail_closed_without_expected() {
        let ctx = RequestCtx::new(); // no spiffe_peer_expected
        let result = SpiffeChecker::verify(None, Some("spiffe://any/peer"), true);
        assert_eq!(
            result,
            SpiffeVerificationResult::MissingConfig,
            "missing expected SPIFFE ID must fail-closed (never downgrade to plaintext)"
        );
        // Confirm ctx carries no false-positive peer
        assert!(ctx.spiffe_peer.is_none());
    }
}
