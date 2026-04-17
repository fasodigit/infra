// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Codec: bidirectional conversion between `http::Request` / `http::Response`
//! (as used by h3) and ARMAGEDDON's internal
//! [`armageddon_common::types::HttpRequest`] /
//! [`armageddon_common::types::HttpResponse`].
//!
//! The codec is kept deliberately lightweight — it does **not** buffer or
//! compress bodies; that responsibility belongs to higher-level middleware.

use std::collections::HashMap;

use armageddon_common::types::{HttpRequest, HttpResponse, HttpVersion};

// ---------------------------------------------------------------------------
// http::Request<()>  →  HttpRequest
// ---------------------------------------------------------------------------

/// Convert an [`http::Request<()>`] (headers only, as returned by h3's accept
/// loop) plus the already-collected body bytes into an [`HttpRequest`].
///
/// # Notes
/// * Multi-value headers with the same name are joined with `,` (RFC 9110 §5.3).
/// * `http::Version::HTTP_3` is mapped to [`HttpVersion::Http3`].
pub fn http_request_to_internal(
    req: http::Request<()>,
    body: Option<Vec<u8>>,
) -> HttpRequest {
    let method = req.method().to_string();
    let uri = req.uri().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let version = map_version(req.version());

    // Merge repeated header names.
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in req.headers() {
        let key = name.to_string();
        let val = value.to_str().unwrap_or("").to_string();
        headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(',');
                existing.push_str(&val);
            })
            .or_insert(val);
    }

    HttpRequest {
        method,
        uri,
        path,
        query,
        headers,
        body,
        version,
    }
}

// ---------------------------------------------------------------------------
// HttpRequest  →  http::Request<()>   (round-trip helper, used in tests)
// ---------------------------------------------------------------------------

/// Convert an [`HttpRequest`] back into an [`http::Request<()>`].
///
/// Header values that are already comma-joined are stored as a single header
/// field (valid per RFC 9110 §5.3 for most headers).
///
/// Returns `None` if the method or URI is unparseable.
pub fn internal_to_http_request(req: &HttpRequest) -> Option<http::Request<()>> {
    let method: http::Method = req.method.parse().ok()?;
    let uri: http::Uri = req.uri.parse().ok()?;

    let mut builder = http::Request::builder()
        .method(method)
        .uri(uri)
        .version(unmap_version(req.version));

    for (k, v) in &req.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }

    builder.body(()).ok()
}

// ---------------------------------------------------------------------------
// HttpResponse helpers
// ---------------------------------------------------------------------------

/// Convert an [`http::Response<()>`] (headers-only, as sent via h3) into an
/// [`HttpResponse`] with an optional body.
pub fn http_response_to_internal(
    resp: http::Response<()>,
    body: Option<Vec<u8>>,
) -> HttpResponse {
    let status = resp.status().as_u16();
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in resp.headers() {
        headers.insert(name.to_string(), value.to_str().unwrap_or("").to_string());
    }
    HttpResponse { status, headers, body }
}

// ---------------------------------------------------------------------------
// Version mapping
// ---------------------------------------------------------------------------

fn map_version(v: http::Version) -> HttpVersion {
    match v {
        http::Version::HTTP_10 => HttpVersion::Http10,
        http::Version::HTTP_11 => HttpVersion::Http11,
        http::Version::HTTP_2 => HttpVersion::Http2,
        http::Version::HTTP_3 => HttpVersion::Http3,
        // Fallback: treat unknown versions as HTTP/1.1.
        _ => HttpVersion::Http11,
    }
}

