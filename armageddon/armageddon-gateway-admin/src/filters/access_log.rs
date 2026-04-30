// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # AdminAccessLogFilter — per-request metrics recorder for admin routes
//!
//! ## Behaviour
//!
//! Runs in the `on_logging` hook (post-flush, always called regardless of
//! short-circuiting) and increments
//! `armageddon_admin_requests_total{path, method, status}`.
//!
//! The `path` label is **normalised** to strip dynamic path segments
//! (e.g. `/api/admin/users/uuid-123` → `/api/admin/users/:id`) so label
//! cardinality stays bounded for Prometheus.
//!
//! ## Failure modes
//!
//! `on_logging` never fails — any error is swallowed and logged.

use async_trait::async_trait;
use pingora_proxy::Session;
use tracing::debug;

use armageddon_forge::pingora::ctx::RequestCtx;
use armageddon_forge::pingora::filters::ForgeFilter;

use crate::metrics::admin_requests_total;

// ── filter ────────────────────────────────────────────────────────────────────

/// Pingora `ForgeFilter` that records per-request Prometheus metrics.
pub struct AdminAccessLogFilter {
    admin_cluster: String,
}

impl AdminAccessLogFilter {
    pub fn new(admin_cluster: impl Into<String>) -> Self {
        Self {
            admin_cluster: admin_cluster.into(),
        }
    }
}

impl Default for AdminAccessLogFilter {
    fn default() -> Self {
        Self::new("bff_admin")
    }
}

#[async_trait]
impl ForgeFilter for AdminAccessLogFilter {
    fn name(&self) -> &'static str {
        "admin-access-log"
    }

    async fn on_logging(&self, session: &mut Session, ctx: &RequestCtx) {
        // Only record on admin cluster (or jwks cluster for /.well-known/).
        if ctx.cluster != self.admin_cluster && ctx.cluster != "auth_ms" {
            return;
        }

        let raw_path = session.req_header().uri.path();
        let method = session.req_header().method.as_str();
        let status = session
            .response_written()
            .map(|r| r.status.as_u16())
            .unwrap_or(0)
            .to_string();
        let normalised = normalise_path(raw_path);

        debug!(
            request_id = %ctx.request_id,
            path = %normalised,
            method = %method,
            status = %status,
            "admin access log"
        );

        admin_requests_total()
            .with_label_values(&[&normalised, method, &status])
            .inc();
    }
}

/// Normalise a path by replacing UUID-like and numeric segments with
/// fixed placeholders so Prometheus label cardinality is bounded.
///
/// Examples:
/// - `/api/admin/users/018e5f3c-dead-beef-0000-000000000001/roles` → `/api/admin/users/:id/roles`
/// - `/api/admin/sessions/42` → `/api/admin/sessions/:id`
/// - `/api/admin/settings` → `/api/admin/settings` (unchanged)
fn normalise_path(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            if seg.is_empty() {
                seg.to_string()
            } else if looks_like_id(seg) {
                ":id".to_string()
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Heuristic: a segment looks like a dynamic ID if it is a UUID, a pure
/// decimal number, or longer than 20 characters (likely an opaque token).
fn looks_like_id(seg: &str) -> bool {
    // UUID: 8-4-4-4-12 hex with dashes
    let bytes = seg.as_bytes();
    if bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
    {
        return true;
    }
    // Pure numeric
    if seg.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // Long opaque token (e.g. base64 / hex)
    if seg.len() > 20
        && seg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return true;
    }
    false
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_uuid_segment() {
        let path = "/api/admin/users/018e5f3c-dead-beef-0000-000000000001/roles";
        assert_eq!(normalise_path(path), "/api/admin/users/:id/roles");
    }

    #[test]
    fn normalise_numeric_segment() {
        let path = "/api/admin/sessions/42";
        assert_eq!(normalise_path(path), "/api/admin/sessions/:id");
    }

    #[test]
    fn normalise_static_path_unchanged() {
        let path = "/api/admin/settings";
        assert_eq!(normalise_path(path), "/api/admin/settings");
    }

    #[test]
    fn normalise_jwks() {
        let path = "/.well-known/jwks.json";
        assert_eq!(normalise_path(path), "/.well-known/jwks.json");
    }
}
