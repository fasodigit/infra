// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! HTTP handlers for the ARMAGEDDON admin API.

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::state::{AdminApiState, ServerInfo};

// -- helpers --

fn yaml_response(body: String) -> Response {
    let mut resp = body.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/yaml"),
    );
    resp
}

fn plain_text(body: String) -> Response {
    let mut resp = body.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    resp
}

// -- /stats --

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    #[serde(default)]
    pub format: Option<String>,
}

/// `GET /stats` — Prometheus exposition (or JSON with `?format=json`).
pub async fn get_stats(
    State(state): State<Arc<AdminApiState>>,
    Query(q): Query<StatsQuery>,
) -> Response {
    if matches!(q.format.as_deref(), Some("json")) {
        let tree = state.stats.prometheus_json().await;
        Json(tree).into_response()
    } else {
        let text = state.stats.prometheus_text().await;
        plain_text(text)
    }
}

// -- /clusters --

/// `GET /clusters` — list of upstream clusters with health + breaker state.
pub async fn get_clusters(State(state): State<Arc<AdminApiState>>) -> Response {
    let clusters = state.clusters.clusters().await;
    Json(serde_json::json!({ "clusters": clusters })).into_response()
}

// -- /config_dump --

#[derive(Debug, Deserialize)]
pub struct ConfigDumpQuery {
    #[serde(default)]
    pub format: Option<String>,
}

/// `GET /config_dump` — JSON by default, `?format=yaml` for YAML.
pub async fn get_config_dump(
    State(state): State<Arc<AdminApiState>>,
    Query(q): Query<ConfigDumpQuery>,
) -> Response {
    if matches!(q.format.as_deref(), Some("yaml")) {
        let yaml = state.config.dump_yaml().await;
        yaml_response(yaml)
    } else {
        let json = state.config.dump_json().await;
        Json(json).into_response()
    }
}

// -- /runtime --

/// `GET /runtime` — runtime flag / feature-flag values.
pub async fn get_runtime(State(state): State<Arc<AdminApiState>>) -> Response {
    let flags = state.runtime.runtime_flags().await;
    let map: BTreeMap<String, serde_json::Value> = flags.into_iter().collect();
    Json(map).into_response()
}

// -- /server_info --

#[derive(Debug, Serialize)]
pub struct ServerInfoView {
    pub version: String,
    pub build_sha: String,
    pub hostname: String,
    pub started_at: String,
    pub uptime_secs: u64,
}

/// `GET /server_info` — version, uptime, build SHA, hostname.
pub async fn get_server_info(State(state): State<Arc<AdminApiState>>) -> Json<ServerInfoView> {
    let ServerInfo {
        version,
        build_sha,
        hostname,
        started_at,
    } = state.server_info.as_ref().clone();

    let now = chrono::Utc::now();
    let uptime_secs = now
        .signed_duration_since(started_at)
        .num_seconds()
        .max(0) as u64;

    Json(ServerInfoView {
        version,
        build_sha,
        hostname,
        started_at: started_at.to_rfc3339(),
        uptime_secs,
    })
}

// -- /listeners --

/// `GET /listeners` — active listeners on the gateway.
pub async fn get_listeners(State(state): State<Arc<AdminApiState>>) -> Response {
    let listeners = state.clusters.listeners().await;
    Json(serde_json::json!({ "listeners": listeners })).into_response()
}

// -- /health --

/// `GET /health` — aggregated health of ARMAGEDDON engines.
pub async fn get_health(State(state): State<Arc<AdminApiState>>) -> Response {
    let h = state.health.aggregated_health().await;
    let status_code = if h.status == "SERVING" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status_code, Json(h)).into_response()
}

// -- POST /logging --

#[derive(Debug, Deserialize)]
pub struct LoggingRequest {
    pub level: String,
}

#[derive(Debug, Serialize)]
pub struct LoggingResponse {
    pub previous: Option<String>,
    pub current: String,
}

/// `POST /logging` — dynamic log-level change.
pub async fn post_logging(
    State(state): State<Arc<AdminApiState>>,
    Json(body): Json<LoggingRequest>,
) -> Result<Json<LoggingResponse>, (StatusCode, String)> {
    let normalized = body.level.trim().to_ascii_lowercase();
    let valid = matches!(
        normalized.as_str(),
        "trace" | "debug" | "info" | "warn" | "error" | "off"
    );
    if !valid {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "invalid log level '{}': expected one of trace|debug|info|warn|error|off",
                body.level
            ),
        ));
    }

    let previous = state.swap_log_level(normalized.clone());
    tracing::info!(
        previous = ?previous,
        current = %normalized,
        "admin-api: log level updated"
    );
    Ok(Json(LoggingResponse {
        previous,
        current: normalized,
    }))
}

// -- fallback --

pub async fn not_found() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "unknown admin endpoint")
}
