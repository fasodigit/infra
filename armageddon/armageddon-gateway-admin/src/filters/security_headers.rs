// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # SecurityHeadersFilter — WAF / security response headers for admin routes
//!
//! ## Injected headers
//!
//! On every **response** from a request routed to the `"bff_admin"` cluster:
//!
//! | Header | Value |
//! |--------|-------|
//! | `Strict-Transport-Security` | `max-age=63072000; includeSubDomains` |
//! | `X-Frame-Options` | `DENY` |
//! | `X-Content-Type-Options` | `nosniff` |
//! | `Cache-Control` | `no-store` (default) _or_ `private, max-age=30` for `GET /api/admin/settings` |
//!
//! ## Cache-Control special case
//!
//! `GET /api/admin/settings` returns slowly-changing data (30s TTL per CLAUDE-DESIGN §4.9).
//! For that specific path + method, `Cache-Control: private, max-age=30` is set
//! instead of `no-store` so the BFF can cache the response without leaking it
//! through a shared proxy.
//!
//! ## Failure modes
//!
//! Header insertion on a Pingora `ResponseHeader` is infallible — it either
//! succeeds or replaces an existing value.  This filter never short-circuits.

use async_trait::async_trait;
use pingora_proxy::Session;

use armageddon_forge::pingora::ctx::RequestCtx;
use armageddon_forge::pingora::filters::{Decision, ForgeFilter};

// ── filter ────────────────────────────────────────────────────────────────────

/// Pingora `ForgeFilter` that injects security response headers on admin routes.
pub struct SecurityHeadersFilter {
    /// Cluster name for which headers are injected (default: `"bff_admin"`).
    admin_cluster: String,
}

impl SecurityHeadersFilter {
    pub fn new(admin_cluster: impl Into<String>) -> Self {
        Self {
            admin_cluster: admin_cluster.into(),
        }
    }
}

impl Default for SecurityHeadersFilter {
    fn default() -> Self {
        Self::new("bff_admin")
    }
}

#[async_trait]
impl ForgeFilter for SecurityHeadersFilter {
    fn name(&self) -> &'static str {
        "security-headers"
    }

    async fn on_response(
        &self,
        session: &mut Session,
        res: &mut pingora::http::ResponseHeader,
        ctx: &mut RequestCtx,
    ) -> Decision {
        // Only inject headers on admin-cluster responses.
        if ctx.cluster != self.admin_cluster {
            return Decision::Continue;
        }

        // HSTS — two years, including sub-domains.
        res.insert_header(
            "Strict-Transport-Security",
            "max-age=63072000; includeSubDomains",
        )
        .ok();

        // Clickjacking protection.
        res.insert_header("X-Frame-Options", "DENY").ok();

        // MIME-type sniffing prevention.
        res.insert_header("X-Content-Type-Options", "nosniff").ok();

        // Cache-Control: default is no-store; GET /api/admin/settings is the
        // exception per CLAUDE-DESIGN §4.9 ("cache BFF 30s").
        let path = session.req_header().uri.path();
        let method = session.req_header().method.as_str();

        let cache_control = if is_settings_get(path, method) {
            "private, max-age=30"
        } else {
            "no-store"
        };
        res.insert_header("Cache-Control", cache_control).ok();

        Decision::Continue
    }
}

/// Returns `true` for `GET /api/admin/settings` (exact path).
///
/// Sub-paths like `/api/admin/settings/history` keep `no-store` because
/// history data should not be cached at all.
fn is_settings_get(path: &str, method: &str) -> bool {
    method.eq_ignore_ascii_case("GET")
        && (path == "/api/admin/settings" || path == "/api/admin/settings/")
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_get_matches_exact_path() {
        assert!(is_settings_get("/api/admin/settings", "GET"));
        assert!(is_settings_get("/api/admin/settings/", "GET"));
        assert!(is_settings_get("/api/admin/settings", "get"));
    }

    #[test]
    fn settings_get_does_not_match_sub_path() {
        assert!(!is_settings_get("/api/admin/settings/history", "GET"));
        assert!(!is_settings_get(
            "/api/admin/settings/otp.lifetime_seconds",
            "GET"
        ));
    }

    #[test]
    fn settings_put_does_not_match() {
        assert!(!is_settings_get("/api/admin/settings", "PUT"));
        assert!(!is_settings_get("/api/admin/settings", "PATCH"));
    }

    #[test]
    fn non_settings_path_does_not_match() {
        assert!(!is_settings_get("/api/admin/users", "GET"));
        assert!(!is_settings_get("/api/admin/audit", "GET"));
    }
}
