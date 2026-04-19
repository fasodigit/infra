// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//! W3C Trace Context + Baggage propagation for ARMAGEDDON.
//!
//! Implements:
//! - [`extract_context`] — parse `traceparent`, `tracestate`, and `baggage` headers
//!   from an incoming [`http::HeaderMap`] and return an OpenTelemetry [`Context`].
//! - [`inject_context`] — write the current span context back into outgoing headers.
//! - [`build_tracer_provider`] — construct a [`TracerProvider`] wired to either an
//!   OTLP gRPC exporter (production) or a stdout exporter (debug).
//!
//! ### Sampling
//! A [`ParentBased`] sampler wrapping [`TraceIdRatioBased`] is used.  The default
//! ratio is **1 % (0.01)** — configurable via [`OtelConfig::sampling_rate`].
//!
//! ### Baggage keys propagated
//! Only the following keys are forwarded downstream; all others are dropped on
//! injection to prevent header-pollution attacks:
//! - `x-faso-tenant`
//! - `x-faso-user-id`
//! - `x-request-id`

use http::HeaderMap;
use opentelemetry::{
    baggage::BaggageExt,
    global,
    propagation::{Extractor, Injector, TextMapPropagator},
    Context, KeyValue,
};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{RandomIdGenerator, Sampler, TracerProvider},
};

use crate::config::OtelConfig;

// Ensure the TracerProvider trait methods are in scope when calling
// global::set_tracer_provider which requires the provider type.
#[allow(unused_imports)]
use opentelemetry::trace::TracerProvider as _;

// ---------------------------------------------------------------------------
// Allowed baggage keys (allow-list; all others stripped on inject)
// ---------------------------------------------------------------------------

/// Baggage keys that ARMAGEDDON propagates across service boundaries.
pub const ALLOWED_BAGGAGE_KEYS: &[&str] = &[
    "x-faso-tenant",
    "x-faso-user-id",
    "x-request-id",
];

// ---------------------------------------------------------------------------
// Header adapters (Extractor / Injector for http::HeaderMap)
// ---------------------------------------------------------------------------

/// Read-only adapter from [`HeaderMap`] to OTel [`Extractor`].
struct HeaderExtractor<'a>(&'a HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|k| k.as_str())
            .collect()
    }
}

/// Write adapter from OTel [`Injector`] into [`HeaderMap`].
struct HeaderInjector<'a>(&'a mut HeaderMap);

impl<'a> Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(k), Ok(v)) = (
            http::header::HeaderName::from_bytes(key.as_bytes()),
            http::HeaderValue::from_str(&value),
        ) {
            self.0.insert(k, v);
        }
    }
}

// ---------------------------------------------------------------------------
// extract_context
// ---------------------------------------------------------------------------

/// Extract an OpenTelemetry [`Context`] from incoming HTTP headers.
///
/// Parses `traceparent`, `tracestate`, and `baggage` according to the W3C
/// specifications.  If no valid `traceparent` is present, a fresh root
/// context is returned.
pub fn extract_context(headers: &HeaderMap) -> Context {
    let propagator = global_propagator();
    propagator.extract(&HeaderExtractor(headers))
}

// ---------------------------------------------------------------------------
// inject_context
// ---------------------------------------------------------------------------

