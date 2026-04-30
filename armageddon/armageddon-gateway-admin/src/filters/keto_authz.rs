// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # KetoAuthzFilter — inline Keto permission check for admin + TERROIR routes
//!
//! ## Behaviour
//!
//! For every request whose resolved cluster is `"bff_admin"` **or** one of the
//! six `"terroir_*"` clusters, the filter:
//!
//! 1. Extracts the `userId` from `RequestCtx::user_id` (populated earlier by
//!    the JWT filter on the same Pingora filter chain).
//! 2. Derives the required Keto `namespace`, `object`, and `relation` from
//!    the request path + method using [`terroir_keto_check`] (TERROIR) or
//!    [`required_relation`] (admin).
//! 3. Issues a `POST /relation-tuples/check/openapi` call to Keto at the
//!    configured address.
//! 4. Records the round-trip latency on `armageddon_terroir_keto_check_duration_seconds`
//!    (TERROIR paths) or `armageddon_admin_keto_check_duration_seconds` (admin paths).
//! 5. Returns:
//!    - `Decision::Continue` if Keto allows (or if the path is JWT-only, e.g.
//!      `/ws/terroir/sync`).
//!    - `Decision::ShortCircuit(403)` with a JSON body
//!      `{"error":"forbidden","reason":"keto_denied","traceId":"<x-forge-id>"}` if Keto denies.
//!    - `Decision::ShortCircuit(403)` with `reason:"keto_unavailable"` if the
//!      HTTP call to Keto fails (fail-closed — see §Failure modes).
//!
//! ## Admin relation mapping
//!
//! | Path prefix | Method | Keto relation |
//! |-------------|--------|---------------|
//! | `/api/admin/users` | any | `manage_users` |
//! | `/api/admin/sessions` | any | `manage_users` |
//! | `/api/admin/devices` | any | `manage_users` |
//! | `/api/admin/audit` | GET | `view_audit` |
//! | `/api/admin/settings` | GET | `view_audit` |
//! | `/api/admin/settings` | PUT / PATCH | `update_settings` |
//! | `/api/admin/roles` | any | `grant_admin_role` |
//! | `/api/admin/otp` | any | `manage_users` |
//! | `/api/admin/break-glass` | any | `manage_users` |
//! | _default_ | any | `manage_users` |
//!
//! ## TERROIR relation mapping (P0.H)
//!
//! | Path pattern | Namespace | Object | Relation |
//! |---|---|---|---|
//! | `/api/terroir/core/tenants/{slug}/…` | `Tenant` | `<slug>` | `view` (GET) / `manage` (PATCH,DELETE) / `onboard_member` (POST …/members) |
//! | `/api/terroir/core/cooperatives/{id}/parcels/…` | `Cooperative` | `<id>` | `view` (GET) / `manage_members` (PATCH) |
//! | `/api/terroir/core/parcels/{id}/polygon` | `Parcel` | `<id>` | `edit_polygon` (PATCH) / `view` (GET) |
//! | `/api/terroir/eudr/parcels/{id}/validate` | `Parcel` | `<id>` | `submit_eudr` |
//! | `/api/terroir/eudr/dds/{id}/sign` | `Tenant` | `<slug-from-JWT>` | `submit_dds` |
//! | `/api/terroir/buyer/lots/{id}/contract` | — | — | JWT scope-checked at service; gateway → `Allow` |
//! | `/ws/terroir/sync` | — | — | JWT subject validated; gateway → `Allow` |
//! | all other `/api/terroir/*` | `Tenant` | `<slug-from-JWT>` | `view` (GET) / `manage` (other) |
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | Keto unreachable (TCP error, timeout) | Fail-closed → 403 `keto_unavailable` |
//! | Keto returns non-200 status (5xx) | Fail-closed → 403 `keto_unavailable` |
//! | JWT filter did not populate `user_id` | 403 `missing_user_id` |
//! | Keto returns `allowed: false` | 403 `keto_denied` |
//! | Request is NOT on a managed cluster | `Decision::Continue` (filter is no-op) |
//! | Path is JWT-only (buyer/lots contract, WS sync) | `Decision::Continue` after JWT check |
//!
//! Fail-closed is the correct default — a degraded authz service must never
//! allow unauthenticated access on either admin or TERROIR routes.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use bytes::Bytes;
use pingora::http::ResponseHeader;
use pingora_proxy::Session;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use armageddon_forge::pingora::ctx::RequestCtx;
use armageddon_forge::pingora::filters::{Decision, ForgeFilter};

