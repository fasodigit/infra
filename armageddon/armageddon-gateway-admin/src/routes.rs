// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # Admin + TERROIR route declarations for ARMAGEDDON
//!
//! ## Admin route table
//!
//! | Path prefix | Cluster | Timeout | Notes |
//! |-------------|---------|---------|-------|
//! | `/api/admin/` | `bff_admin` | 30 s | All admin endpoints routed to BFF :4800 |
//! | `/.well-known/jwks.json` | `auth_ms` | 5 s | JWKS endpoint — auth-ms :8801 already registered |
//!
//! ## TERROIR route table (P0.H)
//!
//! | Path prefix | Cluster | Timeout | Notes |
//! |-------------|---------|---------|-------|
//! | `/api/terroir/core/` | `terroir_core` | 10 s | Producteurs, parcelles, household — terroir-core :8830 |
//! | `/api/terroir/eudr/` | `terroir_eudr` | 60 s | Validation EUDR + DDS — terroir-eudr :8831 |
//! | `/api/terroir/payment/` | `terroir_payment` | 30 s | Mobile money orchestration — terroir-payment :8832 |
//! | `/api/terroir/mobile-bff/` | `terroir_mobile_bff` | 15 s | BFF agent terrain — terroir-mobile-bff :8833 |
//! | `/api/terroir/ussd/` | `terroir_ussd` | 5 s | Gateway USSD/SMS interactif — terroir-ussd :8834 |
//! | `/api/terroir/buyer/` | `terroir_buyer` | 30 s | Portail acheteurs DDS — terroir-buyer :8835 |
//! | `/ws/terroir/sync` | `terroir_mobile_bff` | — | Sync CRDT delta-encoded (WS upgrade) |
//!
//! ## Timeout rationale (admin)
//!
//! Some admin operations (CSV export, bulk recovery-code generation) can take
//! up to 30 s.  The default gateway timeout of 5 s would cause spurious
//! 504 errors for those endpoints, so admin routes get a dedicated 30 s budget.
//!
//! The JWKS endpoint retains the 5 s default because it is latency-sensitive
//! (it is called on every JWT cache miss) and should never take more than a
//! few hundred milliseconds.
//!
//! ## Timeout rationale (TERROIR)
//!
//! - `terroir_eudr` gets 60 s because Hansen GFC polygon intersection on a
//!   cold cache can take 40-50 s (tile fetch + raster scan).
//! - `terroir_ussd` gets 5 s — USSD sessions are interactive; the provider
//!   drops the session after ~7 s so the service must respond well within that.
//! - `terroir_core` uses 10 s for standard CRUD; bulk household CRDT merge
//!   pushes toward that budget.
//! - `terroir_buyer` uses 30 s to accommodate Vault PKI DDS signing on download.
//!
//! ## Upstream cluster endpoints
//!
//! | Cluster name | Target | Port | Protocol |
//! |---|---|---|---|
//! | `bff_admin` | poulets-platform/bff | 4800 | HTTP/1.1 |
//! | `auth_ms` | auth-ms | 8801 | HTTP/1.1 |
//! | `auth_ms_ws` | auth-ms | 8801 | HTTP/1.1 → WS upgrade |
//! | `terroir_core` | terroir-core | 8830 | HTTP/1.1 |
//! | `terroir_eudr` | terroir-eudr | 8831 | HTTP/1.1 |
//! | `terroir_payment` | terroir-payment | 8832 | HTTP/1.1 |
//! | `terroir_mobile_bff` | terroir-mobile-bff | 8833 | HTTP/1.1 + WS upgrade |
//! | `terroir_ussd` | terroir-ussd | 8834 | HTTP/1.1 |
//! | `terroir_buyer` | terroir-buyer | 8835 | HTTP/1.1 |
//!
//! All clusters must be declared in `config/armageddon.yaml` under
//! `gateway.clusters`.  This module only declares the **routing** rules;
//! upstream endpoint registration happens in the main binary's bootstrap.
//!
//! ## Wave 2 integration points
//!
//! The six terroir clusters are consumed by the following Rust services that
//! must expose their HTTP listener on the port documented above:
//!
//! - `terroir-core` (P1) — Axum :8830, gRPC :8730
//! - `terroir-eudr` (P1) — Axum :8831, gRPC :8731
//! - `terroir-payment` (P2) — Spring Boot :8832 (via mobile-money-lib refactor)
//! - `terroir-mobile-bff` (P1) — Axum :8833, WebSocket on same port
//! - `terroir-ussd` (P3) — Axum :8834 (simulator on loopback during P0-P2)
//! - `terroir-buyer` (P3) — Axum :8835

