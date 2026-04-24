// SPDX-License-Identifier: AGPL-3.0-or-later
//! gRPC-Web translation layer for the Pingora gateway (M4-2 wave 2).
//!
//! Bridges browser/mobile clients speaking **gRPC-Web** (HTTP/1.1-compatible
//! framing) to upstream services that speak native **gRPC** (HTTP/2, binary
//! framing).
//!
//! ## Protocol overview
//!
//! ```text
//! Client (browser)              ARMAGEDDON-FORGE (Pingora)          Upstream (gRPC)
//!   grpc-web+proto  ──detect──► translate headers + body ──HTTP/2──► application/grpc+proto
//!                               ◄── wrap trailers in body ◄──────────
//! ```
//!
//! ## Frame format (gRPC / gRPC-Web shared)
//!
//! ```text
//! ┌───────┬───────────┬──────────────┐
//! │ flags │  length   │  data...     │
//! │ 1 byte│  4 bytes  │  N bytes     │
//! └───────┴───────────┴──────────────┘
//! ```
//! - `flags == 0x00` : data frame
//! - `flags == 0x80` : trailer frame (gRPC-Web only; carries `grpc-status`,
//!   `grpc-message`)
//!
//! ## gRPC-Web-text
//!
//! `application/grpc-web-text` bodies are base64-encoded.  The entire payload
//! (data frames + trailing trailer frame) is base64-encoded as one blob.
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | `Content-Type` not recognised | `UnsupportedContentType` error — caller should respond 415 |
//! | Base64 decode failure (text mode) | `BodyDecodeError` |
//! | Truncated 5-byte frame header | `InvalidFrame` |
//! | Upstream gRPC returns non-OK HTTP | `UpstreamUnavailable` |
//!
//! ## Integration in the Pingora filter chain
//!
//! Detection happens in `request_filter` (via [`detect_grpc_web`]) which
//! sets `ctx.grpc_web_mode`.  The `upstream_request_filter` hook then converts
//! the `Content-Type` to `application/grpc+proto` so the upstream sees native
//! gRPC.  On the response path, `response_body_filter` wraps the upstream body
//! with a trailer frame and optionally base64-encodes the result.
//!
//! Because Pingora 0.3 does not expose a streaming body accumulator, the
//! current implementation accumulates the full upstream body in memory before
//! re-framing.  This is functionally correct but not ideal for large server-
//! streaming responses.  TODO(M5): switch to chunk-level framing once Pingora
//! 0.4 exposes `response_body_filter` with a mutable accumulation buffer.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use bytes::{Buf, BufMut, Bytes, BytesMut};

// ── Error type ─────────────────────────────────────────────────────────────

/// Errors produced by the gRPC-Web transcoder.
#[derive(Debug, thiserror::Error)]
pub enum GrpcWebError {
    /// `Content-Type` is not a gRPC-Web content type.
    #[error("unsupported Content-Type for gRPC-Web transcoding: '{0}'")]
    UnsupportedContentType(String),

    /// Body could not be decoded (base64 or frame parsing).
    #[error("body decode error: {0}")]
    BodyDecodeError(String),

    /// A framing invariant was violated (truncated header, wrong flag, etc.).
    #[error("invalid gRPC frame: {0}")]
    InvalidFrame(String),

    /// Upstream gRPC service returned a non-OK transport error.
    #[error("upstream gRPC service unavailable: {0}")]
    UpstreamUnavailable(String),
}

// ── Variant detection ──────────────────────────────────────────────────────

/// Recognised gRPC-Web content-type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcWebVariant {
    /// `application/grpc-web+proto` — binary body.
    Binary,
    /// `application/grpc-web-text` — base64-encoded body.
    Text,
}

impl GrpcWebVariant {
    /// Parse a `Content-Type` header value.
    ///
    /// Returns `None` when the value is not a gRPC-Web content type.
    pub fn from_content_type(ct: &str) -> Option<Self> {
        let ct = ct.trim();
        if ct.starts_with("application/grpc-web-text") {
            Some(Self::Text)
        } else if ct.starts_with("application/grpc-web") {
            Some(Self::Binary)
        } else {
            None
        }
    }

    /// Matching response `Content-Type` value for the downstream client.
    pub fn response_content_type(self) -> &'static str {
        match self {
            Self::Binary => "application/grpc-web+proto",
            Self::Text => "application/grpc-web-text",
        }
    }
}

/// Detect gRPC-Web from a `Content-Type` string.
///
/// Returns `Some(GrpcWebVariant)` when the request carries a gRPC-Web
/// content type, `None` otherwise.  Callers should call this from
/// `request_filter` and store the result in `ctx.grpc_web_mode`.
pub fn detect_grpc_web(content_type: Option<&str>) -> Option<GrpcWebVariant> {
    GrpcWebVariant::from_content_type(content_type?)
}