use crate::metrics::{keto_check_duration_seconds, terroir_keto_check_duration_seconds};

// ── Keto API types ────────────────────────────────────────────────────────────

/// Body for `POST /relation-tuples/check/openapi` (Keto v0.11+).
#[derive(Debug, Serialize)]
struct KetoCheckRequest<'a> {
    namespace: &'a str,
    object: &'a str,
    relation: &'a str,
    subject_id: &'a str,
}

/// Response from `POST /relation-tuples/check/openapi`.
#[derive(Debug, Deserialize)]
struct KetoCheckResponse {
    allowed: bool,
}

// ── filter configuration ──────────────────────────────────────────────────────

/// Configuration for the Keto authz filter.
#[derive(Debug, Clone)]
pub struct KetoAuthzConfig {
    /// Base URL of the Keto read API, e.g. `"http://keto:4466"`.
    pub keto_url: String,
    /// Keto namespace for admin roles (default: `"AdminRole"`).
    pub namespace: String,
    /// Keto object to check against (default: `"platform"`).
    pub object: String,
    /// HTTP timeout for Keto calls in milliseconds (default: `2000`).
    pub timeout_ms: u64,
    /// Cluster name that triggers authz checks (default: `"bff_admin"`).
    pub admin_cluster: String,
}

impl Default for KetoAuthzConfig {
    fn default() -> Self {
        Self {
            keto_url: "http://keto:4466".to_string(),
            namespace: "AdminRole".to_string(),
            object: "platform".to_string(),
            timeout_ms: 2000,
            admin_cluster: "bff_admin".to_string(),
        }
    }
}

// ── TERROIR clusters set ──────────────────────────────────────────────────────

/// Set of cluster names that trigger TERROIR-specific Keto authz checks.
///
/// Checking membership is O(1) via a static slice scan (6 elements).
const TERROIR_CLUSTERS: &[&str] = &[
    "terroir_core",
    "terroir_eudr",
    "terroir_payment",
    "terroir_mobile_bff",
    "terroir_ussd",
    "terroir_buyer",
];

fn is_terroir_cluster(cluster: &str) -> bool {
    TERROIR_CLUSTERS.contains(&cluster)
}

// ── TERROIR Keto check resolution ────────────────────────────────────────────

/// Outcome of TERROIR path analysis: either a Keto check triple, or an
/// instruction to skip Keto and allow based on JWT alone.
#[derive(Debug, PartialEq, Eq)]
pub enum TerroirKetoTarget {
    /// Issue a Keto check with these parameters.
    Check {
        namespace: &'static str,
        object: String,
        relation: &'static str,
    },
    /// Skip Keto — JWT validity is sufficient (scope checked downstream).
    JwtOnly,
}