use std::collections::HashMap;

use armageddon_forge::pingora::filters::router::RouteTable;

// ── ClusterConfig ─────────────────────────────────────────────────────────────

/// Upstream cluster configuration returned by per-cluster builder functions.
///
/// The caller (gateway bootstrap) uses this to register the cluster endpoint
/// in the ARMAGEDDON upstream registry under the given `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterConfig {
    /// Logical cluster name, e.g. `"terroir_core"`.
    pub name: &'static str,
    /// Upstream hostname (resolved by ARMAGEDDON DNS resolver at startup).
    pub host: &'static str,
    /// Upstream TCP port.
    pub port: u16,
    /// Timeout in milliseconds for this cluster.
    pub timeout_ms: u64,
}

// ── admin route table builder ─────────────────────────────────────────────────

/// Build the `RouteTable` entries required for the admin stream (D1).
///
/// Merge the returned `RouteTable` into the gateway's existing `RouteTable`
/// (e.g. via `RouteTable::merge` or by constructing the full table with these
/// entries included).
///
/// # Arguments
///
/// * `default_cluster` — the fallback cluster used when no admin rule matches.
///   Pass the value from the existing gateway config.
///
/// # WebSocket route
///
/// `/ws/admin/approval` is registered as an exact-match route to cluster
/// `auth_ms_ws`.  The `WebSocketProxyFilter` intercepts the upgrade request
/// **before** this routing step and populates `ctx.cluster = "auth_ms_ws"`,
/// so the route entry here serves as a belt-and-suspenders guard in case the
/// filter chain order is misconfigured.
pub fn build_admin_route_table(default_cluster: impl Into<String>) -> RouteTable {
    let mut exact: HashMap<String, String> = HashMap::new();

    // JWKS endpoint — served by auth-ms.
    exact.insert("/.well-known/jwks.json".to_string(), "auth_ms".to_string());

    // WS push-approval endpoint — proxied to auth-ms WS cluster.
    exact.insert("/ws/admin/approval".to_string(), "auth_ms_ws".to_string());

    // All admin API routes → bff_admin.
    // Expressed as a prefix so sub-paths (users, sessions, etc.) are covered.
    let prefix = vec![("/api/admin/".to_string(), "bff_admin".to_string())];

    RouteTable::new(exact, prefix, vec![], default_cluster)
}

/// Returns the cluster name used for the WebSocket push-approval upstream.
///
/// Register this cluster in `config/armageddon.yaml` pointing at
/// `auth-ms:8801`.  It must support HTTP/1.1 WebSocket upgrades.
pub fn ws_approval_cluster() -> &'static str {
    "auth_ms_ws"
}

/// Upstream timeout overrides for admin clusters.
///
/// The gateway's `upstream_timeout_ms` field in `PingoraGatewayConfig` is a
/// single global value.  To honour the per-route 30 s admin timeout, callers
/// should read this map and apply it when building the upstream peer for admin
/// routes, overriding the global default.
///
/// Returns `(cluster_name, timeout_ms)` pairs.
pub fn admin_timeout_overrides() -> Vec<(&'static str, u64)> {
    vec![
        ("bff_admin", 30_000), // 30 seconds — CSV export, recovery codes, etc.
        ("auth_ms", 5_000),    // 5 seconds  — JWKS fetch must be fast.
    ]
}

// ── TERROIR route table builder ───────────────────────────────────────────────