// ── Frame codec ────────────────────────────────────────────────────────────

/// Parse the leading data frame (flags=0x00) from a buffer.
///
/// Returns `(message_bytes, remaining_bytes)` or an error when the buffer is
/// truncated or the frame flags are unexpected.
pub fn parse_grpc_frame(mut buf: Bytes) -> Result<(Bytes, Bytes), GrpcWebError> {
    if buf.len() < 5 {
        return Err(GrpcWebError::InvalidFrame(format!(
            "need ≥5 bytes for frame header, got {}",
            buf.len()
        )));
    }
    let flags = buf.get_u8();
    let length = buf.get_u32() as usize;

    if flags != 0x00 {
        return Err(GrpcWebError::InvalidFrame(format!(
            "expected data frame (flags=0x00), got 0x{flags:02x}"
        )));
    }
    if buf.len() < length {
        return Err(GrpcWebError::InvalidFrame(format!(
            "frame declares {length} bytes but only {} remain",
            buf.len()
        )));
    }
    let data = buf.split_to(length);
    Ok((data, buf))
}

/// Build a 5-byte length-prefixed gRPC data frame (flags=0x00).
pub fn build_grpc_frame(payload: &[u8]) -> Bytes {
    let mut out = BytesMut::with_capacity(5 + payload.len());
    out.put_u8(0x00);
    out.put_u32(payload.len() as u32);
    out.put_slice(payload);
    out.freeze()
}

/// Build a gRPC-Web trailer frame (flags=0x80).
///
/// The trailer payload is formatted as HTTP/1.1-style header lines:
/// `grpc-status: 0\r\ngrpc-message: \r\n`.
pub fn build_trailer_frame(grpc_status: u32, grpc_message: &str) -> Bytes {
    let trailer_text = format!(
        "grpc-status: {grpc_status}\r\ngrpc-message: {grpc_message}\r\n"
    );
    let payload = trailer_text.as_bytes();
    let mut out = BytesMut::with_capacity(5 + payload.len());
    out.put_u8(0x80);
    out.put_u32(payload.len() as u32);
    out.put_slice(payload);
    out.freeze()
}

/// Parse a trailer frame payload back to `(grpc_status, grpc_message)`.
///
/// The `payload` parameter is the raw bytes **after** the 5-byte frame header.
pub fn parse_trailer_payload(payload: &[u8]) -> (u32, String) {
    let text = String::from_utf8_lossy(payload);
    let mut status = 0u32;
    let mut message = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("grpc-status:") {
            status = v.trim().parse().unwrap_or(2);
        } else if let Some(v) = line.strip_prefix("grpc-message:") {
            message = v.trim().to_string();
        }
    }
    (status, message)
}

// ── Body transcoder ────────────────────────────────────────────────────────

/// Configuration for the gRPC-Web body transcoder used in the response path.
#[derive(Debug, Clone)]
pub struct GrpcWebConfig {
    /// Whether to inject CORS `Access-Control-Expose-Headers` for gRPC-Web
    /// trailers.  Defaults to `true`.
    pub inject_cors_expose_headers: bool,
}

impl Default for GrpcWebConfig {
    fn default() -> Self {
        Self {
            inject_cors_expose_headers: true,
        }
    }
}

/// Assemble a complete gRPC-Web response body from an upstream gRPC body and
/// the extracted trailer values.
///
/// Layout: `[ upstream data frames verbatim ][ gRPC-Web trailer frame ]`
///
/// The trailing frame carries `grpc-status` and `grpc-message` (already
/// extracted from the upstream HTTP/2 trailers by the caller).
///
/// When `variant == Text`, the final bytes are base64-encoded.
pub fn assemble_grpc_web_body(
    upstream_body: &[u8],
    grpc_status: u32,
    grpc_message: &str,
    variant: GrpcWebVariant,
) -> Bytes {
    let mut out = BytesMut::with_capacity(upstream_body.len() + 32);
    out.extend_from_slice(upstream_body);
    out.extend_from_slice(&build_trailer_frame(grpc_status, grpc_message));
    let raw = out.freeze();

    match variant {
        GrpcWebVariant::Binary => raw,
        GrpcWebVariant::Text => Bytes::from(B64.encode(&raw)),
    }
}

/// Decode a gRPC-Web-text (base64-encoded) request body into raw gRPC bytes.
///
/// Returns `Err(GrpcWebError::BodyDecodeError)` on invalid base64.
pub fn decode_grpc_web_text_body(body: &[u8]) -> Result<Bytes, GrpcWebError> {
    let decoded = B64
        .decode(body)
        .map_err(|e| GrpcWebError::BodyDecodeError(e.to_string()))?;
    Ok(Bytes::from(decoded))
}

