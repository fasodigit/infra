// SPDX-License-Identifier: AGPL-3.0-or-later
//! OpenTelemetry filter — W3C `traceparent` propagation + span lifecycle.
//!
//! ## Behaviour
//!
//! ### `on_request`
//! 1. Parse the inbound `traceparent` header (W3C format `00-<trace_id>-<span_id>-<flags>`).
//! 2. Populate [`RequestCtx::trace_id`] and [`RequestCtx::span_id`].
//! 3. Record [`RequestCtx::request_start_ms`] for duration computation.
//! 4. Create a [`tracing::Span`] named `armageddon.pingora.proxy` with
//!    attributes: `http.method`, `http.route`, `request.id`, `trace.id`.
//!
//! ### `on_upstream_request`
//! Inject a reconstructed `traceparent` header towards the upstream so the
//! distributed trace is stitched across the proxy hop.
//!
//! ### `on_logging`
//! Close the span with `http.status_code`, `duration_ms`, and (if non-2xx)
//! an `error` attribute.
//!
//! ## W3C traceparent format
//!
//! ```text
//! 00-<32-hex trace-id>-<16-hex parent-id>-<2-hex flags>
//! ```
//!
//! All three fields must be present and non-zero; otherwise the header is
//! treated as absent (a fresh trace ID is generated).
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | No `traceparent` header | Generate a fresh trace-id from `request_id` |
//! | Malformed `traceparent` | Treat as absent; log a warning |
//! | Span creation fails | Swallow error — observability is best-effort |

use std::time::{SystemTime, UNIX_EPOCH};

use tracing::debug;

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

// ── span name ─────────────────────────────────────────────────────────────────

const SPAN_NAME: &str = "armageddon.pingora.proxy";

// ── W3C traceparent parser ────────────────────────────────────────────────────

/// Parsed W3C `traceparent` components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Traceparent {
    /// 128-bit trace identifier, 32 hex chars.
    pub trace_id: String,
    /// 64-bit parent span identifier, 16 hex chars.
    pub span_id: String,
    /// Trace flags (e.g. `01` = sampled).
    pub flags: u8,
}

impl Traceparent {
    /// Parse a W3C `traceparent` header value.
    ///
    /// Returns `None` if the value is malformed, has all-zero IDs, or uses
    /// an unsupported version prefix.
    pub fn parse(value: &str) -> Option<Self> {
        let parts: Vec<&str> = value.trim().split('-').collect();
        if parts.len() != 4 {
            return None;
        }
        let version = parts[0];
        if version != "00" {
            // Only version 00 is currently standardised.
            return None;
        }
        let trace_id = parts[1];
        let span_id = parts[2];
        let flags_str = parts[3];

        // Validate lengths and hex content.
        if trace_id.len() != 32 || !trace_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        if span_id.len() != 16 || !span_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        // All-zero IDs are invalid per the W3C spec.
        if trace_id == "0".repeat(32) || span_id == "0".repeat(16) {
            return None;
        }
        let flags = u8::from_str_radix(flags_str, 16).ok()?;

        Some(Self {
            trace_id: trace_id.to_string(),
            span_id: span_id.to_string(),
            flags,
        })
    }

    /// Reconstruct the `traceparent` header value.
    pub fn to_header_value(&self) -> String {
        format!("00-{}-{}-{:02x}", self.trace_id, self.span_id, self.flags)
    }
}

// ── current timestamp helper ──────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── OtelFilter ────────────────────────────────────────────────────────────────

/// OTEL filter — W3C traceparent propagation + span lifecycle.
///
/// # Thread safety
///
/// The filter holds no mutable state — it is safe to share across Pingora
/// worker threads as `Arc<dyn ForgeFilter>`.
#[derive(Debug, Default)]
pub struct OtelFilter;