/// Build the `RouteTable` entries required for the TERROIR stream (P0.H).
///
/// Merge the returned table into the gateway's global `RouteTable` alongside
/// the admin table.  The TERROIR prefix `/api/terroir/` is distinct from
/// `/api/admin/` so there is no overlap.
///
/// # Arguments
///
/// * `default_cluster` — fallback cluster for unmatched paths.
///
/// # WebSocket route
///
/// `/ws/terroir/sync` is an exact-match route targeting `terroir_mobile_bff`.
/// Mobile agents use this endpoint to push CRDT delta-encoded updates while
/// online.  JWT validation and KAYA rate-limiting must precede this route in
/// the filter chain (same pattern as `/ws/admin/approval`).
pub fn build_terroir_route_table(default_cluster: impl Into<String>) -> RouteTable {
    let mut exact: HashMap<String, String> = HashMap::new();

    // CRDT delta-sync WS endpoint — mobile BFF handles the upgrade.
    exact.insert(
        "/ws/terroir/sync".to_string(),
        "terroir_mobile_bff".to_string(),
    );

    // Prefix routes — ordered longest-first within the same tier; RouteTable
    // picks the first matching prefix so more-specific prefixes must come first.
    let prefix = vec![
        // terroir-core :8830 — producers, parcels, households, cooperatives
        ("/api/terroir/core/".to_string(), "terroir_core".to_string()),
        // terroir-eudr :8831 — EUDR validation, Hansen GFC, DDS management
        ("/api/terroir/eudr/".to_string(), "terroir_eudr".to_string()),
        // terroir-payment :8832 — mobile money orchestration
        (
            "/api/terroir/payment/".to_string(),
            "terroir_payment".to_string(),
        ),
        // terroir-mobile-bff :8833 — agent terrain BFF (REST + WS)
        (
            "/api/terroir/mobile-bff/".to_string(),
            "terroir_mobile_bff".to_string(),
        ),
        // terroir-ussd :8834 — USSD/SMS gateway (simulator in P0-P2)
        ("/api/terroir/ussd/".to_string(), "terroir_ussd".to_string()),
        // terroir-buyer :8835 — buyer portal, DDS download + Vault PKI signing
        (
            "/api/terroir/buyer/".to_string(),
            "terroir_buyer".to_string(),
        ),
    ];

    RouteTable::new(exact, prefix, vec![], default_cluster)
}

/// Upstream timeout overrides for all TERROIR clusters.
///
/// Same contract as [`admin_timeout_overrides`]: callers apply these per-cluster
/// overrides when building upstream peers, superseding the global gateway default.
///
/// # Timeout rationale
///
/// | Cluster | ms | Reason |
/// |---|---|---|
/// | `terroir_core` | 10 000 | Standard CRUD; CRDT merge can take ~5 s on large households |
/// | `terroir_eudr` | 60 000 | Hansen GFC cold-cache tile fetch + polygon intersection |
/// | `terroir_payment` | 30 000 | Mobile-money provider round-trip (Hub2/AT SLA) |
/// | `terroir_mobile_bff` | 15 000 | Batch sync with CRDT merge; larger than core because payload can be multiple entities |
/// | `terroir_ussd` | 5 000 | USSD session is interactive; provider drops after ~7 s |
/// | `terroir_buyer` | 30 000 | Vault PKI DDS signing on download |
pub fn terroir_timeout_overrides() -> Vec<(&'static str, u64)> {
    vec![
        ("terroir_core", 10_000),
        ("terroir_eudr", 60_000),
        ("terroir_payment", 30_000),
        ("terroir_mobile_bff", 15_000),
        ("terroir_ussd", 5_000),
        ("terroir_buyer", 30_000),
    ]
}

// ── TERROIR cluster builders ──────────────────────────────────────────────────

/// Cluster config for `terroir-core` (producteurs, parcelles, household).
///
/// HTTP on port 8830, gRPC on 8730 (gRPC not managed by this gateway).
/// Timeout: 10 s (standard CRUD + CRDT merge budget).
pub fn terroir_core_cluster() -> ClusterConfig {
    ClusterConfig {
        name: "terroir_core",
        host: "terroir-core",
        port: 8830,
        timeout_ms: 10_000,
    }
}

/// Cluster config for `terroir-eudr` (validation EUDR + DDS management).
///
/// HTTP on port 8831, gRPC on 8731.
/// Timeout: 60 s — Hansen GFC cold-cache polygon intersection can reach 40-50 s.
pub fn terroir_eudr_cluster() -> ClusterConfig {
    ClusterConfig {
        name: "terroir_eudr",
        host: "terroir-eudr",
        port: 8831,
        timeout_ms: 60_000,
    }
}

/// Cluster config for `terroir-payment` (mobile money orchestration).
///
/// HTTP on port 8832 (Spring Boot).  No gRPC.
/// Timeout: 30 s — provider SLA for Hub2/AT acknowledgement.
pub fn terroir_payment_cluster() -> ClusterConfig {
    ClusterConfig {
        name: "terroir_payment",
        host: "terroir-payment",
        port: 8832,
        timeout_ms: 30_000,
    }
}

/// Cluster config for `terroir-mobile-bff` (BFF agent terrain + CRDT sync WS).
///
/// HTTP + WS upgrade on port 8833.
/// Timeout: 15 s — batch sync payload with multiple CRDT entities.
pub fn terroir_mobile_bff_cluster() -> ClusterConfig {
    ClusterConfig {
        name: "terroir_mobile_bff",
        host: "terroir-mobile-bff",
        port: 8833,
        timeout_ms: 15_000,
    }
}

