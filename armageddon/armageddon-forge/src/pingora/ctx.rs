// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-request context propagated through the Pingora filter chain.
//!
//! [`RequestCtx`] is created once per request in
//! [`crate::pingora::gateway::PingoraGateway::new_ctx`] and handed to every
//! filter / engine as a `&mut` reference.  Fields are populated incrementally
//! as the request moves through the pipeline:
//!
//! | Field                     | Populated by                | Gate   |
//! |---------------------------|-----------------------------|--------|
//! | `request_id`              | `new_ctx` (uuid v4)         | M0     |
//! | `trace_id`                | OTEL filter (W3C parser)    | M1 #99 |
//! | `cluster`                 | Router filter               | M1 #95 |
//! | `upstream_addr`           | Upstream selector           | M2 #103|
//! | `user_id` / `tenant_id`   | JWT filter                  | M1 #97 |
//! | `roles`                   | JWT filter                  | M1 #97 |
//! | `spiffe_peer`             | mTLS upstream filter        | M2     |
//! | `feature_flags`           | Feature-flag filter         | M1 #98 |
//! | `waf_score`               | SENTINEL / ARBITER engines  | M3 #104|
//! | `ai_score`                | ORACLE / AI engines         | M3 #104|
//! | `cdc_outbox_id`           | Webhook / CDC plumbing      | M4     |
//!
//! The struct derives `Default` so `PingoraGateway::new_ctx` can simply call
//! [`RequestCtx::new`] (which fills `request_id` with a fresh UUID).

/// Per-request context shared across filters, engines and the upstream path.
#[derive(Debug, Default, Clone)]
pub struct RequestCtx {
    /// Unique request identifier — UUID v4 generated in `new_ctx`.
    /// Injected as the `x-forge-id` header by the core request filter.
    pub request_id: String,

    /// W3C `traceparent` trace identifier — populated by OTEL filter (M1 #99).
    pub trace_id: String,

    /// Logical cluster name resolved by the router filter (M1 #95).
    pub cluster: String,

    /// Resolved upstream address (host:port) selected by the upstream
    /// selector (M2 #103) and used by `upstream_peer`.
    pub upstream_addr: String,

    /// Authenticated user identifier set by the JWT filter (M1 #97).
    pub user_id: Option<String>,

    /// Tenant identifier parsed from the JWT `tenant_id` claim (M1 #97).
    pub tenant_id: Option<String>,

    /// Roles / scopes parsed from the JWT (M1 #97).
    pub roles: Vec<String>,

    /// Peer SPIFFE ID observed during upstream mTLS handshake (M2).
    pub spiffe_peer: Option<String>,

    /// Feature-flag identifiers injected by the feature-flag filter (M1 #98).
    pub feature_flags: Vec<String>,

    /// Aggregate WAF score from SENTINEL / ARBITER engines (M3 #104).
    /// Range: 0.0 (safe) ─ 1.0 (block).
    pub waf_score: f32,

    /// Aggregate AI / anomaly score from ORACLE / AI engines (M3 #104).
    /// Range: 0.0 (safe) ─ 1.0 (block).
    pub ai_score: f32,

    /// Webhook / CDC outbox correlation identifier (M4).
    pub cdc_outbox_id: Option<String>,
}

impl RequestCtx {
    /// Construct a new context with a freshly-minted UUID v4 as `request_id`.
    pub fn new() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ctx_has_fresh_uuid() {
        let a = RequestCtx::new();
        let b = RequestCtx::new();
        assert_ne!(a.request_id, b.request_id);
        assert_eq!(a.request_id.len(), 36); // canonical UUID v4 length
    }

    #[test]
    fn default_ctx_has_empty_request_id() {
        let c = RequestCtx::default();
        assert!(c.request_id.is_empty());
        assert!(c.cluster.is_empty());
        assert!(c.roles.is_empty());
        assert_eq!(c.waf_score, 0.0);
    }
}
