// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Prometheus metrics for the admin API itself.
//!
//! The admin-api increments `armageddon_admin_requests_total{path,method,status}`
//! on every request. The counter is registered on the default Prometheus
//! global registry so it naturally shows up in any `StatsProvider` that
//! dumps the global registry (TODO wiring).

use axum::{extract::Request, middleware::Next, response::Response};
use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, IntCounterVec};

lazy_static! {
    /// `armageddon_admin_requests_total{path, method, status}` — one inc
    /// per admin-api request after it is routed.
    pub static ref ADMIN_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "armageddon_admin_requests_total",
        "Total requests served by the ARMAGEDDON admin API.",
        &["path", "method", "status"]
    )
    .expect("register admin requests counter");
}

/// Axum middleware that increments `ADMIN_REQUESTS_TOTAL` after each request.
pub async fn track_request(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();
    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();
    ADMIN_REQUESTS_TOTAL
        .with_label_values(&[path.as_str(), method.as_str(), status.as_str()])
        .inc();
    response
}
