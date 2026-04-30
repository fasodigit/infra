// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Axum route handlers for the Admin API.
//!
//! Route table:
//!   GET  /admin/health                  → 200 "OK"
//!   GET  /admin/config_dump             → JSON snapshot of active GatewayConfig
//!   GET  /admin/listeners               → JSON listeners + TLS state
//!   GET  /admin/clusters                → JSON clusters + endpoints + circuit-breaker state
//!   GET  /admin/stats                   → JSON metrics counters
//!   GET  /admin/stats/prometheus        → Prometheus text format (legacy alias)
//!   GET  /admin/metrics                 → Prometheus text exposition format
//!                                          (canonical scrape endpoint, unauthenticated)
//!   POST /admin/config/reload           → hot-reload from disk, return diff
//!   POST /admin/clusters/{name}/drain   → drain a cluster
//!   POST /admin/reset_counters          → reset stats counters

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::AdminError;
use crate::server::AdminConfig;
use crate::state::AdminState;
use crate::config_reload;

// -- auth middleware helper --

/// Validate the `X-Admin-Token` header when auth is enabled.
///
/// Uses `subtle::ConstantTimeEq` to prevent timing attacks.
fn check_auth(headers: &HeaderMap, cfg: &AdminConfig) -> Result<(), AdminError> {
    let Some(ref expected) = cfg.admin_token else {
        return Ok(()); // auth disabled
    };
    let provided = headers
        .get("X-Admin-Token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    use subtle::ConstantTimeEq;
    let ok: bool = provided
        .as_bytes()
        .ct_eq(expected.as_bytes())
        .into();

    if ok {
        Ok(())
    } else {
        tracing::warn!("admin: unauthorized access attempt (bad or missing X-Admin-Token)");
        Err(AdminError::Unauthorized)
    }
}

// -- handlers --

/// `GET /admin/health` — liveness probe, always 200.
async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// `GET /admin/config_dump` — JSON snapshot of the active GatewayConfig.
async fn handle_config_dump(
    State((state, _cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> impl IntoResponse {
    let config = state.config_store.load_full();
    Json(json!({ "config": *config }))
}

/// `GET /admin/listeners` — JSON list of listeners with TLS state.
async fn handle_listeners(
    State((state, _cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> impl IntoResponse {
    let config = state.config_store.load_full();
    let listeners: Vec<Value> = config
        .listeners
        .iter()
        .map(|l| {
            json!({
                "name": l.name,
                "address": l.address,
                "port": l.port,
                "protocol": serde_json::to_value(&l.protocol).unwrap_or(Value::Null),
                "tls_enabled": l.tls.is_some(),
                "tls_min_version": l.tls.as_ref().map(|t| &t.min_version),
                "tls_alpn": l.tls.as_ref().map(|t| &t.alpn),
            })
        })
        .collect();
    Json(json!({ "listeners": listeners }))
}

/// `GET /admin/clusters` — JSON clusters with endpoints and circuit-breaker state.
async fn handle_clusters(
    State((state, _cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> impl IntoResponse {
    let views = state.cluster_breakers.all_views();
    Json(json!({ "clusters": views }))
}

/// `GET /admin/stats` — JSON metrics dump.
async fn handle_stats(
    State((state, _cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> impl IntoResponse {
    let snapshot = state.stats.snapshot_json();
    Json(snapshot)
}

/// `GET /admin/stats/prometheus` — Prometheus text-format scrape endpoint.
async fn handle_stats_prometheus(
    State((state, _cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> Response<Body> {
    let text = state.stats.snapshot_prometheus_text();
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .body(Body::from(text))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })
}

/// `GET /admin/metrics` — canonical Prometheus scrape endpoint.
///
/// Returns metrics from the shared `prometheus::Registry` in text exposition
/// format (`text/plain; version=0.0.4; charset=utf-8`). Unauthenticated by
/// design — production deployments rely on the loopback bind + network
/// policy / firewall to scope access.
///
/// On encoder failure (rare — only on bad UTF-8 in label values) responds
/// with 500 and a brief error message.
pub(crate) async fn handle_metrics(
    State((state, _cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> Response<Body> {
    match state.stats.encode_prometheus() {
        Ok(text) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )
            .body(Body::from(text))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            }),
        Err(e) => {
            tracing::warn!(error = %e, "admin: prometheus encode failed");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(Body::from(format!("metrics encode failed: {e}")))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap()
                })
        }
    }
}

/// `POST /admin/config/reload` — reload config from disk, validate, swap.
async fn handle_config_reload(
    headers: HeaderMap,
    State((state, cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> Result<impl IntoResponse, AdminError> {
    check_auth(&headers, &cfg)?;

    let path_str = state.config_path.lock().clone();
    let path = std::path::Path::new(&path_str).to_path_buf();

    let diff = config_reload::reload(&path, &state.config_store).await?;

    // Refresh cluster runtime state to match the new config.
    let new_config = state.config_store.load_full();
    state.cluster_breakers.refresh(&new_config.clusters);

    Ok((StatusCode::OK, Json(json!({ "status": "reloaded", "diff": diff }))))
}

/// `POST /admin/clusters/{name}/drain` — initiate drain for a cluster.
async fn handle_cluster_drain(
    headers: HeaderMap,
    Path(name): Path<String>,
    State((state, cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> Result<impl IntoResponse, AdminError> {
    check_auth(&headers, &cfg)?;

    let breaker = state
        .cluster_breakers
        .get(&name)
        .ok_or_else(|| AdminError::ClusterNotFound(name.clone()))?;

    breaker.drain();
    tracing::info!(cluster = %name, "admin: cluster drain command acknowledged");

    Ok((
        StatusCode::OK,
        Json(json!({ "status": "draining", "cluster": name })),
    ))
}

/// `POST /admin/reset_counters` — reset all Prometheus counter offsets.
async fn handle_reset_counters(
    headers: HeaderMap,
    State((state, cfg)): State<(Arc<AdminState>, AdminConfig)>,
) -> Result<impl IntoResponse, AdminError> {
    check_auth(&headers, &cfg)?;

    state.stats.reset_counters();
    Ok((StatusCode::OK, Json(json!({ "status": "counters_reset" }))))
}

// -- router --

/// Build the Axum router for the Admin API.
pub fn build_router(state: Arc<AdminState>, cfg: AdminConfig) -> Router {
    let shared = (state, cfg);
    Router::new()
        .route("/admin/health", get(handle_health))
        .route("/admin/config_dump", get(handle_config_dump))
        .route("/admin/listeners", get(handle_listeners))
        .route("/admin/clusters", get(handle_clusters))
        .route("/admin/stats", get(handle_stats))
        .route("/admin/stats/prometheus", get(handle_stats_prometheus))
        .route("/admin/metrics", get(handle_metrics))
        .route("/admin/config/reload", post(handle_config_reload))
        .route(
            "/admin/clusters/:name/drain",
            post(handle_cluster_drain),
        )
        .route("/admin/reset_counters", post(handle_reset_counters))
        .with_state(shared)
}
