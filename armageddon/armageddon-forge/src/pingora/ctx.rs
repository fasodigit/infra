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
//! | `span_id`                 | OTEL filter (W3C parser)    | M1 #99 |
//! | `cluster`                 | Router filter               | M1 #95 |
//! | `upstream_addr`           | Upstream selector           | M2 #103|
//! | `user_id` / `tenant_id`   | JWT filter                  | M1 #97 |
//! | `roles`                   | JWT filter                  | M1 #97 |
//! | `bearer_token`            | JWT filter (raw token)      | M1 #97 |
//! | `spiffe_peer`             | mTLS upstream filter        | M2     |
//! | `feature_flags`           | Feature-flag filter         | M1 #98 |
//! | `cors_origin`             | CORS filter                 | M1 #96 |
//! | `veil_nonce`              | VEIL filter                 | M1 #100|
//! | `request_start_ms`        | OTEL filter                 | M1 #99 |
//! | `waf_score`               | SENTINEL / ARBITER engines  | M3 #104|
//! | `ai_score`                | ORACLE / AI engines         | M3 #104|
//! | `cdc_outbox_id`           | Webhook / CDC plumbing      | M4     |
//! | `compression_session`     | Compression wiring          | M4 #105|
//! | `grpc_web_mode`           | gRPC-Web protocol handler   | M4 #105|
//! | `ws_upgrade`              | WebSocket protocol handler  | M4 #105|
//! | `traffic_split_shadow`    | Traffic split decision      | M4 #105|
//!
//! The struct derives `Default` so `PingoraGateway::new_ctx` can simply call
//! [`RequestCtx::new`] (which fills `request_id` with a fresh UUID).

#[cfg(feature = "pingora")]
use crate::pingora::protocols::compression::{CompressionLevel, CompressionStream, Encoding};

/// Per-request context shared across filters, engines and the upstream path.
///
/// # Clone semantics for M4 fields
///
/// `compression_session` is **not cloned** — a live encoder state is
/// per-request and must not be shared.  Cloning a `RequestCtx` (e.g. for
/// metrics snapshots) produces a copy with `compression_session = None`.
#[derive(Debug)]
pub struct RequestCtx {
    /// Unique request identifier — UUID v4 generated in `new_ctx`.
    /// Injected as the `x-forge-id` header by the core request filter.
    pub request_id: String,

    /// W3C `traceparent` trace identifier — populated by OTEL filter (M1 #99).
    pub trace_id: String,

    /// W3C `traceparent` span identifier — populated by OTEL filter (M1 #99).
    pub span_id: String,

    /// Timestamp in milliseconds since UNIX epoch when the request arrived,
    /// used by the OTEL filter for `duration_ms` in `on_logging`.
    pub request_start_ms: u64,

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

    /// Raw Bearer token (populated by JWT filter when a valid token is present).
    ///
    /// Stored so downstream engines can access the token without re-parsing.
    /// **Never log or include in responses.**
    pub bearer_token: Option<String>,

    /// Expected peer SPIFFE ID set by the upstream selector when the cluster
    /// carries `tls_required = true` (M2).  Read by `UpstreamMtlsFilter`.
    pub spiffe_peer_expected: Option<String>,

    /// Peer SPIFFE ID observed / validated during upstream mTLS handshake (M2).
    pub spiffe_peer: Option<String>,

    /// Feature-flag identifiers injected by the feature-flag filter (M1 #98).
    pub feature_flags: Vec<String>,

    /// CORS `Origin` header observed in the downstream request (M1 #96).
    ///
    /// Populated by the CORS filter in `on_request` and consumed in
    /// `on_response` to inject `Access-Control-Allow-Origin`.  Replaces the
    /// previous `"cors:origin:"` stringly-typed prefix in `feature_flags`.
    pub cors_origin: Option<String>,

    /// CSP nonce minted by the VEIL filter (M1 #100).
    ///
    /// Replaces the previous `"veil:nonce:"` stringly-typed prefix in
    /// `feature_flags`.  Downstream HTML-rewriting filters can read this
    /// without scanning the flags vec.
    pub veil_nonce: Option<String>,