/// Cluster config for `terroir-ussd` (USSD/SMS gateway).
///
/// HTTP on port 8834.  During P0-P2 this points at `terroir-ussd-simulator`
/// on loopback; the host is overridden by the runtime config seeded from
/// `terroir.ussd.simulator_enabled` in the settings cache.
/// Timeout: 5 s — hard USSD session boundary.
pub fn terroir_ussd_cluster() -> ClusterConfig {
    ClusterConfig {
        name: "terroir_ussd",
        host: "terroir-ussd",
        port: 8834,
        timeout_ms: 5_000,
    }
}

/// Cluster config for `terroir-buyer` (buyer portal, DDS download + signing).
///
/// HTTP on port 8835.
/// Timeout: 30 s — Vault PKI DDS signing on download adds ~2-5 s.
pub fn terroir_buyer_cluster() -> ClusterConfig {
    ClusterConfig {
        name: "terroir_buyer",
        host: "terroir-buyer",
        port: 8835,
        timeout_ms: 30_000,
    }
}

/// All TERROIR cluster configs as a `Vec`, in port order.
///
/// Convenience for gateway bootstrap code that must register all clusters
/// in a single loop.
pub fn all_terroir_clusters() -> Vec<ClusterConfig> {
    vec![
        terroir_core_cluster(),
        terroir_eudr_cluster(),
        terroir_payment_cluster(),
        terroir_mobile_bff_cluster(),
        terroir_ussd_cluster(),
        terroir_buyer_cluster(),
    ]
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── admin route tests ─────────────────────────────────────────────────────

    #[test]
    fn jwks_route_resolves_to_auth_ms() {
        let table = build_admin_route_table("default");
        assert_eq!(table.resolve("/.well-known/jwks.json"), "auth_ms");
    }

    #[test]
    fn admin_prefix_resolves_to_bff_admin() {
        let table = build_admin_route_table("default");
        assert_eq!(table.resolve("/api/admin/users"), "bff_admin");
        assert_eq!(table.resolve("/api/admin/settings"), "bff_admin");
        assert_eq!(table.resolve("/api/admin/otp/issue"), "bff_admin");
        assert_eq!(table.resolve("/api/admin/audit"), "bff_admin");
        assert_eq!(
            table.resolve("/api/admin/break-glass/activate"),
            "bff_admin"
        );
    }

    #[test]
    fn non_admin_path_falls_through_to_default() {
        let table = build_admin_route_table("default");
        assert_eq!(table.resolve("/api/public/health"), "default");
        assert_eq!(table.resolve("/api/poulets/feed"), "default");
    }

    #[test]
    fn timeout_overrides_include_bff_admin() {
        let overrides = admin_timeout_overrides();
        let bff = overrides.iter().find(|(c, _)| *c == "bff_admin");
        assert!(bff.is_some(), "bff_admin must have a timeout override");
        assert_eq!(bff.unwrap().1, 30_000);
    }

    #[test]
    fn timeout_overrides_include_auth_ms() {
        let overrides = admin_timeout_overrides();
        let auth = overrides.iter().find(|(c, _)| *c == "auth_ms");
        assert!(auth.is_some(), "auth_ms must have a timeout override");
        assert_eq!(auth.unwrap().1, 5_000);
    }

    #[test]
    fn ws_approval_route_resolves_to_auth_ms_ws() {
        let table = build_admin_route_table("default");
        assert_eq!(table.resolve("/ws/admin/approval"), "auth_ms_ws");
    }

    #[test]
    fn ws_approval_cluster_constant() {
        assert_eq!(ws_approval_cluster(), "auth_ms_ws");
    }

    // ── TERROIR route table tests ─────────────────────────────────────────────

    #[test]
    fn terroir_core_prefix_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(
            table.resolve("/api/terroir/core/tenants/coop-burkina/producers"),
            "terroir_core"
        );
        assert_eq!(
            table.resolve("/api/terroir/core/cooperatives/42/parcels"),
            "terroir_core"
        );
        assert_eq!(
            table.resolve("/api/terroir/core/parcels/7/polygon"),
            "terroir_core"
        );
        assert_eq!(
            table.resolve("/api/terroir/core/tenants/slug/household"),
            "terroir_core"
        );
    }

    #[test]
    fn terroir_eudr_prefix_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(
            table.resolve("/api/terroir/eudr/parcels/7/validate"),
            "terroir_eudr"
        );
        assert_eq!(
            table.resolve("/api/terroir/eudr/dds/12/sign"),
            "terroir_eudr"
        );
    }

    #[test]
    fn terroir_payment_prefix_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(
            table.resolve("/api/terroir/payment/orders"),
            "terroir_payment"
        );
        assert_eq!(
            table.resolve("/api/terroir/payment/orders/99/status"),
            "terroir_payment"
        );
    }

    #[test]
    fn terroir_mobile_bff_prefix_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(
            table.resolve("/api/terroir/mobile-bff/sync/batch"),
            "terroir_mobile_bff"
        );
        assert_eq!(
            table.resolve("/api/terroir/mobile-bff/entities/parcel"),
            "terroir_mobile_bff"
        );
    }

    #[test]
    fn terroir_ussd_prefix_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(table.resolve("/api/terroir/ussd/session"), "terroir_ussd");
        assert_eq!(
            table.resolve("/api/terroir/ussd/otp/verify"),
            "terroir_ussd"
        );
    }

    #[test]
    fn terroir_buyer_prefix_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(
            table.resolve("/api/terroir/buyer/lots/33/contract"),
            "terroir_buyer"
        );
        assert_eq!(
            table.resolve("/api/terroir/buyer/dds/download/abc"),
            "terroir_buyer"
        );
    }

    #[test]
    fn terroir_ws_sync_exact_match_resolves() {
        let table = build_terroir_route_table("default");
        assert_eq!(table.resolve("/ws/terroir/sync"), "terroir_mobile_bff");
    }

    #[test]
    fn terroir_non_terroir_path_falls_through_to_default() {
        let table = build_terroir_route_table("default");
        assert_eq!(table.resolve("/api/admin/users"), "default");
        assert_eq!(table.resolve("/api/public/health"), "default");
        assert_eq!(table.resolve("/api/poulets/feed"), "default");
    }

    #[test]
    fn terroir_timeout_overrides_all_clusters_present() {
        let overrides = terroir_timeout_overrides();
        let cluster_names: Vec<&str> = overrides.iter().map(|(c, _)| *c).collect();
        for expected in &[
            "terroir_core",
            "terroir_eudr",
            "terroir_payment",
            "terroir_mobile_bff",
            "terroir_ussd",
            "terroir_buyer",
        ] {
            assert!(
                cluster_names.contains(expected),
                "missing timeout override for cluster: {expected}"
            );
        }
    }

    #[test]
    fn terroir_timeout_overrides_values_match_spec() {
        let overrides = terroir_timeout_overrides();
        let find = |name: &str| -> u64 {
            overrides
                .iter()
                .find(|(c, _)| *c == name)
                .map(|(_, t)| *t)
                .unwrap_or(0)
        };
        assert_eq!(find("terroir_core"), 10_000);
        assert_eq!(find("terroir_eudr"), 60_000);
        assert_eq!(find("terroir_payment"), 30_000);
        assert_eq!(find("terroir_mobile_bff"), 15_000);
        assert_eq!(find("terroir_ussd"), 5_000);
        assert_eq!(find("terroir_buyer"), 30_000);
    }

    // ── cluster config builder tests ──────────────────────────────────────────

    #[test]
    fn cluster_builders_return_correct_ports() {
        assert_eq!(terroir_core_cluster().port, 8830);
        assert_eq!(terroir_eudr_cluster().port, 8831);
        assert_eq!(terroir_payment_cluster().port, 8832);
        assert_eq!(terroir_mobile_bff_cluster().port, 8833);
        assert_eq!(terroir_ussd_cluster().port, 8834);
        assert_eq!(terroir_buyer_cluster().port, 8835);
    }

    #[test]
    fn cluster_builders_return_correct_names() {
        assert_eq!(terroir_core_cluster().name, "terroir_core");
        assert_eq!(terroir_eudr_cluster().name, "terroir_eudr");
        assert_eq!(terroir_payment_cluster().name, "terroir_payment");
        assert_eq!(terroir_mobile_bff_cluster().name, "terroir_mobile_bff");
        assert_eq!(terroir_ussd_cluster().name, "terroir_ussd");
        assert_eq!(terroir_buyer_cluster().name, "terroir_buyer");
    }

    #[test]
    fn cluster_builders_timeout_matches_overrides() {
        // Both sources must agree so there is no discrepancy at bootstrap.
        let overrides = terroir_timeout_overrides();
        let find_override = |name: &str| -> u64 {
            overrides
                .iter()
                .find(|(c, _)| *c == name)
                .map(|(_, t)| *t)
                .unwrap_or(0)
        };
        for cfg in all_terroir_clusters() {
            assert_eq!(
                cfg.timeout_ms,
                find_override(cfg.name),
                "timeout_ms mismatch for cluster {}",
                cfg.name
            );
        }
    }

    #[test]
    fn all_terroir_clusters_has_six_entries() {
        assert_eq!(all_terroir_clusters().len(), 6);
    }
}