/// CORS headers required for gRPC-Web browser compatibility.
///
/// Exposes `grpc-status`, `grpc-message`, and `grpc-encoding` so that browser
/// JS clients can read the gRPC-Web trailers.
///
/// Returns a `(header-name, header-value)` pair slice suitable for iteration.
pub fn grpc_web_cors_expose_headers() -> &'static str {
    "grpc-status, grpc-message, grpc-encoding"
}

// ── Pingora pipeline helpers ───────────────────────────────────────────────

/// Check whether incoming request headers indicate a CORS preflight for
/// gRPC-Web.
///
/// Returns `true` when `Method == OPTIONS` and `Access-Control-Request-Method`
/// contains `POST` — the standard CORS preflight pattern.
pub fn is_grpc_web_preflight(method: &str, headers: &[(&str, &str)]) -> bool {
    if !method.eq_ignore_ascii_case("OPTIONS") {
        return false;
    }
    headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("access-control-request-method")
            && v.eq_ignore_ascii_case("POST")
    })
}

/// Build the upstream `Content-Type` replacement value for a gRPC-Web
/// request being forwarded as native gRPC.
///
/// Both binary and text variants map to `application/grpc+proto` upstream.
pub fn upstream_grpc_content_type() -> &'static str {
    "application/grpc+proto"
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- variant detection ---------------------------------------------------

    #[test]
    fn detect_binary_variant() {
        assert_eq!(
            GrpcWebVariant::from_content_type("application/grpc-web+proto"),
            Some(GrpcWebVariant::Binary)
        );
        assert_eq!(
            GrpcWebVariant::from_content_type("application/grpc-web"),
            Some(GrpcWebVariant::Binary)
        );
    }

    #[test]
    fn detect_text_variant() {
        assert_eq!(
            GrpcWebVariant::from_content_type("application/grpc-web-text"),
            Some(GrpcWebVariant::Text)
        );
        assert_eq!(
            GrpcWebVariant::from_content_type("application/grpc-web-text+proto"),
            Some(GrpcWebVariant::Text)
        );
    }

    #[test]
    fn detect_non_grpc_web_returns_none() {
        assert!(GrpcWebVariant::from_content_type("application/json").is_none());
        assert!(GrpcWebVariant::from_content_type("application/grpc+proto").is_none());
        assert!(GrpcWebVariant::from_content_type("").is_none());
    }

    #[test]
    fn detect_grpc_web_helper() {
        assert_eq!(
            detect_grpc_web(Some("application/grpc-web+proto")),
            Some(GrpcWebVariant::Binary)
        );
        assert!(detect_grpc_web(None).is_none());
        assert!(detect_grpc_web(Some("text/html")).is_none());
    }

    // -- frame codec ---------------------------------------------------------

    #[test]
    fn data_frame_roundtrip() {
        let payload = b"hello world";
        let framed = build_grpc_frame(payload);
        assert_eq!(framed.len(), 5 + payload.len());
        assert_eq!(framed[0], 0x00);

        let (data, remainder) = parse_grpc_frame(framed).expect("parse ok");
        assert_eq!(data.as_ref(), payload);
        assert!(remainder.is_empty());
    }

    #[test]
    fn data_frame_with_trailing_bytes() {
        let payload = b"first";
        let extra = b"extra";
        let mut framed = BytesMut::new();
        framed.extend_from_slice(&build_grpc_frame(payload));
        framed.extend_from_slice(extra);

        let (data, remainder) = parse_grpc_frame(framed.freeze()).expect("parse");
        assert_eq!(data.as_ref(), payload);
        assert_eq!(remainder.as_ref(), extra);
    }

    #[test]
    fn data_frame_truncated_returns_error() {
        let truncated = Bytes::from_static(b"\x00\x00\x00");
        let err = parse_grpc_frame(truncated).unwrap_err();
        assert!(matches!(err, GrpcWebError::InvalidFrame(_)));
    }

    #[test]
    fn data_frame_wrong_flag_returns_error() {
        // Build a frame with flags=0x80 (trailer) but call parse_grpc_frame expecting 0x00.
        let frame = build_trailer_frame(0, "");
        let err = parse_grpc_frame(frame).unwrap_err();
        assert!(matches!(err, GrpcWebError::InvalidFrame(_)));
    }

    // -- trailer codec -------------------------------------------------------

    #[test]
    fn trailer_frame_ok_status() {
        let frame = build_trailer_frame(0, "");
        assert_eq!(frame[0], 0x80, "trailer flag must be 0x80");
        let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
        let payload = &frame[5..5 + length];
        let (status, message) = parse_trailer_payload(payload);
        assert_eq!(status, 0);
        assert_eq!(message, "");
    }

    #[test]
    fn trailer_frame_error_status() {
        let frame = build_trailer_frame(3, "invalid argument");
        let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
        let payload = &frame[5..5 + length];
        let (status, message) = parse_trailer_payload(payload);
        assert_eq!(status, 3);
        assert_eq!(message, "invalid argument");
    }

    // -- body assembly -------------------------------------------------------

    #[test]
    fn assemble_binary_body() {
        let message = b"response_data";
        let upstream_body = build_grpc_frame(message);

        let assembled = assemble_grpc_web_body(&upstream_body, 0, "", GrpcWebVariant::Binary);

        // First frame: data (0x00)
        assert_eq!(assembled[0], 0x00);
        let data_len = u32::from_be_bytes([
            assembled[1], assembled[2], assembled[3], assembled[4],
        ]) as usize;
        assert_eq!(data_len, message.len());

        // Trailer frame follows immediately
        let trailer_offset = 5 + data_len;
        assert_eq!(assembled[trailer_offset], 0x80, "trailer flag");
        let t_len = u32::from_be_bytes([
            assembled[trailer_offset + 1],
            assembled[trailer_offset + 2],
            assembled[trailer_offset + 3],
            assembled[trailer_offset + 4],
        ]) as usize;
        let t_payload = &assembled[trailer_offset + 5..trailer_offset + 5 + t_len];
        let (status, _) = parse_trailer_payload(t_payload);
        assert_eq!(status, 0);
    }

    #[test]
    fn assemble_text_body_is_valid_base64() {
        let assembled =
            assemble_grpc_web_body(b"", 0, "", GrpcWebVariant::Text);
        // Must be valid base64.
        let decoded = B64.decode(assembled.as_ref()).expect("valid base64");
        // Decoded starts with trailer frame (0x80) since no data.
        assert_eq!(decoded[0], 0x80);
    }

    #[test]
    fn text_body_decode_roundtrip() {
        let original = b"\x00\x00\x00\x00\x05hello";
        let encoded = B64.encode(original);
        let decoded = decode_grpc_web_text_body(encoded.as_bytes()).expect("ok");
        assert_eq!(decoded.as_ref(), original);
    }

    #[test]
    fn text_body_decode_error_on_invalid_base64() {
        let err = decode_grpc_web_text_body(b"!!! NOT BASE64 !!!").unwrap_err();
        assert!(matches!(err, GrpcWebError::BodyDecodeError(_)));
    }

    // -- CORS preflight detection --------------------------------------------

    #[test]
    fn cors_preflight_detected() {
        let headers = vec![("access-control-request-method", "POST")];
        assert!(is_grpc_web_preflight("OPTIONS", &headers));
    }

    #[test]
    fn cors_preflight_not_options_method() {
        let headers = vec![("access-control-request-method", "POST")];
        assert!(!is_grpc_web_preflight("GET", &headers));
    }

    #[test]
    fn cors_preflight_missing_header() {
        assert!(!is_grpc_web_preflight("OPTIONS", &[]));
    }

    // -- upstream helpers ----------------------------------------------------

    #[test]
    fn upstream_content_type_is_grpc_proto() {
        assert_eq!(upstream_grpc_content_type(), "application/grpc+proto");
    }

    #[test]
    fn cors_expose_headers_contains_grpc_status() {
        let h = grpc_web_cors_expose_headers();
        assert!(h.contains("grpc-status"));
        assert!(h.contains("grpc-message"));
    }

    // -- server streaming simulation (multi-frame body) ----------------------

    #[test]
    fn server_streaming_multiple_frames() {
        // Simulate 3 data frames from upstream, then a trailer.
        let frames: &[&[u8]] = &[b"frame1", b"frame2", b"frame3"];
        let mut upstream_body = BytesMut::new();
        for f in frames {
            upstream_body.extend_from_slice(&build_grpc_frame(f));
        }

        let assembled = assemble_grpc_web_body(
            &upstream_body,
            0,
            "ok",
            GrpcWebVariant::Binary,
        );

        // Walk frames.
        let mut remaining = assembled.clone();
        let mut decoded_frames: Vec<Bytes> = Vec::new();
        for _ in 0..3 {
            let (data, rest) = parse_grpc_frame(remaining).expect("data frame");
            decoded_frames.push(data);
            remaining = rest;
        }
        assert_eq!(decoded_frames[0].as_ref(), b"frame1");
        assert_eq!(decoded_frames[1].as_ref(), b"frame2");
        assert_eq!(decoded_frames[2].as_ref(), b"frame3");

        // Trailer frame last.
        assert_eq!(remaining[0], 0x80, "trailer flag");
    }
}
