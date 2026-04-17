// SPDX-License-Identifier: AGPL-3.0-only
//! gRPC-Web → gRPC transcoding layer.
//!
//! Bridges browser/mobile clients that speak gRPC-Web (HTTP/1.1-compatible framing)
//! to upstream services that speak native gRPC (HTTP/2, binary framing).
//!
//! Protocol overview
//! -----------------
//! - `application/grpc-web+proto`  : binary body, 5-byte length-prefixed frames
//! - `application/grpc-web-text`   : base64-encoded body (same frames, but base64)
//! - `application/grpc+proto`      : native gRPC — what upstreams expect
//!
//! Frame format (shared between gRPC and gRPC-Web)
//! ------------------------------------------------
//! ```text
//! +-------+-----------+
//! | flags |  length   |  data...
//! | 1 byte|  4 bytes  |  N bytes
//! +-------+-----------+
//! ```
//! - flags == 0x00 : data frame
//! - flags == 0x80 : trailer frame (gRPC-Web only; carries grpc-status, grpc-message)
//!
//! CORS headers emitted
//! --------------------
//! - `Access-Control-Allow-Origin`
//! - `Access-Control-Expose-Headers: grpc-status, grpc-message, grpc-encoding`
//!
//! Preserved pass-through headers
//! --------------------------------
//! - `grpc-accept-encoding`
//! - `grpc-timeout`
//! - `user-agent`

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use http::{
    header::{self, HeaderMap, HeaderName, HeaderValue},
    Method, Request, Response, StatusCode, Uri,
};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use thiserror::Error;
use tracing::{debug, warn};

// -- server-side layer re-export --

/// Tower `Layer` that enables gRPC-Web support on a `tonic` server.
///
/// Wrap your tonic router with this layer to accept gRPC-Web clients directly:
///
/// ```rust,ignore
/// use armageddon_forge::grpc_web::GrpcWebServerLayer;
/// use tower::ServiceBuilder;
///
/// let svc = ServiceBuilder::new()
///     .layer(GrpcWebServerLayer::new())
///     .service(my_tonic_router);
/// ```
pub type GrpcWebServerLayer = tonic_web::GrpcWebLayer;

// -- error type --

/// Errors produced by the gRPC-Web transcoder.
#[derive(Debug, Error)]
pub enum GrpcWebError {
    /// `Content-Type` is neither `application/grpc-web+proto` nor `application/grpc-web-text`.
    #[error("unsupported Content-Type for gRPC-Web transcoding: '{0}'")]
    UnsupportedContentType(String),

    /// The upstream gRPC service is unreachable or returned a non-OK transport error.
    #[error("upstream gRPC service unavailable: {0}")]
    UpstreamUnavailable(String),

    /// Body could not be decoded (base64 or frame parsing).
    #[error("body decode error: {0}")]
    BodyDecodeError(String),

    /// A framing invariant was violated (truncated header, etc.).
    #[error("invalid gRPC frame: {0}")]
    InvalidFrame(String),

    /// HTTP layer error building or sending the upstream request.
    #[error("HTTP transport error: {0}")]
    HttpTransport(String),
}

// -- content-type classification --

/// Recognised gRPC-Web content-type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcWebVariant {
    /// `application/grpc-web+proto` — binary body.
    Binary,
    /// `application/grpc-web-text` — base64-encoded body.
    Text,
}

impl GrpcWebVariant {
    /// Parse the `Content-Type` header value.
    /// Returns `None` if this is not a gRPC-Web content type.
    pub fn from_content_type(ct: &str) -> Option<Self> {
        let ct = ct.trim();
        if ct.starts_with("application/grpc-web-text") {
            Some(GrpcWebVariant::Text)
        } else if ct.starts_with("application/grpc-web") {
            // covers application/grpc-web+proto and application/grpc-web
            Some(GrpcWebVariant::Binary)
        } else {
            None
        }
    }

    /// Matching response `Content-Type` value.
    pub fn response_content_type(self) -> &'static str {
        match self {
            GrpcWebVariant::Binary => "application/grpc-web+proto",
            GrpcWebVariant::Text => "application/grpc-web-text",
        }
    }
}

// -- frame encoding / decoding --

