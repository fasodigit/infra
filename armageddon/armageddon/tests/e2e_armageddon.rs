// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! End-to-end integration tests for ARMAGEDDON Vague 1 wire-up.
//!
//! These tests start lightweight in-process servers (no real SPIRE / xDS /
//! KAYA required) and verify that:
//!
//! 1. The config loader accepts the `armageddon-full.yaml` example file.
//! 2. The admin server starts and responds to `GET /admin/clusters`.
//! 3. The response cache (in-memory) stores and retrieves an entry correctly.
//!
//! Note: tests that require the QUIC listener are skipped when TLS files are
//! absent (CI environments without generated certs).

use std::time::Duration;

// ---------------------------------------------------------------------------
// Test 1: config loader accepts armageddon-full.yaml
// ---------------------------------------------------------------------------
#[test]
fn config_loader_accepts_full_yaml() {
    // The path is relative to the workspace root; adjust if running from
    // a different working directory.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/armageddon-full.yaml"
    );

    let loader = armageddon_config::ConfigLoader::from_file(path);
    assert!(
        loader.is_ok(),
        "armageddon-full.yaml should parse without errors: {:?}",
        loader.err()
    );

    let config = loader.unwrap().get();

    // Verify Vague 1 sections parsed correctly
    assert!(
        config.gateway.quic.is_some(),
        "quic section should be present in armageddon-full.yaml"
    );
    assert!(
        config.gateway.cache.is_some(),
        "cache section should be present in armageddon-full.yaml"
    );
    assert!(
        config.gateway.admin.is_some(),
        "admin section should be present in armageddon-full.yaml"
    );
    assert_eq!(
        config.gateway.lb.algorithm,
        "round_robin",
        "lb.algorithm should default to round_robin"
    );
    assert_eq!(
        config.gateway.retry.max_retries,
        2,
        "retry.max_retries should be 2"
    );
    assert!(
        config.gateway.websocket_enabled,
        "websocket_enabled should be true"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Admin server starts and /admin/clusters returns JSON
// ---------------------------------------------------------------------------
#[tokio::test]
async fn admin_server_clusters_endpoint() {
    use armageddon_admin::{AdminConfig, AdminServer, AdminState};
    use armageddon_common::types::Cluster;
    use armageddon_config::GatewayConfig;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::sync::broadcast;

    // Find a free port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind free port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Build minimal gateway config with one dummy cluster
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/armageddon-full.yaml"
    );
    let loader = armageddon_config::ConfigLoader::from_file(path)
        .expect("config load for admin test");
    let config = loader.get();
    let gateway_cfg = config.gateway.clone();

    let admin_config = AdminConfig {
        bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        admin_token: None,
    };
    let state = AdminState::new(gateway_cfg, path.to_string());
    let server = AdminServer::new(admin_config, state);

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

    let handle = tokio::spawn(async move {
        let _ = server.run(shutdown_rx).await;
    });

    // Give the server a moment to bind
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Fetch /admin/clusters
    let url = format!("http://127.0.0.1:{}/admin/clusters", port);
    let resp = reqwest_get_text(&url).await;

    // Signal shutdown
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;

    assert!(
        resp.is_ok(),
        "GET /admin/clusters should return a valid response: {:?}",
        resp.err()
    );
    let body = resp.unwrap();
    // The body should be valid JSON.
    // The endpoint may return an array or {"clusters": [...]} depending on the route impl.
    let parsed: serde_json::Value = serde_json::from_str(&body)
        .expect("admin /admin/clusters should return valid JSON");

    // Accept both shapes: bare array or {"clusters": [...]}
    let has_clusters = parsed.is_array()
        || parsed
            .as_object()
            .map(|o| o.contains_key("clusters"))
            .unwrap_or(false);
    assert!(
        has_clusters,
        "admin /admin/clusters should return cluster data, got: {}",
        parsed
    );
}