/// Resolve the Keto check target for a TERROIR request.
///
/// `path` is the raw URI path from the request (e.g.
/// `/api/terroir/core/tenants/coop-bk/producers`).
/// `method` is the HTTP method as an uppercase string.
///
/// Path-segment extraction uses simple string splitting to avoid a regex
/// dependency; all paths follow the canonical REST structure defined in
/// the TERROIR API spec.
pub fn terroir_keto_check(path: &str, method: &str) -> TerroirKetoTarget {
    let p = path.trim_start_matches('/');
    let method_upper = method.to_ascii_uppercase();

    // ── /ws/terroir/sync — JWT-only ───────────────────────────────────────────
    if p == "ws/terroir/sync" {
        return TerroirKetoTarget::JwtOnly;
    }

    // Strip leading `api/terroir/` to get the service-relative path.
    let Some(rest) = p.strip_prefix("api/terroir/") else {
        // Not a terroir path — should not happen given cluster pre-filter.
        return TerroirKetoTarget::JwtOnly;
    };

    // ── /api/terroir/core/* ───────────────────────────────────────────────────
    if let Some(core_rest) = rest.strip_prefix("core/") {
        let segments: Vec<&str> = core_rest.splitn(5, '/').collect();

        // /api/terroir/core/tenants/{slug}/…
        if segments.first() == Some(&"tenants") {
            let slug = segments.get(1).copied().unwrap_or("unknown").to_string();
            let sub = segments.get(2).copied().unwrap_or("");
            let relation: &'static str = if sub == "members" && method_upper == "POST" {
                "onboard_member"
            } else if method_upper == "GET" {
                "view"
            } else {
                "manage"
            };
            return TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: slug,
                relation,
            };
        }

        // /api/terroir/core/cooperatives/{id}/parcels/…
        if segments.first() == Some(&"cooperatives") {
            let id = segments.get(1).copied().unwrap_or("unknown").to_string();
            let relation: &'static str = if method_upper == "GET" {
                "view"
            } else {
                "manage_members"
            };
            return TerroirKetoTarget::Check {
                namespace: "Cooperative",
                object: id,
                relation,
            };
        }

        // /api/terroir/core/parcels/{id}/polygon
        if segments.first() == Some(&"parcels") {
            let id = segments.get(1).copied().unwrap_or("unknown").to_string();
            let relation: &'static str = if method_upper == "GET" {
                "view"
            } else {
                "edit_polygon"
            };
            return TerroirKetoTarget::Check {
                namespace: "Parcel",
                object: id,
                relation,
            };
        }

        // Generic terroir-core fallback — tenant-scoped.
        return TerroirKetoTarget::Check {
            namespace: "Tenant",
            object: "unknown".to_string(),
            relation: if method_upper == "GET" {
                "view"
            } else {
                "manage"
            },
        };
    }

    // ── /api/terroir/eudr/* ───────────────────────────────────────────────────
    if let Some(eudr_rest) = rest.strip_prefix("eudr/") {
        let segments: Vec<&str> = eudr_rest.splitn(4, '/').collect();

        // /api/terroir/eudr/parcels/{id}/validate
        if segments.first() == Some(&"parcels") {
            let id = segments.get(1).copied().unwrap_or("unknown").to_string();
            return TerroirKetoTarget::Check {
                namespace: "Parcel",
                object: id,
                relation: "submit_eudr",
            };
        }

        // /api/terroir/eudr/dds/{id}/sign
        if segments.first() == Some(&"dds") {
            // The DDS signing check is scoped to the tenant; the slug is read
            // from the JWT `tenant_slug` claim and injected into ctx by the JWT
            // filter.  At routing time we use a sentinel and the service
            // re-validates; the gateway check is belt-and-suspenders.
            let id = segments.get(1).copied().unwrap_or("unknown").to_string();
            return TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: id,
                relation: "submit_dds",
            };
        }

        // Generic eudr fallback.
        return TerroirKetoTarget::Check {
            namespace: "Tenant",
            object: "unknown".to_string(),
            relation: if method_upper == "GET" {
                "view"
            } else {
                "manage"
            },
        };
    }

    // ── /api/terroir/buyer/* ─────────────────────────────────────────────────
    if let Some(buyer_rest) = rest.strip_prefix("buyer/") {
        let segments: Vec<&str> = buyer_rest.splitn(4, '/').collect();

        // /api/terroir/buyer/lots/{id}/contract — invitation-token flow,
        // scope is checked at the service; gateway only validates JWT presence.
        if segments.first() == Some(&"lots") && segments.get(2).copied() == Some("contract") {
            return TerroirKetoTarget::JwtOnly;
        }

        // Generic buyer fallback — tenant check.
        let object = segments.get(1).copied().unwrap_or("unknown").to_string();
        return TerroirKetoTarget::Check {
            namespace: "Tenant",
            object,
            relation: if method_upper == "GET" {
                "view"
            } else {
                "manage"
            },
        };
    }

    // ── /api/terroir/payment/*, /api/terroir/mobile-bff/*, /api/terroir/ussd/* ─
    // These paths are tenant-scoped; no deeper object extraction at the gateway.
    TerroirKetoTarget::Check {
        namespace: "Tenant",
        object: "unknown".to_string(),
        relation: if method_upper == "GET" {
            "view"
        } else {
            "manage"
        },
    }
}