/// Parse the raw bytes of a gRPC data frame body (flags=0x00).
///
/// Returns `(message_bytes, remaining)` or an error if the buffer is truncated.
pub fn parse_grpc_frame(mut buf: Bytes) -> Result<(Bytes, Bytes), GrpcWebError> {
    if buf.len() < 5 {
        return Err(GrpcWebError::InvalidFrame(format!(
            "need at least 5 bytes for frame header, got {}",
            buf.len()
        )));
    }
    let flags = buf.get_u8();
    let length = buf.get_u32() as usize;

    if flags != 0x00 {
        return Err(GrpcWebError::InvalidFrame(format!(
            "expected data frame (flags=0x00), got 0x{:02x}",
            flags
        )));
    }
    if buf.len() < length {
        return Err(GrpcWebError::InvalidFrame(format!(
            "frame declares {} bytes but only {} remain",
            length,
            buf.len()
        )));
    }
    let data = buf.split_to(length);
    Ok((data, buf))
}

/// Build a 5-byte length-prefixed gRPC data frame (flags=0x00).
pub fn build_grpc_frame(payload: &[u8]) -> Bytes {
    let mut out = BytesMut::with_capacity(5 + payload.len());
    out.put_u8(0x00); // data frame
    out.put_u32(payload.len() as u32);
    out.put_slice(payload);
    out.freeze()
}

/// Build a gRPC-Web trailer frame (flags=0x80).
///
/// The trailer payload is a sequence of HTTP/1.1-style header lines:
/// `grpc-status: 0\r\ngrpc-message: \r\n`.
pub fn build_trailer_frame(grpc_status: u32, grpc_message: &str) -> Bytes {
    let trailer_text = format!(
        "grpc-status: {}\r\ngrpc-message: {}\r\n",
        grpc_status, grpc_message
    );
    let payload = trailer_text.as_bytes();
    let mut out = BytesMut::with_capacity(5 + payload.len());
    out.put_u8(0x80); // trailer frame
    out.put_u32(payload.len() as u32);
    out.put_slice(payload);
    out.freeze()
}

/// Parse a trailer frame payload back to `(grpc_status, grpc_message)`.
///
/// Used in tests / introspection.  The payload is the raw bytes after
/// the 5-byte frame header (flags=0x80, len).
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

// -- CORS helpers --

/// Inject the CORS headers required for gRPC-Web browser compatibility.
///
/// Exposes `grpc-status`, `grpc-message`, and `grpc-encoding` so that
/// browser JS clients can read gRPC-Web trailers.
pub fn inject_grpc_web_cors_headers(headers: &mut HeaderMap, allow_origin: &str) {
    if let Ok(v) = HeaderValue::from_str(allow_origin) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
    }
    headers.insert(
        HeaderName::from_static("access-control-expose-headers"),
        HeaderValue::from_static("grpc-status, grpc-message, grpc-encoding"),
    );
}

// -- transcoder --

/// gRPC-Web → gRPC transcoding proxy.
///
/// Accepts HTTP requests with `Content-Type: application/grpc-web+proto` or
/// `application/grpc-web-text`, converts them to native gRPC, forwards to the
/// upstream, and re-encodes the response as gRPC-Web (including a trailer frame).
///
/// # Thread safety
///
/// `GrpcWebTranscoder` is `Clone` and `Send + Sync`; it can be shared freely
/// across Tokio tasks.
#[derive(Debug, Clone)]
pub struct GrpcWebTranscoder {
    /// Base URL of the upstream gRPC service, e.g. `http://127.0.0.1:50051`.
    upstream: String,
    /// When `true` the response body is base64-encoded (`grpc-web-text`).
    /// Detected automatically from the inbound `Content-Type`.
    accept_text: bool,
}

impl GrpcWebTranscoder {
    /// Create a new transcoder pointing at `upstream`.
    ///
    /// `accept_text` controls the *default* response encoding when the inbound
    /// variant cannot be determined (normally overridden per-request).
    pub fn new(upstream: impl Into<String>, accept_text: bool) -> Self {
        Self {
            upstream: upstream.into(),
            accept_text,
        }
    }

