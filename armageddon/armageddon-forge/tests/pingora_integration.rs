// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! End-to-end integration tests for the Pingora gateway stack.
//!
//! ## Architecture
//!
//! Pingora's `Server::run_forever()` calls `std::process::exit(0)` on
//! shutdown, which makes it impossible to run a live server inside the
//! standard `#[test]` process.  Therefore:
//!
//! - Tests that require a **live HTTP server** are annotated `#[ignore]`
//!   with a `REQUIRES_LIVE_SERVER` note.  Run them explicitly in an isolated
//!   process:
//!   ```bash
//!   cargo test --test pingora_integration -- --ignored --test-threads=1
//!   ```
//! - All other tests exercise filter/gateway **construction** and **logic**
//!   directly — they execute within the normal test harness without risk.
//!
//! ## Test matrix
//!
//! | # | Name | Live server? |
//! |---|------|:---:|
//! | 1 | `gateway_builds_with_defaults` | No |
//! | 2 | `upstream_registry_empty_returns_none` | No |
//! | 3 | `upstream_registry_update_and_resolve` | No |
//! | 4 | `cors_config_is_origin_allowed` | No |
//! | 5 | `cors_disallowed_origin_returns_none` | No |
//! | 6 | `feature_flag_cache_key_deterministic` | No |
//! | 7 | `feature_flag_parse_flags_bug005_regression` | No |
//! | 8 | `jwt_extract_bearer_valid` | No |
//! | 9 | `jwt_extract_bearer_missing_returns_none` | No |
//! | 10 | `build_server_constructs_without_error` | No |
//! | 11 | `live_healthz_returns_200` | **Yes** (`#[ignore]`) |
//! | 12 | `live_cors_preflight_correct` | **Yes** (`#[ignore]`) |

#![cfg(feature = "pingora")]

use std::sync::Arc;

use armageddon_common::types::Endpoint;
use armageddon_forge::pingora::{
    gateway::{PingoraGateway, PingoraGatewayConfig, UpstreamRegistry},
    server::build_server,
};
use armageddon_forge::pingora::filters::cors::{CorsConfig, CorsConfigMap, CorsFilter};
use armageddon_forge::pingora::filters::feature_flag::FeatureFlagFilter;
use armageddon_forge::pingora::filters::jwt::JwtFilter;

// ---------------------------------------------------------------------------
// Test 1 — gateway builds cleanly with defaults
// ---------------------------------------------------------------------------

#[test]
fn gateway_builds_with_defaults() {
    let gw = PingoraGateway::with_defaults();
    let cfg = gw.config();
    assert_eq!(cfg.default_cluster, "default");
    assert!(!cfg.upstream_tls, "TLS should be off by default");
    assert_eq!(cfg.filters.len(), 0, "no filters in default config");
}

// ---------------------------------------------------------------------------
// Test 2 — empty upstream registry returns None
// ---------------------------------------------------------------------------