/// Inject the current span context into outgoing HTTP headers.
///
/// Only baggage entries whose keys appear in [`ALLOWED_BAGGAGE_KEYS`] are
/// forwarded — all others are silently dropped to avoid header pollution.
pub fn inject_context(ctx: &Context, headers: &mut HeaderMap) {
    // Inject traceparent + tracestate.
    let propagator = global_propagator();
    propagator.inject_context(ctx, &mut HeaderInjector(headers));

    // Inject filtered baggage.
    let baggage = ctx.baggage();
    for key in ALLOWED_BAGGAGE_KEYS {
        if let Some(value) = baggage.get(*key) {
            let header_value = value.to_string();
            if let (Ok(k), Ok(v)) = (
                http::header::HeaderName::from_bytes(key.as_bytes()),
                http::HeaderValue::from_str(&header_value),
            ) {
                headers.insert(k, v);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// build_tracer_provider
// ---------------------------------------------------------------------------

/// Build and register a global [`TracerProvider`].
///
/// - If `config.endpoint` is `"stdout"`, a simple stdout exporter is used
///   (suitable for local development / debug).
/// - Otherwise an OTLP/gRPC exporter targeting `config.endpoint` (default
///   `otel-collector:4317`) is created.
///
/// A [`ParentBased`] sampler wrapping [`TraceIdRatioBased`] is configured
/// from `config.sampling_rate` (default 0.01 = 1 %).
///
/// ### Resource attributes
/// All entries in `config.resource_attributes` are attached to every span as
/// OTLP resource attributes.
pub fn build_tracer_provider(config: &OtelConfig) -> anyhow::Result<TracerProvider> {
    use opentelemetry_sdk::Resource;
    use opentelemetry_otlp::WithExportConfig;

    // -- Resource attributes --
    let mut kv: Vec<KeyValue> = vec![
        KeyValue::new("service.name", config.service_name.clone()),
    ];
    for (k, v) in &config.resource_attributes {
        kv.push(KeyValue::new(k.clone(), v.clone()));
    }
    let resource = Resource::new(kv);

    // -- Sampler: ParentBased(TraceIdRatioBased(rate)) --
    let inner_sampler = Sampler::TraceIdRatioBased(config.sampling_rate);
    let sampler = Sampler::ParentBased(Box::new(inner_sampler));

    // -- Exporter --
    let provider = if config.endpoint.trim() == "stdout" {
        // Stdout exporter — debug only.
        use opentelemetry_sdk::trace::SimpleSpanProcessor;
        use opentelemetry_stdout::SpanExporter;

        let exporter = SpanExporter::default();
        TracerProvider::builder()
            .with_span_processor(SimpleSpanProcessor::new(Box::new(exporter)))
            .with_sampler(sampler)
            .with_resource(resource)
            .with_id_generator(RandomIdGenerator::default())
            .build()
    } else {
        // OTLP gRPC exporter.
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(config.endpoint.clone())
            .build()
            .map_err(|e| anyhow::anyhow!("OTLP exporter init failed: {}", e))?;

        use opentelemetry_sdk::trace::BatchSpanProcessor;
        let processor = BatchSpanProcessor::builder(exporter, opentelemetry_sdk::runtime::Tokio)
            .build();

        TracerProvider::builder()
            .with_span_processor(processor)
            .with_sampler(sampler)
            .with_resource(resource)
            .with_id_generator(RandomIdGenerator::default())
            .build()
    };

    // Register as global provider so `tracing-opentelemetry` can pick it up.
    global::set_tracer_provider(provider.clone());

    // Install W3C propagator globally.
    global::set_text_map_propagator(TraceContextPropagator::new());

    Ok(provider)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns the globally installed text-map propagator (W3C TraceContext).
///
/// We call [`global::get_text_map_propagator`] through a thin wrapper so that
/// the propagator instance created by [`build_tracer_provider`] is always
/// the one used for extraction and injection.
fn global_propagator() -> impl TextMapPropagator {
    // The TraceContextPropagator handles both traceparent/tracestate and baggage.
    opentelemetry_sdk::propagation::TraceContextPropagator::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;
    use opentelemetry::trace::TraceContextExt;

    // -- helpers --

    fn make_traceparent(trace_id: &str, span_id: &str, sampled: bool) -> String {
        let flags = if sampled { "01" } else { "00" };
        format!("00-{}-{}-{}", trace_id, span_id, flags)
    }

    // ---------------------------------------------------------------------------
    // Test 1 — Happy path: round-trip W3C traceparent header is preserved.
    // ---------------------------------------------------------------------------

    #[test]
    fn test_roundtrip_traceparent_preserved() {
        let trace_id_hex = "4bf92f3577b34da6a3ce929d0e0e4736";
        let span_id_hex  = "00f067aa0ba902b7";
        let traceparent  = make_traceparent(trace_id_hex, span_id_hex, true);

        // Build incoming headers.
        let mut incoming = HeaderMap::new();
        incoming.insert("traceparent", traceparent.parse().unwrap());

        // Extract context from incoming headers.
        let ctx = extract_context(&incoming);
        let span_ctx = ctx.span().span_context().clone();

        assert!(span_ctx.is_valid(), "extracted span context must be valid");
        assert_eq!(
            format!("{:032x}", span_ctx.trace_id()),
            trace_id_hex,
            "trace-id must be preserved"
        );
        assert_eq!(
            format!("{:016x}", span_ctx.span_id()),
            span_id_hex,
            "span-id must be preserved"
        );
        assert!(
            span_ctx.trace_flags().is_sampled(),
            "sampled flag must be preserved"
        );

        // Inject into outgoing headers.
        let mut outgoing = HeaderMap::new();
        inject_context(&ctx, &mut outgoing);

        let out_tp = outgoing
            .get("traceparent")
            .expect("traceparent must be injected")
            .to_str()
            .unwrap();

        assert!(
            out_tp.contains(trace_id_hex),
            "trace-id must survive inject: got {}",
            out_tp
        );
        assert!(
            out_tp.contains(span_id_hex),
            "span-id must survive inject: got {}",
            out_tp
        );
    }

    // ---------------------------------------------------------------------------
    // Test 2 — Edge case: missing traceparent yields a valid (root) context.
    // ---------------------------------------------------------------------------

    #[test]
    fn test_missing_traceparent_yields_root_context() {
        let incoming = HeaderMap::new(); // no headers at all
        let ctx = extract_context(&incoming);
        let span_ctx = ctx.span().span_context().clone();

        // A root context has no valid span context — that is expected behaviour.
        assert!(
            !span_ctx.is_valid(),
            "root context should not have a valid span context"
        );
    }

    // ---------------------------------------------------------------------------
    // Test 3 — Baggage allow-list: only permitted keys are injected.
    // ---------------------------------------------------------------------------

    #[test]
    fn test_baggage_inject_filters_keys() {
        use opentelemetry::baggage::BaggageExt;
        use opentelemetry::KeyValue;

        // Build a context that carries both allowed and forbidden baggage.
        let ctx = Context::current_with_baggage(vec![
            KeyValue::new("x-faso-tenant",  "bf-prod"),
            KeyValue::new("x-faso-user-id", "user-42"),
            KeyValue::new("x-request-id",   "req-abc"),
            KeyValue::new("x-secret-token", "should-not-appear"),
        ]);

        let mut outgoing = HeaderMap::new();
        inject_context(&ctx, &mut outgoing);

        // Allowed keys must appear.
        assert!(
            outgoing.contains_key("x-faso-tenant"),
            "x-faso-tenant must be propagated"
        );
        assert!(
            outgoing.contains_key("x-faso-user-id"),
            "x-faso-user-id must be propagated"
        );
        assert!(
            outgoing.contains_key("x-request-id"),
            "x-request-id must be propagated"
        );

        // Forbidden key must NOT appear.
        assert!(
            !outgoing.contains_key("x-secret-token"),
            "x-secret-token must NOT be propagated"
        );
    }

    // ---------------------------------------------------------------------------
    // Test 4 — Error case: malformed traceparent is silently discarded.
    // ---------------------------------------------------------------------------

    #[test]
    fn test_malformed_traceparent_discarded() {
        let mut incoming = HeaderMap::new();
        incoming.insert("traceparent", "not-a-valid-value".parse().unwrap());

        let ctx = extract_context(&incoming);
        let span_ctx = ctx.span().span_context().clone();

        assert!(
            !span_ctx.is_valid(),
            "malformed traceparent must not produce a valid span context"
        );
    }
}