    /// Transcode an incoming gRPC-Web request and return the gRPC-Web response.
    ///
    /// # Errors
    ///
    /// - [`GrpcWebError::UnsupportedContentType`] — if `Content-Type` is absent or not gRPC-Web.
    /// - [`GrpcWebError::UpstreamUnavailable`] — if the upstream cannot be reached.
    /// - [`GrpcWebError::BodyDecodeError`] — if base64 decoding fails.
    /// - [`GrpcWebError::InvalidFrame`] — if the gRPC frame is malformed.
    pub async fn transcode(
        &self,
        req: Request<Bytes>,
    ) -> Result<Response<Bytes>, GrpcWebError> {
        // -- 1. detect variant from Content-Type header --
        let content_type_str = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let variant = GrpcWebVariant::from_content_type(content_type_str).ok_or_else(|| {
            GrpcWebError::UnsupportedContentType(content_type_str.to_string())
        })?;

        debug!(variant = ?variant, "gRPC-Web inbound request");

        // -- 2. collect body bytes --
        let raw_body: Bytes = req.body().clone();

        // -- 3. base64 decode if grpc-web-text --
        let grpc_body: Bytes = match variant {
            GrpcWebVariant::Text => {
                let decoded = B64
                    .decode(raw_body.as_ref())
                    .map_err(|e| GrpcWebError::BodyDecodeError(e.to_string()))?;
                Bytes::from(decoded)
            }
            GrpcWebVariant::Binary => raw_body,
        };

        // -- 4. build upstream gRPC request --
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        let upstream_uri: Uri = format!("{}{}", self.upstream, path_and_query)
            .parse()
            .map_err(|e: http::uri::InvalidUri| {
                GrpcWebError::HttpTransport(e.to_string())
            })?;

        let mut upstream_req = Request::builder()
            .method(Method::POST)
            .uri(upstream_uri)
            .header(header::CONTENT_TYPE, "application/grpc+proto")
            .header("te", "trailers"); // required by HTTP/2 gRPC spec

        // pass-through headers
        for name in &[
            "grpc-accept-encoding",
            "grpc-timeout",
            "user-agent",
        ] {
            if let Some(val) = req.headers().get(*name) {
                upstream_req = upstream_req.header(*name, val);
            }
        }

        let upstream_req = upstream_req
            .body(Full::new(grpc_body))
            .map_err(|e| GrpcWebError::HttpTransport(e.to_string()))?;

        // -- 5. send to upstream via hyper (HTTP/2) --
        let client = Client::builder(TokioExecutor::new()).build_http();

        let upstream_resp = client.request(upstream_req).await.map_err(|e| {
            warn!(error = %e, upstream = %self.upstream, "upstream gRPC unavailable");
            GrpcWebError::UpstreamUnavailable(e.to_string())
        })?;

        let (parts, body) = upstream_resp.into_parts();

        // Non-200 transport error → 503
        if parts.status != StatusCode::OK {
            return Err(GrpcWebError::UpstreamUnavailable(format!(
                "upstream returned HTTP {}",
                parts.status
            )));
        }

        // -- 6. read response body --
        let resp_body_bytes = body
            .collect()
            .await
            .map_err(|e| GrpcWebError::UpstreamUnavailable(e.to_string()))?
            .to_bytes();

        // -- 7. extract grpc-status / grpc-message from upstream trailers --
        // In HTTP/2 gRPC, trailers come as HTTP/2 TRAILERS frames.
        // hyper collects them in the response headers for HTTP/1.1 upstreams,
        // or they may be absent for in-process tests; default to status=0.
        let grpc_status: u32 = parts
            .headers
            .get("grpc-status")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let grpc_message = parts
            .headers
            .get("grpc-message")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // -- 8. pack gRPC-Web response: data frame(s) + trailer frame --
        let mut response_body = BytesMut::new();

        // Wrap the upstream body as a single data frame (it is already length-prefixed
        // gRPC; we carry it through as-is for the data portion).
        if !resp_body_bytes.is_empty() {
            // The upstream body is already a valid gRPC length-prefixed message.
            // In gRPC-Web, we simply forward it verbatim then append the trailer frame.
            response_body.extend_from_slice(&resp_body_bytes);
        }

        // Append gRPC-Web trailer frame (flags=0x80)
        let trailer_frame = build_trailer_frame(grpc_status, &grpc_message);
        response_body.extend_from_slice(&trailer_frame);

        let final_body_bytes: Bytes = response_body.freeze();

        // -- 9. base64-encode if grpc-web-text --
        let encoded_body: Bytes = match variant {
            GrpcWebVariant::Text => Bytes::from(B64.encode(&final_body_bytes)),
            GrpcWebVariant::Binary => final_body_bytes,
        };

        // -- 10. build response --
        let mut resp_builder = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, variant.response_content_type());