    /// Aggregate WAF score from SENTINEL / ARBITER engines (M3 #104).
    /// Range: 0.0 (safe) ─ 1.0 (block).
    pub waf_score: f32,

    /// Aggregate AI / anomaly score from ORACLE / AI engines (M3 #104).
    /// Range: 0.0 (safe) ─ 1.0 (block).
    pub ai_score: f32,

    /// Webhook / CDC outbox correlation identifier (M4).
    pub cdc_outbox_id: Option<String>,

    // ── M4 protocol scratch slots ────────────────────────────────────────────

    /// Per-request compression encoder state, held between `response_filter`
    /// (header negotiation) and `response_body_filter` (body streaming).
    ///
    /// `None` means pass-through (no compression for this response).
    /// Set by the compression wiring in `gateway.rs` after negotiation.
    #[cfg(feature = "pingora")]
    pub compression_session: Option<CompressionSession>,

    /// When set, the downstream request was detected as gRPC-Web.
    ///
    /// Carries the variant (`Binary` / `Text`) so that body filters can
    /// encode/decode correctly without re-inspecting headers.
    pub grpc_web_mode: Option<GrpcWebMode>,

    /// True when the downstream request carries valid WebSocket upgrade
    /// headers.  Set by the WebSocket detection code in `request_filter`.
    pub ws_upgrade: bool,

    /// When traffic_split decides a shadow target, its cluster name is stored
    /// here so that `upstream_peer` can fire-and-forget the shadow request.
    pub traffic_split_shadow: Option<String>,
}

// ── M4 auxiliary types ─────────────────────────────────────────────────────

/// State maintained between `response_filter` and `response_body_filter` for
/// streaming compression.
///
/// `response_filter` negotiates encoding and stores this struct in
/// `ctx.compression_session`; every subsequent `response_body_filter` call
/// feeds bytes into `stream` and drains compressed output.
#[cfg(feature = "pingora")]
pub struct CompressionSession {
    /// Active streaming encoder — consumed on the final chunk via `finish()`.
    pub stream: CompressionStream,
    /// The chosen encoding (mirrors `stream.encoding()`, kept for quick access).
    pub encoding: Encoding,
    /// Compression level chosen at negotiation time.
    pub level: CompressionLevel,
}

#[cfg(feature = "pingora")]
impl std::fmt::Debug for CompressionSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompressionSession")
            .field("encoding", &self.encoding)
            .field("level", &self.level)
            .finish()
    }
}

/// gRPC-Web content-type variant detected on the downstream request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcWebMode {
    /// `application/grpc-web+proto` — binary framing.
    Binary,
    /// `application/grpc-web-text` — base64-encoded framing.
    Text,
}

impl Default for RequestCtx {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            trace_id: String::new(),
            span_id: String::new(),
            request_start_ms: 0,
            cluster: String::new(),
            upstream_addr: String::new(),
            user_id: None,
            tenant_id: None,
            roles: Vec::new(),
            bearer_token: None,
            spiffe_peer_expected: None,
            spiffe_peer: None,
            feature_flags: Vec::new(),
            cors_origin: None,
            veil_nonce: None,
            waf_score: 0.0,
            ai_score: 0.0,
            cdc_outbox_id: None,
            #[cfg(feature = "pingora")]
            compression_session: None,
            grpc_web_mode: None,
            ws_upgrade: false,
            traffic_split_shadow: None,
        }
    }
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

