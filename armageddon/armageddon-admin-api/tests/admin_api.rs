// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Integration tests for the ARMAGEDDON admin API.

use armageddon_admin_api::{
    auth::{bearer_auth, AuthState},
    build_router, providers::{NullClusterProvider, NullConfigDumper, NullHealthProvider,
        NullRuntimeProvider, NullShadowProvider, NullStatsProvider,
        ShadowProvider, ShadowStateSnapshot},
    state::{AdminApiState, ServerInfo},
    AdminApi, AdminApiConfig,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    middleware,
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt; // oneshot

fn make_state() -> Arc<AdminApiState> {
    AdminApiState::new(
        Arc::new(NullStatsProvider),
        Arc::new(NullClusterProvider),
        Arc::new(NullConfigDumper),
        Arc::new(NullRuntimeProvider),
        Arc::new(NullHealthProvider),
        ServerInfo {
            version: "test".to_string(),
            build_sha: "abc123".to_string(),
            hostname: "host".to_string(),
            started_at: Utc::now(),
        },
        "info",
    )
}

fn make_state_with_shadow(shadow: Arc<dyn ShadowProvider>) -> Arc<AdminApiState> {
    AdminApiState::new_with_shadow(
        Arc::new(NullStatsProvider),
        Arc::new(NullClusterProvider),
        Arc::new(NullConfigDumper),
        Arc::new(NullRuntimeProvider),
        Arc::new(NullHealthProvider),
        shadow,
        ServerInfo {
            version: "test".to_string(),
            build_sha: "abc123".to_string(),
            hostname: "host".to_string(),
            started_at: Utc::now(),
        },
        "info",
    )
}

fn router_with_shadow(shadow: Arc<dyn ShadowProvider>) -> Router {
    let cfg = AdminApiConfig::default();
    build_router(
        make_state_with_shadow(shadow),
        Arc::new(AuthState::disabled()),
        &cfg,
    )
}

fn router_no_auth() -> Router {
    let cfg = AdminApiConfig::default();
    build_router(make_state(), Arc::new(AuthState::disabled()), &cfg)
}

fn router_with_token(token: &str) -> Router {
    let cfg = AdminApiConfig::default();
    build_router(
        make_state(),
        Arc::new(AuthState::with_token(token)),
        &cfg,
    )
}

// -- /stats --

#[tokio::test]
async fn get_stats_returns_prometheus_text() {
    let app = router_no_auth();
    let resp = app
        .oneshot(Request::builder().uri("/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.starts_with("text/plain"), "got {ct}");
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.starts_with("# "), "expected Prometheus comment, got {text:?}");
}

#[tokio::test]
async fn get_stats_json_format() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/stats?format=json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v.is_object());
}

// -- /clusters --

#[tokio::test]
async fn get_clusters_returns_json_object_with_array() {
    let app = router_no_auth();
    let resp = app
        .oneshot(Request::builder().uri("/clusters").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("clusters").unwrap().is_array());
}

// -- /config_dump --

#[tokio::test]
async fn get_config_dump_json_default() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/config_dump")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let _: Value = serde_json::from_slice(&body).unwrap();
}

#[tokio::test]
async fn get_config_dump_yaml_on_demand() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/config_dump?format=yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(ct, "application/yaml");
}

// -- /runtime --

#[tokio::test]
async fn get_runtime_returns_empty_object_default() {
    let app = router_no_auth();
    let resp = app
        .oneshot(Request::builder().uri("/runtime").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v.is_object());
    assert!(v.as_object().unwrap().is_empty());
}

// -- /server_info --

#[tokio::test]
async fn get_server_info_contains_version_and_uptime() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/server_info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("version").and_then(|x| x.as_str()), Some("test"));
    assert_eq!(v.get("build_sha").and_then(|x| x.as_str()), Some("abc123"));
    assert!(v.get("uptime_secs").and_then(|x| x.as_u64()).is_some());
}

// -- /listeners --

#[tokio::test]
async fn get_listeners_returns_listeners_array() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/listeners")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("listeners").unwrap().is_array());
}

// -- /health --

#[tokio::test]
async fn get_health_returns_serving() {
    let app = router_no_auth();
    let resp = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("status").and_then(|x| x.as_str()), Some("SERVING"));
}

// -- POST /logging --