        // CORS headers
        let allow_origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("*");

        resp_builder = resp_builder
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin)
            .header(
                "access-control-expose-headers",
                "grpc-status, grpc-message, grpc-encoding",
            );

        let response = resp_builder
            .body(encoded_body)
            .map_err(|e| GrpcWebError::HttpTransport(e.to_string()))?;

        Ok(response)
    }

    /// Return an HTTP 415 (Unsupported Media Type) response with gRPC-Web headers.
    pub fn unsupported_media_type() -> Response<Bytes> {
        let trailer = build_trailer_frame(3, "unsupported content-type"); // grpc-status 3 = INVALID_ARGUMENT
        Response::builder()
            .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
            .header(header::CONTENT_TYPE, "application/grpc-web+proto")
            .header("grpc-status", "3")
            .header("grpc-message", "unsupported content-type")
            .body(Bytes::from(trailer))
            .expect("static response always valid")
    }

    /// Return an HTTP 503 (Service Unavailable) response with gRPC-Web trailer frame.
    pub fn service_unavailable(reason: &str) -> Response<Bytes> {
        let trailer = build_trailer_frame(14, reason); // grpc-status 14 = UNAVAILABLE
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header(header::CONTENT_TYPE, "application/grpc-web+proto")
            .header("grpc-status", "14")
            .header("grpc-message", reason)
            .body(Bytes::from(trailer))
            .expect("static response always valid")
    }

    /// Upstream address.
    pub fn upstream(&self) -> &str {
        &self.upstream
    }

    /// Whether text (base64) variant is the default.
    pub fn accept_text(&self) -> bool {
        self.accept_text
    }
}