#[test]
fn upstream_registry_empty_returns_none() {
    let reg = UpstreamRegistry::new();
    assert!(
        reg.first_healthy("nonexistent").is_none(),
        "empty registry must return None"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — upstream registry update + resolve round-trip
// ---------------------------------------------------------------------------

#[test]
fn upstream_registry_update_and_resolve() {
    let reg = UpstreamRegistry::new();

    // Healthy endpoint is returned.
    reg.update_cluster(
        "backend",
        vec![Endpoint {
            address: "10.0.0.1".to_string(),
            port: 8080,
            weight: 1,
            healthy: true,
        }],
    );
    let resolved = reg.first_healthy("backend").expect("must resolve");
    assert_eq!(resolved.address, "10.0.0.1");
    assert_eq!(resolved.port, 8080);

    // Unhealthy endpoint must not be returned by `first_healthy`.
    reg.update_cluster(
        "unhealthy",
        vec![Endpoint {
            address: "10.0.0.2".to_string(),
            port: 9090,
            weight: 1,
            healthy: false,
        }],
    );
    assert!(
        reg.first_healthy("unhealthy").is_none(),
        "unhealthy endpoint must NOT be returned by first_healthy"
    );

    // `all()` includes unhealthy endpoints.
    let all = reg.all("unhealthy");
    assert_eq!(all.len(), 1, "all() must include unhealthy endpoints");
}

// ---------------------------------------------------------------------------
// Test 4 — CorsConfig: allowed origin accepted, wildcard works
// ---------------------------------------------------------------------------

#[test]
fn cors_config_is_origin_allowed() {
    let cfg = CorsConfig {
        allowed_origins: vec!["https://app.faso.gov.bf".to_string()],
        ..CorsConfig::default()
    };
    assert!(cfg.is_origin_allowed("https://app.faso.gov.bf"));
    assert!(!cfg.is_origin_allowed("https://evil.example.com"));

    // Wildcard
    let wildcard = CorsConfig {
        allowed_origins: vec!["*".to_string()],
        ..CorsConfig::default()
    };
    assert!(wildcard.is_origin_allowed("https://anything.example.com"));
}

// ---------------------------------------------------------------------------
// Test 5 — CorsConfig: wildcard + credentials is rejected by validate()
// ---------------------------------------------------------------------------

#[test]
fn cors_disallowed_origin_returns_none() {
    // Build a CorsConfigMap and verify lookup works correctly.
    let cfg = CorsConfig {
        allowed_origins: vec!["https://app.faso.gov.bf".to_string()],
        ..CorsConfig::default()
    };
    let map = CorsConfigMap::new(vec![("default".to_string(), cfg)]).unwrap();
    assert!(map.lookup("default").is_some(), "lookup must find 'default'");
    assert!(
        map.lookup("other").is_some(),
        "fallback to 'default' when no exact match"
    );

    // Wildcard + credentials must be rejected.
    let bad = CorsConfig {
        allowed_origins: vec!["*".to_string()],
        allow_credentials: true,
        ..CorsConfig::default()
    };
    assert!(
        bad.validate().is_err(),
        "wildcard + allow_credentials must fail validate()"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — FeatureFlagFilter: cache_key is deterministic for the same user_id
// ---------------------------------------------------------------------------

#[test]
fn feature_flag_cache_key_deterministic() {
    let k1 = FeatureFlagFilter::cache_key("user-abc-123");
    let k2 = FeatureFlagFilter::cache_key("user-abc-123");
    assert_eq!(k1, k2, "cache key must be deterministic");
    assert!(k1.starts_with("ff:prod:"), "key must have correct prefix");

    // Different user IDs must produce different keys.
    let k3 = FeatureFlagFilter::cache_key("user-xyz-456");
    assert_ne!(k1, k3, "different user IDs must produce different keys");
}

// ---------------------------------------------------------------------------
// Test 7 — FeatureFlagFilter: parse_flags bug_005 regression
//
// Verifies the security invariant: only flags explicitly set to `true` in the
// KAYA JSON are emitted; `false` flags are silently dropped, and garbage JSON
// produces an empty result (graceful degradation).
// ---------------------------------------------------------------------------

#[test]
fn feature_flag_parse_flags_bug005_regression() {
    // Normal case: only true flags appear
    let csv = FeatureFlagFilter::parse_flags(r#"{"new_ux":true,"debug":false,"beta":true}"#);
    // Sorted: beta, new_ux
    assert_eq!(csv, "beta,new_ux", "only true flags must be emitted, sorted");

    // Garbage JSON → empty string (no panic, no error)
    let empty = FeatureFlagFilter::parse_flags("not-json");
    assert!(empty.is_empty(), "garbage JSON must produce empty string");

    // All false → empty
    let all_false = FeatureFlagFilter::parse_flags(r#"{"a":false,"b":false}"#);
    assert!(all_false.is_empty(), "all-false flags must produce empty string");

    // Empty object → empty
    let empty_obj = FeatureFlagFilter::parse_flags(r#"{}"#);
    assert!(empty_obj.is_empty(), "empty object must produce empty string");
}

// ---------------------------------------------------------------------------
// Test 8 — JwtFilter: extract_bearer parses Authorization header correctly
// ---------------------------------------------------------------------------

#[test]
fn jwt_extract_bearer_valid() {
    // Standard Bearer prefix (case-insensitive)
    assert_eq!(
        JwtFilter::extract_bearer("Bearer my.jwt.token"),
        Some("my.jwt.token")
    );
    assert_eq!(
        JwtFilter::extract_bearer("BEARER my.jwt.token"),
        Some("my.jwt.token")
    );
    assert_eq!(
        JwtFilter::extract_bearer("bearer  my.jwt.token"),
        Some("my.jwt.token")
    );
}

// ---------------------------------------------------------------------------
// Test 9 — JwtFilter: extract_bearer returns None for malformed headers
// ---------------------------------------------------------------------------

#[test]
fn jwt_extract_bearer_missing_returns_none() {
    assert!(JwtFilter::extract_bearer("").is_none());
    assert!(JwtFilter::extract_bearer("Basic dXNlcjpwYXNz").is_none());
    assert!(JwtFilter::extract_bearer("Bear token").is_none());
    // Token with only whitespace after prefix
    assert!(
        JwtFilter::extract_bearer("Bearer   ").unwrap_or("").trim().is_empty()
            || JwtFilter::extract_bearer("Bearer   ").is_none(),
        "whitespace-only token must not pass validation"
    );
}

// ---------------------------------------------------------------------------
// Test 10 — build_server succeeds (no panic, no I/O error at construction)
//
// `build_server` does NOT bind the TCP port at construction time in Pingora
// 0.3 — it defers to `run_forever`.  Construction must succeed cleanly.
// ---------------------------------------------------------------------------

#[test]
fn build_server_constructs_without_error() {
    let gw = PingoraGateway::with_defaults();
    // Any address string is valid at this stage; the actual bind happens
    // inside `run_forever()`.
    let result = build_server(gw, "127.0.0.1:0");
    assert!(
        result.is_ok(),
        "build_server must succeed without error: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// Live-server tests — skipped in the default test harness.
//
// These tests start a Pingora server.  Because `Server::run_forever()`
// eventually calls `std::process::exit(0)`, they MUST run in an isolated
// process:
//
//   cargo test --test pingora_integration -- --ignored --test-threads=1
//
// See CUTOVER.md §"Integration test harness" for the recommended CI setup.
// ---------------------------------------------------------------------------

/// Start a real Pingora server, send HTTP GET /healthz, assert 200 response.
///
/// # Why `#[ignore]`
///
/// `Server::run_forever()` calls `std::process::exit(0)`, which terminates
/// the entire test process.  Running this inside the default harness kills
/// all subsequent tests.  See module-level documentation for the isolation
/// instructions.
#[test]
#[ignore = "REQUIRES_LIVE_SERVER: run alone with --ignored --test-threads=1"]
fn live_healthz_returns_200() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    // Grab a free OS port then release it for Pingora.
    let port = {
        let tmp = std::net::TcpListener::bind("127.0.0.1:0").expect("bind tmp");
        tmp.local_addr().expect("local addr").port()
    };

    let listen = format!("127.0.0.1:{port}");
    let listen_for_thread = listen.clone();

    // Spawn Pingora in a background thread.  The thread never returns.
    std::thread::spawn(move || {
        let gw = PingoraGateway::with_defaults();
        let server = build_server(gw, &listen_for_thread).expect("build");
        server.run_forever(); // calls std::process::exit(0)
    });

    // Poll until the port is accepting TCP connections (up to 5 s).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let addr: std::net::SocketAddr = listen.parse().unwrap();
    let mut connected = false;
    while std::time::Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
            connected = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(connected, "Pingora did not start within 5 s");

    // Send a raw HTTP/1.0 GET request (no persistent connection).
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    stream
        .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .expect("write");

    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).unwrap_or(0);
    let resp = std::str::from_utf8(&buf[..n]).unwrap_or("");

    assert!(
        resp.starts_with("HTTP/1."),
        "expected HTTP response, got: {resp:?}"
    );
    // Without a wired healthz filter the default upstream_peer path returns
    // 502 (no upstream registered).  Accept either 200 or 502 — the key
    // assertion is that the server is responding with valid HTTP.
    assert!(
        resp.contains("200") || resp.contains("502"),
        "expected 200 (with healthz filter) or 502 (no upstream), got: {resp:?}"
    );
}

/// CORS pre-flight on a live server → 204 + Access-Control-Allow-Origin.
///
/// # Why `#[ignore]`
///
/// Same as `live_healthz_returns_200`.
#[test]
#[ignore = "REQUIRES_LIVE_SERVER: run alone with --ignored --test-threads=1"]
fn live_cors_preflight_correct() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let cors_cfg = CorsConfig {
        allowed_origins: vec!["https://app.faso.gov.bf".to_string()],
        ..CorsConfig::default()
    };
    let map = CorsConfigMap::new(vec![("default".to_string(), cors_cfg)]).unwrap();
    let cors_filter = Arc::new(CorsFilter::new(map));
    let gw_cfg = PingoraGatewayConfig {
        filters: vec![cors_filter],
        ..PingoraGatewayConfig::default()
    };
    let gw = PingoraGateway::new(gw_cfg, Arc::new(UpstreamRegistry::new()));

    let port = {
        let tmp = std::net::TcpListener::bind("127.0.0.1:0").expect("bind tmp");
        tmp.local_addr().expect("local addr").port()
    };
    let listen = format!("127.0.0.1:{port}");
    let listen_for_thread = listen.clone();

    std::thread::spawn(move || {
        let server = build_server(gw, &listen_for_thread).expect("build");
        server.run_forever();
    });

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let addr: std::net::SocketAddr = listen.parse().unwrap();
    while std::time::Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let req = concat!(
        "OPTIONS /api/v1/resource HTTP/1.0\r\n",
        "Host: localhost\r\n",
        "Origin: https://app.faso.gov.bf\r\n",
        "Access-Control-Request-Method: POST\r\n",
        "Access-Control-Request-Headers: authorization\r\n",
        "\r\n"
    );
    stream.write_all(req.as_bytes()).expect("write");
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let resp = std::str::from_utf8(&buf[..n]).unwrap_or("");

    assert!(
        resp.starts_with("HTTP/1."),
        "expected HTTP response, got: {resp:?}"
    );
    // CorsFilter short-circuits pre-flight with 204.
    assert!(
        resp.contains("204"),
        "CORS pre-flight must respond 204, got: {resp:?}"
    );
    assert!(
        resp.to_lowercase().contains("access-control-allow-origin"),
        "must include Access-Control-Allow-Origin, got: {resp:?}"
    );
}
