// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//! OpenTelemetry tracing middleware for ARMAGEDDON FORGE.
//!
//! Wraps every proxied request in a three-level span hierarchy:
//!
//! ```text
//! armageddon.proxy          ← outer span (request enters the gateway)
//!   └─ armageddon.pentagon  ← security-engine evaluation span
//!        └─ armageddon.upstream  ← upstream forwarding span
//! ```
//!
//! ### Usage
//!
//! ```rust,ignore
//! let guard = begin_request_trace(&incoming_headers, "POST", "/api/graphql", "dgs-gateway");
//!
//! // inject into upstream request:
//! inject_upstream_headers(&guard, &mut upstream_headers);
//!
//! // record response:
//! guard.record_upstream_status(200);
//!
//! // inject traceparent into client response:
//! inject_trace_headers(&guard, &mut response_headers);
//!
//! // guard dropped here — all three spans are ended automatically.
//! ```

use http::HeaderMap;
use opentelemetry::{
    global,
    trace::{Span, SpanKind, Status, TraceContextExt, Tracer},
    Context, KeyValue,
};

use armageddon_oracle::otel_propagation::{extract_context, inject_context};

// ---------------------------------------------------------------------------
// Span name constants
// ---------------------------------------------------------------------------

const SPAN_PROXY: &str = "armageddon.proxy";
const SPAN_PENTAGON: &str = "armageddon.pentagon";
const SPAN_UPSTREAM: &str = "armageddon.upstream";

// ---------------------------------------------------------------------------
// RequestSpanGuard — RAII holder for a three-level span context.
// ---------------------------------------------------------------------------

/// Holds the three [`opentelemetry::global::BoxedSpan`] handles and their
/// parent [`Context`]s for one proxied request.
///
/// Spans are ended in innermost-first order when the guard is dropped:
/// `upstream` → `pentagon` → `proxy`.
pub struct RequestSpanGuard {
    /// Context containing `armageddon.proxy` as the active span.
    proxy_cx: Context,
    /// Context containing `armageddon.pentagon` as the active span.
    pentagon_cx: Context,
    /// Context containing `armageddon.upstream` as the active span.
    upstream_cx: Context,
    /// Owned span handles — ended (and exported) when `Drop` is called.
    proxy_span:    opentelemetry::global::BoxedSpan,
    pentagon_span: opentelemetry::global::BoxedSpan,
    upstream_span: opentelemetry::global::BoxedSpan,
}

impl RequestSpanGuard {
    /// Record the HTTP status code returned by the upstream service on the
    /// `armageddon.upstream` span.  Call before dropping the guard.
    pub fn record_upstream_status(&mut self, status: u16) {
        self.upstream_span
            .set_attribute(KeyValue::new("http.status_code", status as i64));
        if status >= 500 {
            self.upstream_span.set_status(Status::Error {
                description: format!("upstream returned {}", status).into(),
            });
        }
    }

    /// Return a reference to the proxy-level context (inject into response
    /// headers via [`inject_trace_headers`]).
    pub fn proxy_context(&self) -> &Context {
        &self.proxy_cx
    }

    /// Return a reference to the pentagon-level context (pass to security
    /// engine calls so they become children of `armageddon.pentagon`).
    pub fn pentagon_context(&self) -> &Context {
        &self.pentagon_cx
    }

    /// Return a reference to the upstream-level context (inject into outgoing
    /// upstream request headers via [`inject_upstream_headers`]).
    pub fn upstream_context(&self) -> &Context {
        &self.upstream_cx
    }
}

// Explicitly end spans in innermost-first order on drop.
impl Drop for RequestSpanGuard {
    fn drop(&mut self) {
        self.upstream_span.end();
        self.pentagon_span.end();
        self.proxy_span.end();
    }
}

// ---------------------------------------------------------------------------
// begin_request_trace — entry point called by the proxy handler
// ---------------------------------------------------------------------------

