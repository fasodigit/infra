// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Async HTTP client for Prometheus instant-query API.
//!
//! Used by `CanaryOrchestrator` to poll SLO metrics during each 30 s tick.
//!
//! # Failure modes
//!
//! * **Prometheus unreachable** (TCP refused, timeout): `query_scalar` returns `Err`.
//!   The orchestrator treats this as "unknown" — not a breach.
//! * **PromQL query returns no result**: returns `Ok(0.0)` to avoid false rollbacks.
//! * **Non-200 HTTP status**: returns `Err` with the status code.

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

/// Raw SLO metrics returned from Prometheus for a single tick.
#[derive(Debug, Clone)]
pub struct SloMetrics {
    /// Error rate (0.0–1.0).
    pub error_rate: f64,
    /// p99 latency in milliseconds.
    pub latency_p99_ms: f64,
}

/// Lightweight async wrapper around the Prometheus HTTP API v1.
#[derive(Clone, Debug)]
pub struct PrometheusClient {
    base_url: String,
    client: Client,
}

/// Prometheus `/api/v1/query` response envelope.
#[derive(Debug, Deserialize)]
struct PromResponse {
    status: String,
    data: PromData,
}

#[derive(Debug, Deserialize)]
struct PromData {
    result: Vec<PromResult>,
}

#[derive(Debug, Deserialize)]
struct PromResult {
    value: (f64, String), // [unix_timestamp, "value_string"]
}

impl PrometheusClient {
    /// Create a new client pointed at `base_url` (e.g. `"http://prometheus:9090"`).
    pub fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client construction must not fail");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    /// Execute an instant PromQL query and return the first scalar result.
    ///
    /// Returns `Ok(0.0)` if the query returns no data (e.g. metric not yet emitted).
    /// Returns `Err` on HTTP error or parse failure.
    pub async fn query_scalar(&self, query: &str) -> Result<f64, String> {
        let url = format!("{}/api/v1/query", self.base_url);

        debug!(url = %url, query = %query, "Prometheus instant query");

        let resp = self
            .client
            .get(&url)
            .query(&[("query", query)])
            .send()
            .await
            .map_err(|e| format!("Prometheus HTTP request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Prometheus returned HTTP {}",
                resp.status().as_u16()
            ));
        }

        let body: PromResponse = resp
            .json()
            .await
            .map_err(|e| format!("Prometheus response parse error: {e}"))?;

        if body.status != "success" {
            return Err(format!("Prometheus query failed with status={}", body.status));
        }

        // Return first result's value, or 0.0 when no data.
        match body.data.result.first() {
            None => Ok(0.0),
            Some(r) => r
                .value
                .1
                .parse::<f64>()
                .map_err(|e| format!("Prometheus value parse error: {e}")),
        }
    }
}