// ---------------------------------------------------------------------------
// Test 3: Response cache (in-memory) stores and retrieves an entry
// ---------------------------------------------------------------------------
#[tokio::test]
async fn response_cache_get_put_roundtrip() {
    use armageddon_cache::{CachePolicy, InMemoryKv, ResponseCache};
    use armageddon_common::types::{HttpRequest, HttpResponse, HttpVersion};
    use prometheus::Registry;
    use std::collections::HashMap;
    use std::sync::Arc;

    let kv = Arc::new(InMemoryKv::new());
    let policy = CachePolicy::default();
    let registry = Registry::new();
    let cache = ResponseCache::new(kv, policy, &registry).expect("cache init");

    let req = HttpRequest {
        method: "GET".to_string(),
        uri: "/api/test".to_string(),
        path: "/api/test".to_string(),
        query: None,
        headers: HashMap::new(),
        body: None,
        version: HttpVersion::Http11,
    };

    // Cache miss on first access
    let miss = cache.get(&req).await.expect("cache get should not error");
    assert!(miss.is_none(), "first access should be a cache miss");

    // Store a response
    let mut resp_headers = HashMap::new();
    resp_headers.insert("cache-control".to_string(), "public, max-age=60".to_string());
    let resp = HttpResponse {
        status: 200,
        headers: resp_headers,
        body: Some(b"faso digitalisation response".to_vec()),
    };
    cache
        .put(&req, &resp, Duration::from_secs(60))
        .await
        .expect("cache put should not error");

    // Cache hit on second access
    let hit = cache.get(&req).await.expect("cache get should not error");
    assert!(hit.is_some(), "second access should be a cache hit");
    let cached = hit.unwrap();
    assert_eq!(cached.status, 200);
    assert_eq!(
        cached.body.as_ref(),
        b"faso digitalisation response" as &[u8]
    );
}

// ---------------------------------------------------------------------------
// Test 4: LB algorithm selection compiles and selects an endpoint
// ---------------------------------------------------------------------------
#[test]
fn lb_round_robin_selects_endpoint() {
    use armageddon_lb::{Endpoint, LoadBalancer, RoundRobin};
    use std::sync::Arc;

    let eps: Vec<Arc<Endpoint>> = vec![
        Arc::new(Endpoint::new("a", "10.0.0.1:8080", 1)),
        Arc::new(Endpoint::new("b", "10.0.0.2:8080", 1)),
    ];
    let lb = RoundRobin::new();

    let chosen = lb.select(&eps, None);
    assert!(chosen.is_some(), "RoundRobin should select an endpoint");
    let ep = chosen.unwrap();
    assert!(
        ep.address == "10.0.0.1:8080" || ep.address == "10.0.0.2:8080",
        "selected endpoint must be one of the two configured backends"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Retry budget enforces limits
// ---------------------------------------------------------------------------
#[test]
fn retry_budget_limits_concurrent_retries() {
    use armageddon_retry::RetryBudget;

    // Allow at most 2 concurrent retries via min floor
    let budget = RetryBudget::new(0.0, 2);
    assert!(budget.try_reserve(), "first reserve should succeed");
    assert!(budget.try_reserve(), "second reserve should succeed");
    assert!(!budget.try_reserve(), "third reserve should fail — budget exhausted");

    // Releasing restores a slot
    budget.release_retry();
    assert!(budget.try_reserve(), "after release, one more reserve should succeed");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Tiny HTTP client using `std::net::TcpStream` to avoid reqwest dep.
async fn reqwest_get_text(url: &str) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    // Parse url: http://host:port/path
    let without_scheme = url.strip_prefix("http://").ok_or("not http")?;
    let (host_port, path) = without_scheme
        .split_once('/')
        .map(|(h, p)| (h, format!("/{}", p)))
        .unwrap_or((without_scheme, "/".to_string()));

    let mut stream = TcpStream::connect(host_port)
        .await
        .map_err(|e| format!("connect: {}", e))?;

    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host_port
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("write: {}", e))?;

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("read: {}", e))?;

    let response = String::from_utf8_lossy(&buf).to_string();

    // Split off headers from body
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or(response);

    Ok(body)
}