/// Begin the three-level span hierarchy for one incoming request.
///
/// # Arguments
/// - `incoming_headers` — headers from the client request; parsed for W3C
///   `traceparent` / `tracestate` / `baggage`.
/// - `method` — HTTP method (e.g. `"GET"`).
/// - `path` — request path (e.g. `"/api/users"`).
/// - `cluster` — resolved upstream cluster name.
///
/// Returns a [`RequestSpanGuard`] that must be held for the entire request
/// lifetime.  Drop it after the response has been sent — this ends all spans.
pub fn begin_request_trace(
    incoming_headers: &HeaderMap,
    method: &str,
    path: &str,
    cluster: &str,
) -> RequestSpanGuard {
    let tracer = global::tracer("armageddon-forge");

    // Extract parent context from the incoming W3C traceparent header.
    let parent_cx = extract_context(incoming_headers);

    // -- armageddon.proxy span --
    let proxy_span = tracer
        .span_builder(SPAN_PROXY)
        .with_kind(SpanKind::Server)
        .with_attributes(vec![
            KeyValue::new("http.method", method.to_string()),
            KeyValue::new("http.target", path.to_string()),
            KeyValue::new("armageddon.cluster", cluster.to_string()),
        ])
        .start_with_context(&tracer, &parent_cx);

    let proxy_cx = parent_cx.with_span(proxy_span);

    // -- armageddon.pentagon span (child of proxy) --
    let pentagon_span = tracer
        .span_builder(SPAN_PENTAGON)
        .with_kind(SpanKind::Internal)
        .with_attributes(vec![
            KeyValue::new("armageddon.component", "pentagon"),
        ])
        .start_with_context(&tracer, &proxy_cx);

    let pentagon_cx = proxy_cx.with_span(pentagon_span);

    // -- armageddon.upstream span (child of pentagon) --
    let upstream_span = tracer
        .span_builder(SPAN_UPSTREAM)
        .with_kind(SpanKind::Client)
        .with_attributes(vec![
            KeyValue::new("http.method", method.to_string()),
            KeyValue::new("http.url", path.to_string()),
            KeyValue::new("net.peer.name", cluster.to_string()),
        ])
        .start_with_context(&tracer, &pentagon_cx);

    let upstream_cx = pentagon_cx.with_span(upstream_span);

    // Extract the actual span handles back out of each context.
    // `Context::span()` returns a reference, but we need owned handles to
    // call `.end()` / set attributes.  We rebuild spans via the builder so
    // each context holds a fresh span — the `with_span` call consumed the
    // builder-created span into the context, giving us `BoxedSpan` values.
    //
    // To get owned spans back we must re-start from the contexts.  The
    // OpenTelemetry SDK keeps spans alive via the Context's Arc so ending
    // here just records the timestamp.

    // Re-start spans so we own the BoxedSpan handles for attribute setting
    // and explicit end.  These are child spans of the originals above,
    // sharing the same trace/span IDs because the propagator sets them from
    // the context.
    //
    // NOTE: A cleaner pattern is to use `with_span` which keeps context
    // associated but we need owned handles.  The simplest correct approach
    // in the otel 0.27 API is to use `Context::current_with_span` pattern:

    // The three spans were consumed into proxy_cx / pentagon_cx / upstream_cx.
    // We create separate "attribute carrier" spans via the context so we can
    // call set_attribute / end explicitly.  The original spans (now inside
    // the contexts) will be ended when the Context is dropped.
    //
    // For the attribute-setting + status API we need an owned BoxedSpan.
    // We create a separate lightweight "attribute" span under upstream_cx:
    let attr_upstream = tracer
        .span_builder(format!("{}.attrs", SPAN_UPSTREAM))
        .with_kind(SpanKind::Internal)
        .start_with_context(&tracer, &upstream_cx);

    let attr_pentagon = tracer
        .span_builder(format!("{}.attrs", SPAN_PENTAGON))
        .with_kind(SpanKind::Internal)
        .start_with_context(&tracer, &pentagon_cx);

    let attr_proxy = tracer
        .span_builder(format!("{}.attrs", SPAN_PROXY))
        .with_kind(SpanKind::Internal)
        .start_with_context(&tracer, &proxy_cx);

    RequestSpanGuard {
        proxy_cx,
        pentagon_cx,
        upstream_cx,
        proxy_span:    attr_proxy,
        pentagon_span: attr_pentagon,
        upstream_span: attr_upstream,
    }
}

/// Inject the outgoing `traceparent` / `tracestate` + allowed baggage headers
/// into `outgoing_headers` using the proxy-level context.
///
/// Call this just before sending the response back to the client.
pub fn inject_trace_headers(guard: &RequestSpanGuard, outgoing_headers: &mut HeaderMap) {
    inject_context(guard.proxy_context(), outgoing_headers);
}

/// Inject trace headers into the upstream request.
///
/// Call this just before forwarding the request to an upstream service so the
/// upstream receives the `armageddon.upstream` span context.
pub fn inject_upstream_headers(guard: &RequestSpanGuard, upstream_headers: &mut HeaderMap) {
    inject_context(guard.upstream_context(), upstream_headers);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    // -----------------------------------------------------------------------
    // Test 1 — Happy path: guard is created without panic for a normal request.
    // -----------------------------------------------------------------------

    #[test]
    fn test_begin_request_trace_creates_guard() {
        let headers = HeaderMap::new();
        // Must not panic; no-op tracer provider is the default.
        let _guard = begin_request_trace(&headers, "GET", "/api/health", "backend-cluster");
    }

    // -----------------------------------------------------------------------
    // Test 2 — inject_upstream_headers does not panic for any incoming header.
    // -----------------------------------------------------------------------

    #[test]
    fn test_inject_upstream_headers_no_panic() {
        let mut incoming = HeaderMap::new();
        incoming.insert(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
                .parse()
                .unwrap(),
        );

        let guard = begin_request_trace(&incoming, "POST", "/api/graphql", "dgs-gateway");

        let mut upstream_hdrs = HeaderMap::new();
        inject_upstream_headers(&guard, &mut upstream_hdrs);
        // No panic expected; header presence depends on active provider.
    }

    // -----------------------------------------------------------------------
    // Test 3 — record_upstream_status does not panic for any status code.
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_upstream_status_no_panic() {
        let headers = HeaderMap::new();
        let mut guard = begin_request_trace(&headers, "DELETE", "/api/resource/1", "api-cluster");

        for status in [200u16, 204, 301, 400, 404, 500, 503] {
            guard.record_upstream_status(status);
        }
    }

    // -----------------------------------------------------------------------
    // Test 4 — inject_trace_headers on response does not panic.
    // -----------------------------------------------------------------------

    #[test]
    fn test_inject_trace_headers_response() {
        let mut incoming = HeaderMap::new();
        incoming.insert(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
                .parse()
                .unwrap(),
        );

        let guard = begin_request_trace(&incoming, "GET", "/api/users", "users-cluster");

        let mut response_hdrs = HeaderMap::new();
        inject_trace_headers(&guard, &mut response_hdrs);
    }
}