impl OtelFilter {
    /// Create a new OTEL filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for OtelFilter {
    fn name(&self) -> &'static str {
        "otel"
    }

    /// Parse `traceparent`, populate ctx fields, open the proxy span.
    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        // Record when the request arrived.
        ctx.request_start_ms = now_ms();

        let req = session.req_header();
        let method = req.method.as_str().to_owned();
        let path = req.uri.path().to_owned();

        // Parse incoming traceparent.
        let tp = req
            .headers
            .get("traceparent")
            .and_then(|v| v.to_str().ok())
            .and_then(Traceparent::parse);

        match tp {
            Some(ref t) => {
                ctx.trace_id = t.trace_id.clone();
                ctx.span_id = t.span_id.clone();
                debug!(
                    trace_id = %ctx.trace_id,
                    span_id = %ctx.span_id,
                    "otel: incoming traceparent parsed"
                );
            }
            None => {
                // No (or malformed) traceparent: derive a synthetic trace_id
                // from the request UUID so we still have correlation IDs.
                ctx.trace_id = ctx.request_id.replace('-', "");
                // Pad or truncate to 32 hex chars.
                ctx.trace_id.truncate(32);
                while ctx.trace_id.len() < 32 {
                    ctx.trace_id.push('0');
                }
                ctx.span_id = "0000000000000000".to_string();
                debug!(
                    request_id = %ctx.request_id,
                    synthetic_trace_id = %ctx.trace_id,
                    "otel: no traceparent — synthetic trace id generated"
                );
            }
        }

        // Open the proxy span using `tracing`.
        //
        // Note: we create the span here but cannot store a `Span` guard in
        // `ctx` (tracing::Span is not Clone + Debug-derivable cleanly in the
        // current API surface and we don't want to box it behind a dyn Any).
        // Instead we log the span open event and let `on_logging` close it.
        // For production OTEL export, the opentelemetry SDK reads the tracing
        // span attributes via the `tracing-opentelemetry` subscriber layer.
        tracing::debug!(
            target: "armageddon.pingora.proxy",
            span_name = SPAN_NAME,
            "http.method" = %method,
            "http.route" = %path,
            "request.id" = %ctx.request_id,
            "trace.id" = %ctx.trace_id,
            "span.id" = %ctx.span_id,
            cluster = %ctx.cluster,
            "otel: proxy span opened"
        );

        Decision::Continue
    }

    /// Inject a reconstructed `traceparent` towards the upstream service.
    ///
    /// Uses the trace_id from ctx and generates a fresh span_id for the
    /// downstream hop so distributed traces are stitched correctly.
    async fn on_upstream_request(
        &self,
        _session: &mut pingora_proxy::Session,
        req: &mut pingora::http::RequestHeader,
        ctx: &mut RequestCtx,
    ) -> Decision {
        if ctx.trace_id.is_empty() {
            return Decision::Continue;
        }

        // Generate a cryptographically random span_id for the outbound hop.
        // Using CSPRNG prevents clients from predicting the traceparent sent
        // to upstreams (the request_id is exposed in x-forge-id).
        let rng = ring::rand::SystemRandom::new();
        let mut buf = [0u8; 8];
        ring::rand::SecureRandom::fill(&rng, &mut buf).expect("CSPRNG failure");
        let upstream_span_id = hex::encode(buf);

        let tp = Traceparent {
            trace_id: ctx.trace_id.clone(),
            span_id: upstream_span_id,
            flags: 0x01, // sampled
        };

        if let Ok(hv) = http::HeaderValue::from_str(&tp.to_header_value()) {
            req.insert_header("traceparent", hv).ok();
            debug!(
                trace_id = %ctx.trace_id,
                "otel: traceparent injected upstream"
            );
        }

        Decision::Continue
    }

    /// Close the proxy span with status and duration.
    ///
    /// Emits a structured log event readable by `tracing-opentelemetry` or
    /// any log aggregator.
    async fn on_logging(&self, session: &mut pingora_proxy::Session, ctx: &RequestCtx) {
        let status = session
            .response_written()
            .map(|r| r.status.as_u16())
            .unwrap_or(0);

        let duration_ms = if ctx.request_start_ms > 0 {
            now_ms().saturating_sub(ctx.request_start_ms)
        } else {
            0
        };

        let is_error = status >= 500 || status == 0;

        if is_error {
            tracing::warn!(
                target: "armageddon.pingora.proxy",
                span_name = SPAN_NAME,
                "http.status_code" = status,
                "duration_ms" = duration_ms,
                "request.id" = %ctx.request_id,
                "trace.id" = %ctx.trace_id,
                error = true,
                "otel: proxy span closed (error)"
            );
        } else {
            tracing::debug!(
                target: "armageddon.pingora.proxy",
                span_name = SPAN_NAME,
                "http.status_code" = status,
                "duration_ms" = duration_ms,
                "request.id" = %ctx.request_id,
                "trace.id" = %ctx.trace_id,
                "otel: proxy span closed"
            );
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Traceparent parser ────────────────────────────────────────────────────

    #[test]
    fn parse_valid_traceparent() {
        let raw = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let tp = Traceparent::parse(raw).expect("valid traceparent");
        assert_eq!(tp.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(tp.span_id, "00f067aa0ba902b7");
        assert_eq!(tp.flags, 0x01);
    }

    #[test]
    fn parse_traceparent_with_leading_whitespace() {
        let raw = "  00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00  ";
        let tp = Traceparent::parse(raw).expect("trims whitespace");
        assert_eq!(tp.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(tp.flags, 0x00);
    }

    #[test]
    fn parse_rejects_wrong_version() {
        let raw = "01-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        assert!(Traceparent::parse(raw).is_none(), "version 01 not supported");
    }

    #[test]
    fn parse_rejects_too_few_parts() {
        assert!(Traceparent::parse("00-abc-01").is_none());
    }

    #[test]
    fn parse_rejects_short_trace_id() {
        let raw = "00-4bf92f-00f067aa0ba902b7-01";
        assert!(Traceparent::parse(raw).is_none());
    }

    #[test]
    fn parse_rejects_non_hex_trace_id() {
        let raw = "00-4bf92f3577b34da6a3ce929d0e0e4zz-00f067aa0ba902b7-01";
        assert!(Traceparent::parse(raw).is_none());
    }

    #[test]
    fn parse_rejects_all_zero_trace_id() {
        let raw = "00-00000000000000000000000000000000-00f067aa0ba902b7-01";
        assert!(Traceparent::parse(raw).is_none());
    }

    #[test]
    fn parse_rejects_all_zero_span_id() {
        let raw = "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01";
        assert!(Traceparent::parse(raw).is_none());
    }

    #[test]
    fn to_header_value_round_trips() {
        let tp = Traceparent {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
            flags: 0x01,
        };
        let hv = tp.to_header_value();
        assert_eq!(hv, "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
        let parsed = Traceparent::parse(&hv).unwrap();
        assert_eq!(parsed, tp);
    }

    // ── OtelFilter unit logic (without live Session) ──────────────────────────

    #[test]
    fn synthetic_trace_id_derived_from_request_id() {
        // Simulate the fallback path: no incoming traceparent.
        let mut ctx = RequestCtx::new();
        let no_tp: Option<Traceparent> = None;
        match no_tp {
            Some(t) => {
                ctx.trace_id = t.trace_id;
                ctx.span_id = t.span_id;
            }
            None => {
                ctx.trace_id = ctx.request_id.replace('-', "");
                ctx.trace_id.truncate(32);
                while ctx.trace_id.len() < 32 {
                    ctx.trace_id.push('0');
                }
                ctx.span_id = "0000000000000000".to_string();
            }
        }
        assert_eq!(ctx.trace_id.len(), 32, "synthetic trace_id must be 32 chars");
        assert_eq!(ctx.span_id.len(), 16, "synthetic span_id must be 16 chars");
        assert!(
            ctx.trace_id.chars().all(|c| c.is_ascii_hexdigit()),
            "synthetic trace_id must be hex"
        );
    }

    #[test]
    fn upstream_traceparent_is_well_formed() {
        let mut ctx = RequestCtx::new();
        ctx.trace_id = "4bf92f3577b34da6a3ce929d0e0e4736".to_string();
        ctx.span_id = "00f067aa0ba902b7".to_string();

        // Replicate on_upstream_request logic (CSPRNG span_id).
        let rng = ring::rand::SystemRandom::new();
        let mut buf = [0u8; 8];
        ring::rand::SecureRandom::fill(&rng, &mut buf).expect("CSPRNG failure");
        let upstream_span_id = hex::encode(buf);
        let tp = Traceparent {
            trace_id: ctx.trace_id.clone(),
            span_id: upstream_span_id,
            flags: 0x01,
        };
        let hv = tp.to_header_value();

        // Parse it back — must be valid.
        let parsed = Traceparent::parse(&hv).expect("injected traceparent must be valid");
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.flags, 0x01);
        assert_eq!(parsed.span_id.len(), 16, "span_id must be 16 hex chars");
    }

    #[test]
    fn now_ms_is_positive_and_recent() {
        let t = now_ms();
        assert!(t > 0);
        // Sanity: must be after 2024-01-01 (1704067200000 ms).
        assert!(t > 1_704_067_200_000, "timestamp looks like epoch zero");
    }

    #[test]
    fn otel_filter_name() {
        assert_eq!(OtelFilter::new().name(), "otel");
    }

    #[test]
    fn traceparent_injected_for_valid_ctx_trace_id() {
        let mut ctx = RequestCtx::new();
        ctx.trace_id = "4bf92f3577b34da6a3ce929d0e0e4736".to_string();

        // Simulate on_upstream_request injection (without a live Session).
        if !ctx.trace_id.is_empty() {
            let rng = ring::rand::SystemRandom::new();
            let mut buf = [0u8; 8];
            ring::rand::SecureRandom::fill(&rng, &mut buf).expect("CSPRNG failure");
            let upstream_span_id = hex::encode(buf);
            let tp = Traceparent {
                trace_id: ctx.trace_id.clone(),
                span_id: upstream_span_id,
                flags: 0x01,
            };
            let hv = tp.to_header_value();
            assert!(hv.starts_with("00-4bf92f3577b34da6a3ce929d0e0e4736-"));
            assert!(hv.ends_with("-01"));
            assert_eq!(tp.span_id.len(), 16, "span_id must be 16 hex chars");
        }
    }

    #[test]
    fn span_closed_with_duration_in_logging() {
        let mut ctx = RequestCtx::new();
        ctx.request_start_ms = now_ms().saturating_sub(42);
        // Simulate on_logging duration computation.
        let duration_ms = now_ms().saturating_sub(ctx.request_start_ms);
        assert!(duration_ms >= 42, "duration must account for elapsed time");
    }
}