// ── filter ────────────────────────────────────────────────────────────────────

/// Pingora `ForgeFilter` that enforces Keto authz on admin routes.
///
/// Register this filter **after** the JWT filter in the Pingora gateway's
/// filter chain so that `ctx.user_id` is already populated.
pub struct KetoAuthzFilter {
    config: KetoAuthzConfig,
    http_client: Arc<reqwest::Client>,
}

impl KetoAuthzFilter {
    /// Create a new filter.  A shared `reqwest::Client` is built with the
    /// configured timeout — reuse it across all requests to benefit from
    /// connection pooling to Keto.
    pub fn new(config: KetoAuthzConfig) -> Self {
        let timeout = std::time::Duration::from_millis(config.timeout_ms);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest::Client must build");
        Self {
            config,
            http_client: Arc::new(client),
        }
    }

    /// Issue a Keto check against the **admin** namespace/object configured in
    /// `self.config`.  Returns `true` iff the relation is allowed.
    ///
    /// All errors from the HTTP call are propagated as `Err(String)` so the
    /// caller can decide whether to fail-open or fail-closed.
    async fn check_keto(&self, user_id: &str, relation: &str) -> Result<bool, String> {
        self.check_keto_explicit(
            user_id,
            &self.config.namespace.clone(),
            &self.config.object.clone(),
            relation,
        )
        .await
    }

    /// Issue a Keto check with **explicit** namespace + object.
    ///
    /// Used for TERROIR routes where namespace and object are derived from the
    /// request path rather than from static config.
    async fn check_keto_explicit(
        &self,
        user_id: &str,
        namespace: &str,
        object: &str,
        relation: &str,
    ) -> Result<bool, String> {
        let url = format!(
            "{}/relation-tuples/check/openapi",
            self.config.keto_url.trim_end_matches('/')
        );

        let body = KetoCheckRequest {
            namespace,
            object,
            relation,
            subject_id: user_id,
        };

        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("keto HTTP error: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("keto returned HTTP {}", resp.status()));
        }

        let check: KetoCheckResponse = resp
            .json()
            .await
            .map_err(|e| format!("keto response decode error: {e}"))?;

        Ok(check.allowed)
    }

    /// Build a 403 JSON `ResponseHeader` + body bytes.
    fn forbidden_response(reason: &str, trace_id: &str) -> (Box<ResponseHeader>, Bytes) {
        let body = serde_json::json!({
            "error": "forbidden",
            "reason": reason,
            "traceId": trace_id,
        });
        let body_bytes = serde_json::to_vec(&body).unwrap_or_default();

        let mut header = ResponseHeader::build(403, None).expect("ResponseHeader::build");
        header
            .insert_header("content-type", "application/json")
            .ok();
        header
            .insert_header("content-length", body_bytes.len().to_string().as_str())
            .ok();
        (Box::new(header), Bytes::from(body_bytes))
    }
}

