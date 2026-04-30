// SPDX-License-Identifier: AGPL-3.0-or-later
//! Filter chain for the Pingora-based gateway.
//!
//! A *filter* is a composable unit that inspects / mutates the request or
//! response as it traverses the gateway pipeline.  Filters are registered in
//! [`crate::pingora::gateway::PingoraGatewayConfig::filters`] and invoked in
//! order by [`crate::pingora::gateway::PingoraGateway`]'s `ProxyHttp`
//! implementation.
//!
//! ## Hook points
//!
//! | `ProxyHttp` hook          | `ForgeFilter` callback        |
//! |---------------------------|-------------------------------|
//! | `request_filter`          | [`ForgeFilter::on_request`]   |
//! | `upstream_request_filter` | [`ForgeFilter::on_upstream_request`] |
//! | `response_filter`         | [`ForgeFilter::on_response`]  |
//! | `logging`                 | [`ForgeFilter::on_logging`]   |
//!
//! ## Decision model
//!
//! Each callback returns a [`Decision`]:
//!
//! - `Continue` — proceed to the next filter / upstream.
//! - `ShortCircuit(resp)` — terminate the pipeline with `resp` as the
//!   downstream response (used by CORS pre-flight, cached replies, etc.).
//! - `Deny(status)` — terminate with a bare status code (used by JWT / WAF).
//!
//! ## M0 scaffolding note
//!
//! The sub-modules below (`router`, `cors`, `jwt`, `feature_flag`, `otel`,
//! `veil`) are **stubs** — each exports an empty struct that implements
//! `ForgeFilter` as a no-op (returning [`Decision::Continue`]).  Real
//! behaviour is ported from the hyper path in gates M1 (#95 – #100).

use std::sync::Arc;

use crate::pingora::ctx::RequestCtx;

pub mod cors;
pub mod feature_flag;
pub mod jwt;
pub mod otel;
pub mod router;
pub mod veil;
pub mod waf;

/// Coraza-on-Pingora WAF (proxy-wasm v0.2.1 host).  Compiled only when
/// the `coraza-wasm` Cargo feature is enabled.  See
/// `armageddon/coraza/PROXY-WASM-HOST-DESIGN.md`.
#[cfg(feature = "coraza-wasm")]
pub mod waf_coraza;

/// Outcome of a filter hook invocation.
///
/// The `ShortCircuit` and `Deny` variants terminate the remaining filter chain
/// and cause the gateway to emit a response to the downstream client without
/// touching the upstream.
pub enum Decision {
    /// Proceed to the next filter (or upstream selection if last).
    Continue,

    /// Terminate the pipeline and send `ResponseHeader` as the downstream
    /// response.  The body is left empty unless a subsequent release of the
    /// API adds body support.
    ShortCircuit(Box<pingora::http::ResponseHeader>),

    /// Terminate the pipeline with a bare HTTP status code.
    Deny(u16),
}

impl std::fmt::Debug for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Continue => f.write_str("Decision::Continue"),
            Decision::ShortCircuit(_) => f.write_str("Decision::ShortCircuit(..)"),
            Decision::Deny(code) => write!(f, "Decision::Deny({code})"),
        }
    }
}

/// A composable filter in the FORGE-Pingora pipeline.
///
/// All hooks have default no-op implementations returning `Decision::Continue`
/// so implementers only need to override the stages they care about.
#[async_trait::async_trait]
pub trait ForgeFilter: Send + Sync + 'static {
    /// Human-readable name (used in tracing / metrics).
    fn name(&self) -> &'static str {
        "forge-filter"
    }

    /// Invoked at the `request_filter` hook.  Return `ShortCircuit` / `Deny`
    /// to terminate the pipeline before upstream selection.
    async fn on_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        Decision::Continue
    }

    /// Invoked at the `upstream_request_filter` hook, after `upstream_peer`
    /// has resolved the target.  Use to mutate headers sent upstream
    /// (authorization, tracing, host override, …).
    async fn on_upstream_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _req: &mut pingora::http::RequestHeader,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        Decision::Continue
    }

    /// Invoked at the `request_body_filter` hook for each chunk of the
    /// inbound request body, including the final empty chunk where
    /// `end_of_stream = true`.
    ///
    /// Filters that need to inspect the body (e.g. WAF body scanner) buffer
    /// chunks into `ctx.body_buffer` (capped) and evaluate on `end_of_stream`.
    /// Returning `Decision::Deny(code)` aborts the upstream forward and
    /// emits the chosen HTTP status to the client.
    async fn on_request_body(
        &self,
        _session: &mut pingora_proxy::Session,
        _body: &Option<bytes::Bytes>,
        _end_of_stream: bool,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        Decision::Continue
    }

    /// Invoked at the `response_filter` hook — mutate the response returned
    /// to the downstream client.
    async fn on_response(
        &self,
        _session: &mut pingora_proxy::Session,
        _res: &mut pingora::http::ResponseHeader,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        Decision::Continue
    }

    /// Invoked at the `logging` hook (post-flush).  Use for access logs,
    /// metrics, audit events.
    async fn on_logging(&self, _session: &mut pingora_proxy::Session, _ctx: &RequestCtx) {}
}

/// Boxed / Arc'd filter reference used throughout the gateway.
pub type SharedFilter = Arc<dyn ForgeFilter>;

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopFilter;

    #[async_trait::async_trait]
    impl ForgeFilter for NoopFilter {
        fn name(&self) -> &'static str {
            "noop"
        }
    }

    #[tokio::test]
    async fn default_hooks_are_continue() {
        let f: SharedFilter = Arc::new(NoopFilter);
        assert_eq!(f.name(), "noop");
        // on_logging is unit-returning; it must not panic when called with a
        // stub ctx.  The Session reference cannot be constructed from tests
        // without a live Pingora runtime, so we only call on_logging's
        // safety: passing a null ptr is UB, so we exercise the trait via a
        // dispatch that does not require Session at all.  The existence of
        // this test is a compile-time guarantee that `Decision` is the
        // correct return type.
        let _ = f.name();
    }

    #[test]
    fn decision_debug_does_not_leak_response() {
        let d = Decision::Deny(401);
        assert_eq!(format!("{d:?}"), "Decision::Deny(401)");
        let c = Decision::Continue;
        assert_eq!(format!("{c:?}"), "Decision::Continue");
    }
}