/// Manual `Clone` impl that resets `compression_session` to `None`.
///
/// A streaming encoder is a per-request resource — cloning it makes no
/// sense.  Callers that clone a `RequestCtx` (e.g. to snapshot metrics)
/// get a copy with the encoder slot cleared.
impl Clone for RequestCtx {
    fn clone(&self) -> Self {
        Self {
            request_id: self.request_id.clone(),
            trace_id: self.trace_id.clone(),
            span_id: self.span_id.clone(),
            request_start_ms: self.request_start_ms,
            cluster: self.cluster.clone(),
            upstream_addr: self.upstream_addr.clone(),
            user_id: self.user_id.clone(),
            tenant_id: self.tenant_id.clone(),
            roles: self.roles.clone(),
            bearer_token: self.bearer_token.clone(),
            spiffe_peer_expected: self.spiffe_peer_expected.clone(),
            spiffe_peer: self.spiffe_peer.clone(),
            feature_flags: self.feature_flags.clone(),
            cors_origin: self.cors_origin.clone(),
            veil_nonce: self.veil_nonce.clone(),
            waf_score: self.waf_score,
            ai_score: self.ai_score,
            cdc_outbox_id: self.cdc_outbox_id.clone(),
            // Encoder state is per-request; intentionally not cloned.
            #[cfg(feature = "pingora")]
            compression_session: None,
            grpc_web_mode: self.grpc_web_mode,
            ws_upgrade: self.ws_upgrade,
            traffic_split_shadow: self.traffic_split_shadow.clone(),
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

    #[test]
    fn typed_slots_default_to_none() {
        let c = RequestCtx::default();
        assert!(c.cors_origin.is_none());
        assert!(c.veil_nonce.is_none());
        assert!(c.bearer_token.is_none());
        assert!(c.trace_id.is_empty());
        assert!(c.span_id.is_empty());
        assert_eq!(c.request_start_ms, 0);
    }

    #[test]
    fn typed_slots_are_independently_settable() {
        let mut c = RequestCtx::new();
        c.cors_origin = Some("https://app.faso.dev".to_string());
        c.veil_nonce = Some("abc123".to_string());
        c.bearer_token = Some("eyJ...".to_string());
        assert_eq!(c.cors_origin.as_deref(), Some("https://app.faso.dev"));
        assert_eq!(c.veil_nonce.as_deref(), Some("abc123"));
        assert_eq!(c.bearer_token.as_deref(), Some("eyJ..."));
        // feature_flags is unaffected.
        assert!(c.feature_flags.is_empty());
    }

    // -- M4 protocol slots ---------------------------------------------------

    #[test]
    fn m4_slots_default_to_none_or_false() {
        let c = RequestCtx::default();
        assert!(c.grpc_web_mode.is_none());
        assert!(!c.ws_upgrade);
        assert!(c.traffic_split_shadow.is_none());
    }

    #[test]
    fn grpc_web_mode_is_settable() {
        let mut c = RequestCtx::new();
        c.grpc_web_mode = Some(GrpcWebMode::Binary);
        assert_eq!(c.grpc_web_mode, Some(GrpcWebMode::Binary));
        c.grpc_web_mode = Some(GrpcWebMode::Text);
        assert_eq!(c.grpc_web_mode, Some(GrpcWebMode::Text));
    }

    #[test]
    fn ws_upgrade_is_settable() {
        let mut c = RequestCtx::new();
        c.ws_upgrade = true;
        assert!(c.ws_upgrade);
    }

    #[test]
    fn traffic_split_shadow_is_settable() {
        let mut c = RequestCtx::new();
        c.traffic_split_shadow = Some("shadow-cluster".to_string());
        assert_eq!(
            c.traffic_split_shadow.as_deref(),
            Some("shadow-cluster")
        );
    }

    #[test]
    fn clone_resets_compression_session() {
        let mut c = RequestCtx::new();
        c.grpc_web_mode = Some(GrpcWebMode::Text);
        c.ws_upgrade = true;
        c.traffic_split_shadow = Some("canary".to_string());
        // Clone must not panic and must preserve non-encoder fields.
        let c2 = c.clone();
        assert_eq!(c2.grpc_web_mode, Some(GrpcWebMode::Text));
        assert!(c2.ws_upgrade);
        assert_eq!(c2.traffic_split_shadow.as_deref(), Some("canary"));
        // compression_session is None after clone (encoder not clonable).
        #[cfg(feature = "pingora")]
        assert!(c2.compression_session.is_none());
    }
}