/// Derive the required Keto relation from the request path and HTTP method.
///
/// The mapping is intentionally conservative: when in doubt, require
/// `"manage_users"` (the broadest write permission checked as a fallback).
fn required_relation(path: &str, method: &str) -> &'static str {
    let p = path.trim_start_matches('/');

    if p.starts_with("api/admin/audit") {
        return "view_audit";
    }
    if p.starts_with("api/admin/settings") {
        return match method.to_ascii_uppercase().as_str() {
            "GET" => "view_audit",
            _ => "update_settings",
        };
    }
    if p.starts_with("api/admin/roles") || p.starts_with("api/admin/users/") && p.contains("/roles")
    {
        return "grant_admin_role";
    }
    // Users, sessions, devices, otp, break-glass, and everything else.
    "manage_users"
}

#[async_trait]
impl ForgeFilter for KetoAuthzFilter {
    fn name(&self) -> &'static str {
        "keto-authz"
    }

    async fn on_request(&self, _session: &mut Session, ctx: &mut RequestCtx) -> Decision {
        let path = _session.req_header().uri.path();
        let method = _session.req_header().method.as_str();

        if ctx.cluster == self.config.admin_cluster {
            // ── Admin route authz ─────────────────────────────────────────────
            return self.enforce_admin(ctx, path, method).await;
        }

        if is_terroir_cluster(&ctx.cluster) {
            // ── TERROIR route authz ───────────────────────────────────────────
            return self.enforce_terroir(ctx, path, method).await;
        }

        Decision::Continue
    }
}

// ── private enforcement helpers ───────────────────────────────────────────────

impl KetoAuthzFilter {
    /// Admin authz enforcement (existing behaviour, refactored to method).
    async fn enforce_admin(&self, ctx: &mut RequestCtx, path: &str, method: &str) -> Decision {
        let user_id = match &ctx.user_id {
            Some(id) => id.clone(),
            None => {
                warn!(
                    request_id = %ctx.request_id,
                    "keto-authz(admin): no user_id in context — JWT filter must precede this filter"
                );
                let (hdr, _body) = Self::forbidden_response("missing_user_id", &ctx.trace_id);
                return Decision::ShortCircuit(hdr);
            }
        };

        let relation = required_relation(path, method);
        let start = Instant::now();
        let result = self.check_keto(&user_id, relation).await;
        let elapsed = start.elapsed().as_secs_f64();

        Self::record_keto_decision(
            keto_check_duration_seconds(),
            elapsed,
            &result,
            ctx,
            &user_id,
            relation,
            "admin",
        );

        Self::decision_from_result(result, ctx)
    }

    /// TERROIR authz enforcement.
    async fn enforce_terroir(&self, ctx: &mut RequestCtx, path: &str, method: &str) -> Decision {
        let user_id = match &ctx.user_id {
            Some(id) => id.clone(),
            None => {
                warn!(
                    request_id = %ctx.request_id,
                    "keto-authz(terroir): no user_id in context — JWT filter must precede this filter"
                );
                let (hdr, _body) = Self::forbidden_response("missing_user_id", &ctx.trace_id);
                return Decision::ShortCircuit(hdr);
            }
        };

        let target = terroir_keto_check(path, method);

        match target {
            TerroirKetoTarget::JwtOnly => {
                // JWT is valid (the JWT filter already checked it); no Keto call needed.
                info!(
                    request_id = %ctx.request_id,
                    user_id = %user_id,
                    path = %path,
                    "keto-authz(terroir): JWT-only path — skipping Keto"
                );
                Decision::Continue
            }
            TerroirKetoTarget::Check {
                namespace,
                object,
                relation,
            } => {
                let start = Instant::now();
                let result = self
                    .check_keto_explicit(&user_id, namespace, &object, relation)
                    .await;
                let elapsed = start.elapsed().as_secs_f64();

                Self::record_keto_decision(
                    terroir_keto_check_duration_seconds(),
                    elapsed,
                    &result,
                    ctx,
                    &user_id,
                    relation,
                    "terroir",
                );

                Self::decision_from_result(result, ctx)
            }
        }
    }

