// SPDX-License-Identifier: AGPL-3.0-or-later
//! OpenTelemetry filter — extracts / injects W3C `traceparent` headers and
//! spawns a span for the proxied request.
//!
//! **M0 scaffolding**: this is a no-op stub.  The real implementation
//! (port of `src/otel_middleware.rs`) lands in gate M1 #99.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// OTEL filter — populates `RequestCtx::trace_id`.
#[derive(Debug, Default)]
pub struct OtelFilter;

impl OtelFilter {
    /// Create a new stub OTEL filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for OtelFilter {
    fn name(&self) -> &'static str {
        "otel"
    }

    async fn on_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #99: parse `traceparent`, create a `tracing::Span` with
        // `otel.trace_id` attached, populate ctx.trace_id.
        Decision::Continue
    }

    async fn on_upstream_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _req: &mut pingora::http::RequestHeader,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #99: inject the downstream trace context into the upstream
        // request so tracing is stitched across the proxy hop.
        Decision::Continue
    }

    async fn on_logging(&self, _session: &mut pingora_proxy::Session, _ctx: &RequestCtx) {
        // M1 #99: finish the span, record duration + status.
    }
}