#[tokio::test]
async fn post_logging_accepts_valid_level() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/logging")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"level":"debug"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("current").and_then(|x| x.as_str()), Some("debug"));
}

#[tokio::test]
async fn post_logging_rejects_invalid_level() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/logging")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"level":"VERBOSE"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// -- auth middleware --

async fn ok_handler() -> impl IntoResponse {
    "ok"
}

fn auth_test_router(auth: Arc<AuthState>) -> Router {
    Router::new().route("/protected", get(ok_handler)).layer(
        middleware::from_fn_with_state(auth, bearer_auth),
    )
}

#[tokio::test]
async fn auth_rejects_missing_header() {
    let app = auth_test_router(Arc::new(AuthState::with_token("s3cret")));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_rejects_wrong_token() {
    let app = auth_test_router(Arc::new(AuthState::with_token("s3cret")));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", "Bearer not-the-right-one")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_accepts_correct_token() {
    let app = auth_test_router(Arc::new(AuthState::with_token("s3cret")));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .header("authorization", "Bearer s3cret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_disabled_allows_anonymous() {
    let app = auth_test_router(Arc::new(AuthState::disabled()));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// -- router-level auth integration --

#[tokio::test]
async fn full_router_requires_token_when_configured() {
    let app = router_with_token("topsecret");
    let resp = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn full_router_accepts_token_when_configured() {
    let app = router_with_token("topsecret");
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("authorization", "Bearer topsecret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// -- config: non-loopback + missing token --

#[tokio::test]
async fn admin_api_refuses_public_bind_without_token() {
    // Ensure no accidental token lives in the env.
    std::env::remove_var("ARMAGEDDON_ADMIN_TOKEN_TESTCASE");
    let cfg = AdminApiConfig {
        enabled: true,
        bind_addr: "0.0.0.0:19099".to_string(),
        token_env_var: "ARMAGEDDON_ADMIN_TOKEN_TESTCASE".to_string(),
        cors_allowed_origins: vec![],
    };
    let err = AdminApi::build_with_nulls(cfg).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("ARMAGEDDON_ADMIN_TOKEN_TESTCASE"),
        "unexpected err: {msg}"
    );
}

#[tokio::test]
async fn admin_api_loopback_no_token_is_ok() {
    std::env::remove_var("ARMAGEDDON_ADMIN_TOKEN_TESTCASE2");
    let cfg = AdminApiConfig {
        enabled: true,
        bind_addr: "127.0.0.1:19100".to_string(),
        token_env_var: "ARMAGEDDON_ADMIN_TOKEN_TESTCASE2".to_string(),
        cors_allowed_origins: vec![],
    };
    let api = AdminApi::build_with_nulls(cfg).unwrap();
    assert!(api.bind_addr().ip().is_loopback());
}

// -- CORS regression: the admin API MUST NOT emit any CORS headers so that
//    a malicious page visited by an operator cannot read `/config_dump` or
//    `POST /logging` via a cross-origin `fetch()`. See security advisory
//    "armageddon-admin-api CORS wildcard exfiltration" (2026-04-19).

#[tokio::test]
async fn cross_origin_get_config_dump_has_no_cors_allow_origin_header() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/config_dump")
                .header("origin", "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // The request itself is served (it looks like any other HTTP call from
    // the server's POV), but the browser-enforced SOP check must fail —
    // i.e. no Access-Control-Allow-Origin header is returned.
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "admin API must not emit Access-Control-Allow-Origin; \
         got {:?}",
        resp.headers().get("access-control-allow-origin")
    );
    assert!(
        resp.headers().get("access-control-allow-credentials").is_none(),
        "admin API must not emit Access-Control-Allow-Credentials"
    );
    assert!(
        resp.headers().get("access-control-expose-headers").is_none(),
        "admin API must not emit Access-Control-Expose-Headers"
    );
}

// -- /admin/shadow/* --

/// A test shadow provider that tracks rate changes in an `AtomicU32`.
struct TestShadowProvider {
    rate: std::sync::atomic::AtomicU32,
    gate_enabled: std::sync::atomic::AtomicBool,
}

impl TestShadowProvider {
    fn new(initial_rate: u32) -> Arc<Self> {
        Arc::new(Self {
            rate: std::sync::atomic::AtomicU32::new(initial_rate),
            gate_enabled: std::sync::atomic::AtomicBool::new(true),
        })
    }
}

#[async_trait]
impl ShadowProvider for TestShadowProvider {
    async fn shadow_state(&self) -> ShadowStateSnapshot {
        ShadowStateSnapshot {
            sample_rate: self.rate.load(std::sync::atomic::Ordering::Relaxed),
            gate_tripped_count: 0,
            last_divergence_rate: 0.01,
            window_samples: 150,
            gate_enabled: self.gate_enabled.load(std::sync::atomic::Ordering::Relaxed),
            gate_max_divergence_rate: 0.02,
        }
    }

    async fn set_sample_rate(&self, percent: u32) -> u32 {
        let clamped = percent.min(100);
        self.rate.store(clamped, std::sync::atomic::Ordering::Relaxed);
        clamped
    }

    async fn reconfigure_gate(&self, enabled: Option<bool>, _max_divergence_rate: Option<f64>) {
        if let Some(e) = enabled {
            self.gate_enabled.store(e, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// `POST /admin/shadow/rate` sets rate and returns new value.
#[tokio::test]
async fn post_shadow_rate_sets_rate_and_returns_new_value() {
    let provider = TestShadowProvider::new(10);
    let app = router_with_shadow(provider.clone());

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/shadow/rate")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"percent": 25}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("sample_rate").and_then(Value::as_u64), Some(25));
}

/// `POST /admin/shadow/rate` with percent > 100 returns 400.
#[tokio::test]
async fn post_shadow_rate_rejects_out_of_range_value() {
    let app = router_with_shadow(Arc::new(NullShadowProvider));
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/shadow/rate")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"percent": 101}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// `POST /admin/shadow/rate` with percent = 0 disables shadow mode.
#[tokio::test]
async fn post_shadow_rate_zero_disables_shadow_mode() {
    let provider = TestShadowProvider::new(50);
    let app = router_with_shadow(provider.clone());

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/shadow/rate")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"percent": 0}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("sample_rate").and_then(Value::as_u64), Some(0));
}

/// `GET /admin/shadow/state` returns the expected fields.
#[tokio::test]
async fn get_shadow_state_returns_expected_fields() {
    let provider = TestShadowProvider::new(15);
    let app = router_with_shadow(provider);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/admin/shadow/state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("sample_rate").is_some(), "sample_rate field must be present");
    assert!(v.get("gate_tripped_count").is_some(), "gate_tripped_count must be present");
    assert!(v.get("last_divergence_rate").is_some(), "last_divergence_rate must be present");
    assert!(v.get("window_samples").is_some(), "window_samples must be present");
    assert!(v.get("gate_enabled").is_some(), "gate_enabled must be present");
    assert!(v.get("gate_max_divergence_rate").is_some(), "gate_max_divergence_rate must be present");
    // Verify the actual rate is 15.
    assert_eq!(v.get("sample_rate").and_then(Value::as_u64), Some(15));
}

/// `POST /admin/shadow/gate` reconfigures enabled flag.
#[tokio::test]
async fn post_shadow_gate_reconfigures_enabled() {
    let provider = TestShadowProvider::new(10);
    let app = router_with_shadow(provider);

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/shadow/gate")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"enabled": false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = to_bytes(resp.into_body(), 4096).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.get("gate_enabled").and_then(Value::as_bool), Some(false));
}

/// `POST /admin/shadow/gate` rejects out-of-range max_divergence_rate.
#[tokio::test]
async fn post_shadow_gate_rejects_invalid_divergence_rate() {
    let app = router_with_shadow(Arc::new(NullShadowProvider));
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/shadow/gate")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"max_divergence_rate": 1.5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cross_origin_preflight_for_post_logging_is_not_allowed() {
    let app = router_no_auth();
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/logging")
                .header("origin", "https://evil.example")
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // With the CORS layer removed, OPTIONS on /logging falls through to the
    // axum method router — which responds 405 (method not allowed) because
    // the route only declares POST. No CORS headers either way.
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "preflight must NOT succeed: browsers would then allow the cross-origin POST"
    );
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "preflight must not advertise any allowed origin"
    );
    assert!(
        resp.headers().get("access-control-allow-methods").is_none(),
        "preflight must not advertise any allowed methods"
    );
}