fn unmap_version(v: HttpVersion) -> http::Version {
    match v {
        HttpVersion::Http10 => http::Version::HTTP_10,
        HttpVersion::Http11 => http::Version::HTTP_11,
        HttpVersion::Http2 => http::Version::HTTP_2,
        HttpVersion::Http3 => http::Version::HTTP_3,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Test 5: codec round-trip http::Request → HttpRequest (method, headers,
    //         body bytes, path, query).
    // -----------------------------------------------------------------------
    #[test]
    fn test_codec_roundtrip_get() {
        let http_req = http::Request::builder()
            .method(http::Method::GET)
            .uri("https://api.faso.bf/v1/chickens?farm=1")
            .version(http::Version::HTTP_3)
            .header("x-request-id", "abc-123")
            .header("accept", "application/json")
            .body(())
            .unwrap();

        let internal = http_request_to_internal(http_req, None);

        assert_eq!(internal.method, "GET");
        assert_eq!(internal.path, "/v1/chickens");
        assert_eq!(internal.query.as_deref(), Some("farm=1"));
        assert_eq!(internal.version, HttpVersion::Http3);
        assert_eq!(internal.headers.get("x-request-id").map(|s| s.as_str()), Some("abc-123"));
        assert_eq!(internal.headers.get("accept").map(|s| s.as_str()), Some("application/json"));
        assert!(internal.body.is_none());
    }

    #[test]
    fn test_codec_roundtrip_post_with_body() {
        let body_bytes = b"{\"name\":\"faso\"}".to_vec();

        let http_req = http::Request::builder()
            .method(http::Method::POST)
            .uri("https://api.faso.bf/v1/chickens")
            .version(http::Version::HTTP_3)
            .header("content-type", "application/json")
            .header("content-length", "15")
            .body(())
            .unwrap();

        let internal = http_request_to_internal(http_req, Some(body_bytes.clone()));

        assert_eq!(internal.method, "POST");
        assert_eq!(internal.path, "/v1/chickens");
        assert!(internal.query.is_none());
        assert_eq!(internal.body.as_deref(), Some(body_bytes.as_slice()));
        assert_eq!(
            internal.headers.get("content-type").map(|s| s.as_str()),
            Some("application/json")
        );
    }

    #[test]
    fn test_codec_roundtrip_delete_empty() {
        let http_req = http::Request::builder()
            .method(http::Method::DELETE)
            .uri("https://api.faso.bf/v1/chickens/42")
            .version(http::Version::HTTP_3)
            .body(())
            .unwrap();

        let internal = http_request_to_internal(http_req, None);
        assert_eq!(internal.method, "DELETE");
        assert_eq!(internal.path, "/v1/chickens/42");
        assert!(internal.body.is_none());
        assert!(internal.headers.is_empty());
    }

    #[test]
    fn test_codec_version_mapping() {
        let cases = [
            (http::Version::HTTP_10, HttpVersion::Http10),
            (http::Version::HTTP_11, HttpVersion::Http11),
            (http::Version::HTTP_2, HttpVersion::Http2),
            (http::Version::HTTP_3, HttpVersion::Http3),
        ];
        for (http_v, expected) in cases {
            assert_eq!(map_version(http_v), expected);
            assert_eq!(unmap_version(expected), http_v);
        }
    }

    #[test]
    fn test_codec_internal_to_http_request_roundtrip() {
        let original = HttpRequest {
            method: "PUT".to_string(),
            uri: "https://api.faso.bf/v1/farms/7".to_string(),
            path: "/v1/farms/7".to_string(),
            query: None,
            headers: {
                let mut m = HashMap::new();
                m.insert("authorization".to_string(), "Bearer tok".to_string());
                m
            },
            body: None,
            version: HttpVersion::Http3,
        };

        let http_req = internal_to_http_request(&original).expect("conversion");
        assert_eq!(http_req.method(), http::Method::PUT);
        assert_eq!(http_req.uri().path(), "/v1/farms/7");
        assert_eq!(http_req.version(), http::Version::HTTP_3);
        assert_eq!(
            http_req.headers().get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer tok")
        );
    }

    #[test]
    fn test_codec_multi_value_headers_joined() {
        let http_req = http::Request::builder()
            .method(http::Method::GET)
            .uri("https://api.faso.bf/")
            .version(http::Version::HTTP_3)
            .header("x-tags", "a")
            .header("x-tags", "b")
            .body(())
            .unwrap();

        let internal = http_request_to_internal(http_req, None);
        let tags = internal.headers.get("x-tags").expect("x-tags header");
        // RFC 9110 §5.3 join: must contain both values.
        assert!(tags.contains('a'), "missing 'a' in {tags}");
        assert!(tags.contains('b'), "missing 'b' in {tags}");
    }
}