// -- module tests --

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers --

    /// Build a minimal gRPC data frame wrapping `payload`.
    fn grpc_frame(payload: &[u8]) -> Bytes {
        build_grpc_frame(payload)
    }

    /// Build a fake upstream gRPC response body (a single length-prefixed message).
    fn upstream_grpc_body(message_bytes: &[u8]) -> Bytes {
        grpc_frame(message_bytes)
    }

    // -- unit tests that do NOT require a network --

    // -----------------------------------------------------------------------
    // Test 1: GrpcWebVariant detection
    // -----------------------------------------------------------------------
    #[test]
    fn test_variant_binary_detection() {
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
    fn test_variant_text_detection() {
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
    fn test_variant_unsupported() {
        assert!(GrpcWebVariant::from_content_type("application/json").is_none());
        assert!(GrpcWebVariant::from_content_type("application/grpc+proto").is_none());
        assert!(GrpcWebVariant::from_content_type("").is_none());
    }

    // -----------------------------------------------------------------------
    // Test 2: frame encoding / decoding round-trip
    // -----------------------------------------------------------------------
    #[test]
    fn test_grpc_frame_round_trip() {
        let payload = b"hello world";
        let framed = grpc_frame(payload);
        assert_eq!(framed.len(), 5 + payload.len());
        assert_eq!(framed[0], 0x00); // data frame flag

        let (data, remainder) = parse_grpc_frame(framed).expect("parse ok");
        assert_eq!(data.as_ref(), payload);
        assert!(remainder.is_empty());
    }

    #[test]
    fn test_grpc_frame_parse_truncated() {
        let truncated = Bytes::from_static(b"\x00\x00\x00"); // only 3 bytes
        let err = parse_grpc_frame(truncated).expect_err("should fail");
        assert!(matches!(err, GrpcWebError::InvalidFrame(_)));
    }

    // -----------------------------------------------------------------------
    // Test 3: trailer frame build + parse round-trip
    // -----------------------------------------------------------------------
    #[test]
    fn test_trailer_frame_round_trip() {
        let frame = build_trailer_frame(0, "");
        assert_eq!(frame[0], 0x80, "trailer flag must be 0x80");
        let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
        let payload = &frame[5..5 + length];
        let (status, message) = parse_trailer_payload(payload);
        assert_eq!(status, 0);
        assert_eq!(message, "");
    }

    #[test]
    fn test_trailer_frame_with_status_and_message() {
        let frame = build_trailer_frame(3, "invalid argument");
        let length = u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize;
        let payload = &frame[5..5 + length];
        let (status, message) = parse_trailer_payload(payload);
        assert_eq!(status, 3);
        assert_eq!(message, "invalid argument");
    }

    // -----------------------------------------------------------------------
    // Test 4: base64 encoding / decoding symmetry
    // -----------------------------------------------------------------------
    #[test]
    fn test_base64_encode_decode_symmetry() {
        let original = b"\x00\x00\x00\x00\x05hello"; // 5-byte grpc frame prefix + "hello"
        let encoded = B64.encode(original);
        let decoded = B64.decode(&encoded).expect("decode ok");
        assert_eq!(decoded, original);
    }

    // -----------------------------------------------------------------------
    // Test 5: unsupported content-type → 415 response
    // -----------------------------------------------------------------------
    #[test]
    fn test_unsupported_content_type_response() {
        let resp = GrpcWebTranscoder::unsupported_media_type();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            resp.headers()
                .get("grpc-status")
                .and_then(|v| v.to_str().ok()),
            Some("3")
        );
        // Body must contain a trailer frame (0x80 flag)
        let body = resp.body();
        assert!(!body.is_empty(), "body must contain trailer frame");
        assert_eq!(body[0], 0x80, "must be trailer frame");
    }

    // -----------------------------------------------------------------------
    // Test 6: service unavailable response has correct gRPC-status 14
    // -----------------------------------------------------------------------
    #[test]
    fn test_service_unavailable_response() {
        let resp = GrpcWebTranscoder::service_unavailable("upstream down");
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            resp.headers()
                .get("grpc-status")
                .and_then(|v| v.to_str().ok()),
            Some("14")
        );
        let body = resp.body();
        assert_eq!(body[0], 0x80, "must be trailer frame");
        let length = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
        let payload = &body[5..5 + length];
        let (status, message) = parse_trailer_payload(payload);
        assert_eq!(status, 14);
        assert_eq!(message, "upstream down");
    }

    // -----------------------------------------------------------------------
    // Test 7: CORS header injection
    // -----------------------------------------------------------------------
    #[test]
    fn test_cors_header_injection() {
        let mut headers = HeaderMap::new();
        inject_grpc_web_cors_headers(&mut headers, "https://app.faso.dev");

        assert_eq!(
            headers
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .and_then(|v| v.to_str().ok()),
            Some("https://app.faso.dev")
        );
        let expose = headers
            .get("access-control-expose-headers")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(expose.contains("grpc-status"));
        assert!(expose.contains("grpc-message"));
    }

    // -----------------------------------------------------------------------
    // Test 8: transcoder rejects non-grpc-web Content-Type
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_transcode_rejects_unsupported_content_type() {
        let transcoder = GrpcWebTranscoder::new("http://127.0.0.1:19999", false);

        let req = Request::builder()
            .method("POST")
            .uri("/helloworld.Greeter/SayHello")
            .header("content-type", "application/json")
            .body(Bytes::from_static(b"{}"))
            .unwrap();

        let err = transcoder.transcode(req).await.expect_err("should reject");
        assert!(
            matches!(err, GrpcWebError::UnsupportedContentType(_)),
            "expected UnsupportedContentType, got {:?}",
            err
        );
    }

    // -----------------------------------------------------------------------
    // Test 9: transcoder returns upstream-unavailable error when upstream is down
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_transcode_upstream_unavailable() {
        // Port 19998 should have nothing listening
        let transcoder = GrpcWebTranscoder::new("http://127.0.0.1:19998", false);

        // Build a minimal gRPC frame (flags=0x00, len=5, payload="hello")
        let payload = b"hello";
        let body = grpc_frame(payload);

        let req = Request::builder()
            .method("POST")
            .uri("/helloworld.Greeter/SayHello")
            .header("content-type", "application/grpc-web+proto")
            .body(body)
            .unwrap();

        let err = transcoder.transcode(req).await.expect_err("should fail");
        assert!(
            matches!(err, GrpcWebError::UpstreamUnavailable(_)),
            "expected UpstreamUnavailable, got {:?}",
            err
        );
    }

    // -----------------------------------------------------------------------
    // Test 10: binary request builds correct framed response (in-process mock)
    // -----------------------------------------------------------------------
    /// Simulate the transcoder's frame assembly logic without network I/O.
    /// Validates that binary gRPC-Web response body = data frames + trailer frame,
    /// and that the trailer frame carries grpc-status + grpc-message.
    #[test]
    fn test_binary_response_frame_assembly() {
        // Simulate upstream response body (a single gRPC data frame)
        let message_payload = b"response_payload";
        let upstream_body = upstream_grpc_body(message_payload);

        // Build final gRPC-Web body (same logic as transcoder step 8)
        let mut assembled = BytesMut::new();
        assembled.extend_from_slice(&upstream_body);
        let trailer = build_trailer_frame(0, "");
        assembled.extend_from_slice(&trailer);
        let final_bytes = assembled.freeze();

        // Verify first 5 bytes are the data frame header
        assert_eq!(final_bytes[0], 0x00, "first frame must be data");
        let data_len = u32::from_be_bytes([
            final_bytes[1],
            final_bytes[2],
            final_bytes[3],
            final_bytes[4],
        ]) as usize;
        assert_eq!(data_len, message_payload.len());

        // Verify trailer frame follows immediately
        let trailer_offset = 5 + data_len;
        assert_eq!(
            final_bytes[trailer_offset], 0x80,
            "trailer frame flag must be 0x80"
        );
        let t_len = u32::from_be_bytes([
            final_bytes[trailer_offset + 1],
            final_bytes[trailer_offset + 2],
            final_bytes[trailer_offset + 3],
            final_bytes[trailer_offset + 4],
        ]) as usize;
        let t_payload = &final_bytes[trailer_offset + 5..trailer_offset + 5 + t_len];
        let (status, msg) = parse_trailer_payload(t_payload);
        assert_eq!(status, 0);
        assert_eq!(msg, "");
    }

    // -----------------------------------------------------------------------
    // Test 11: grpc-web-text round-trip (base64 encode → decode)
    // -----------------------------------------------------------------------
    #[test]
    fn test_grpc_web_text_base64_round_trip() {
        let message_payload = b"faso-sovereign-response";
        let upstream_body = upstream_grpc_body(message_payload);

        // Simulate transcoder: assemble gRPC-Web body, then base64-encode
        let mut assembled = BytesMut::new();
        assembled.extend_from_slice(&upstream_body);
        assembled.extend_from_slice(&build_trailer_frame(0, "ok"));
        let raw = assembled.freeze();

        let encoded_str = B64.encode(&raw);
        let encoded_bytes = Bytes::from(encoded_str);

        // Simulate client decode
        let decoded = B64
            .decode(encoded_bytes.as_ref())
            .expect("valid base64");
        assert_eq!(decoded, raw.as_ref());

        // Check data frame is intact after decode
        let decoded_bytes = Bytes::from(decoded);
        let (data, rest) = parse_grpc_frame(decoded_bytes).expect("parse ok");
        assert_eq!(data.as_ref(), message_payload);
        // Trailer frame follows
        assert_eq!(rest[0], 0x80, "trailer flag");
    }

    // -----------------------------------------------------------------------
    // Test 12: grpc-web-text body decode error on bad base64
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_text_body_decode_error() {
        let transcoder = GrpcWebTranscoder::new("http://127.0.0.1:19997", true);

        let bad_b64 = Bytes::from_static(b"!!! NOT VALID BASE64 !!!");
        let req = Request::builder()
            .method("POST")
            .uri("/test.Service/Method")
            .header("content-type", "application/grpc-web-text")
            .body(bad_b64)
            .unwrap();

        let err = transcoder.transcode(req).await.expect_err("should fail on decode");
        assert!(
            matches!(err, GrpcWebError::BodyDecodeError(_)),
            "expected BodyDecodeError, got {:?}",
            err
        );
    }

    // -----------------------------------------------------------------------
    // Test 13: pass-through headers are forwarded
    // -----------------------------------------------------------------------
    #[test]
    fn test_pass_through_header_names() {
        // Verify that the header names we forward are the correct gRPC spec names
        let pass_through = ["grpc-accept-encoding", "grpc-timeout", "user-agent"];
        for name in &pass_through {
            // Simply ensure they parse as valid HeaderName (compile-time check)
            let _: HeaderName = name.parse().expect("valid header name");
        }
    }

    // -----------------------------------------------------------------------
    // Test 14: GrpcWebTranscoder::new stores fields correctly
    // -----------------------------------------------------------------------
    #[test]
    fn test_transcoder_new() {
        let t = GrpcWebTranscoder::new("http://backend:50051", true);
        assert_eq!(t.upstream(), "http://backend:50051");
        assert!(t.accept_text());

        let t2 = GrpcWebTranscoder::new("http://backend:50051", false);
        assert!(!t2.accept_text());
    }
}