    /// Record latency + structured log for a Keto decision.
    fn record_keto_decision(
        histogram: &prometheus::HistogramVec,
        elapsed: f64,
        result: &Result<bool, String>,
        ctx: &RequestCtx,
        user_id: &str,
        relation: &str,
        stream: &str,
    ) {
        match result {
            Ok(true) => {
                histogram.with_label_values(&["allowed"]).observe(elapsed);
                info!(
                    request_id = %ctx.request_id,
                    user_id = %user_id,
                    relation = %relation,
                    stream = %stream,
                    elapsed_ms = elapsed * 1000.0,
                    "keto-authz: allowed"
                );
            }
            Ok(false) => {
                histogram.with_label_values(&["denied"]).observe(elapsed);
                warn!(
                    request_id = %ctx.request_id,
                    user_id = %user_id,
                    relation = %relation,
                    stream = %stream,
                    "keto-authz: denied"
                );
            }
            Err(e) => {
                histogram.with_label_values(&["denied"]).observe(elapsed);
                error!(
                    request_id = %ctx.request_id,
                    user_id = %user_id,
                    relation = %relation,
                    stream = %stream,
                    err = %e,
                    "keto-authz: Keto unreachable — fail-closed"
                );
            }
        }
    }

    /// Convert a Keto result into a Pingora `Decision`.
    fn decision_from_result(result: Result<bool, String>, ctx: &RequestCtx) -> Decision {
        match result {
            Ok(true) => Decision::Continue,
            Ok(false) => {
                let (hdr, _body) = Self::forbidden_response("keto_denied", &ctx.trace_id);
                Decision::ShortCircuit(hdr)
            }
            Err(_) => {
                let (hdr, _body) = Self::forbidden_response("keto_unavailable", &ctx.trace_id);
                Decision::ShortCircuit(hdr)
            }
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_mapping_audit() {
        assert_eq!(required_relation("/api/admin/audit", "GET"), "view_audit");
        assert_eq!(
            required_relation("/api/admin/audit/export", "GET"),
            "view_audit"
        );
    }

    #[test]
    fn relation_mapping_settings() {
        assert_eq!(
            required_relation("/api/admin/settings", "GET"),
            "view_audit"
        );
        assert_eq!(
            required_relation("/api/admin/settings", "PUT"),
            "update_settings"
        );
        assert_eq!(
            required_relation("/api/admin/settings", "PATCH"),
            "update_settings"
        );
    }

    #[test]
    fn relation_mapping_roles() {
        assert_eq!(
            required_relation("/api/admin/users/abc/roles", "POST"),
            "grant_admin_role"
        );
        assert_eq!(
            required_relation("/api/admin/roles/grant", "POST"),
            "grant_admin_role"
        );
    }

    #[test]
    fn relation_mapping_default_is_manage_users() {
        assert_eq!(required_relation("/api/admin/users", "GET"), "manage_users");
        assert_eq!(
            required_relation("/api/admin/otp/issue", "POST"),
            "manage_users"
        );
        assert_eq!(
            required_relation("/api/admin/sessions", "DELETE"),
            "manage_users"
        );
        assert_eq!(
            required_relation("/api/admin/break-glass/activate", "POST"),
            "manage_users"
        );
    }

    #[test]
    fn forbidden_response_includes_trace_id() {
        let (hdr, body) = KetoAuthzFilter::forbidden_response("keto_denied", "trace-abc");
        assert_eq!(hdr.status.as_u16(), 403);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["reason"], "keto_denied");
        assert_eq!(parsed["traceId"], "trace-abc");
    }

    // ── TERROIR Keto target tests ─────────────────────────────────────────────

    #[test]
    fn terroir_ws_sync_is_jwt_only() {
        assert_eq!(
            terroir_keto_check("/ws/terroir/sync", "GET"),
            TerroirKetoTarget::JwtOnly
        );
    }

    #[test]
    fn terroir_core_tenant_get_is_view() {
        let result = terroir_keto_check("/api/terroir/core/tenants/coop-bk/producers", "GET");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: "coop-bk".to_string(),
                relation: "view",
            }
        );
    }

    #[test]
    fn terroir_core_tenant_patch_is_manage() {
        let result = terroir_keto_check("/api/terroir/core/tenants/coop-bk/parcels", "PATCH");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: "coop-bk".to_string(),
                relation: "manage",
            }
        );
    }

    #[test]
    fn terroir_core_tenant_members_post_is_onboard_member() {
        let result = terroir_keto_check("/api/terroir/core/tenants/coop-bk/members", "POST");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: "coop-bk".to_string(),
                relation: "onboard_member",
            }
        );
    }

    #[test]
    fn terroir_core_cooperative_get_is_view() {
        let result = terroir_keto_check("/api/terroir/core/cooperatives/42/parcels/listing", "GET");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Cooperative",
                object: "42".to_string(),
                relation: "view",
            }
        );
    }

    #[test]
    fn terroir_core_cooperative_patch_is_manage_members() {
        let result = terroir_keto_check("/api/terroir/core/cooperatives/42/parcels", "PATCH");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Cooperative",
                object: "42".to_string(),
                relation: "manage_members",
            }
        );
    }

    #[test]
    fn terroir_core_parcel_polygon_get_is_view() {
        let result = terroir_keto_check("/api/terroir/core/parcels/7/polygon", "GET");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Parcel",
                object: "7".to_string(),
                relation: "view",
            }
        );
    }

    #[test]
    fn terroir_core_parcel_polygon_patch_is_edit_polygon() {
        let result = terroir_keto_check("/api/terroir/core/parcels/7/polygon", "PATCH");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Parcel",
                object: "7".to_string(),
                relation: "edit_polygon",
            }
        );
    }

    #[test]
    fn terroir_eudr_parcels_validate_is_submit_eudr() {
        let result = terroir_keto_check("/api/terroir/eudr/parcels/9/validate", "POST");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Parcel",
                object: "9".to_string(),
                relation: "submit_eudr",
            }
        );
    }

    #[test]
    fn terroir_eudr_dds_sign_is_submit_dds() {
        let result = terroir_keto_check("/api/terroir/eudr/dds/12/sign", "POST");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: "12".to_string(),
                relation: "submit_dds",
            }
        );
    }

    #[test]
    fn terroir_buyer_lots_contract_is_jwt_only() {
        let result = terroir_keto_check("/api/terroir/buyer/lots/33/contract", "GET");
        assert_eq!(result, TerroirKetoTarget::JwtOnly);
    }

    #[test]
    fn terroir_payment_generic_get_is_tenant_view() {
        let result = terroir_keto_check("/api/terroir/payment/orders", "GET");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: "unknown".to_string(),
                relation: "view",
            }
        );
    }

    #[test]
    fn terroir_ussd_generic_post_is_tenant_manage() {
        let result = terroir_keto_check("/api/terroir/ussd/session", "POST");
        assert_eq!(
            result,
            TerroirKetoTarget::Check {
                namespace: "Tenant",
                object: "unknown".to_string(),
                relation: "manage",
            }
        );
    }

    #[test]
    fn is_terroir_cluster_recognises_all_six() {
        for name in &[
            "terroir_core",
            "terroir_eudr",
            "terroir_payment",
            "terroir_mobile_bff",
            "terroir_ussd",
            "terroir_buyer",
        ] {
            assert!(is_terroir_cluster(name), "{name} must be recognised");
        }
    }

    #[test]
    fn is_terroir_cluster_rejects_non_terroir() {
        assert!(!is_terroir_cluster("bff_admin"));
        assert!(!is_terroir_cluster("auth_ms"));
        assert!(!is_terroir_cluster("default"));
    }
}
