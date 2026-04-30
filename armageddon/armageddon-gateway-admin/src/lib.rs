// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # armageddon-gateway-admin — Stream D1 (admin) + P0.H (TERROIR) gateway module
//!
//! Implements the **ARMAGEDDON gateway** layer for:
//!
//! - **Phase 4.b** FASO admin-UI (Stream D1)
//! - **Phase TERROIR P0.H** — six terroir microservices + CRDT sync WebSocket
//!
//! Reference documents:
//!
//! - `docs/GAP-ANALYSIS-PHASE-4A.md` §12 (ARMAGEDDON gateway)
//! - `docs/CLAUDE-DESIGN-PROMPT-ADMIN-UI.md` §2 (gateway :8080) and §7
//!   (Redpanda topic `admin.settings.changed`)
//! - `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §4 P0.9 and §2
//!   (service ports 8830-8835)
//!
//! ## Components
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`routes`] | Admin `RouteTable` + TERROIR `RouteTable`; cluster config builders |
//! | [`filters::keto_authz`] | Keto `AdminRole` check (admin) + namespace-aware check (TERROIR) |
//! | [`filters::security_headers`] | HSTS / X-Frame-Options / Cache-Control injection |
//! | [`filters::otp_rate_limit`] | Per-user 3 req/5 min limit on `POST /api/admin/otp/issue` |
//! | [`filters::access_log`] | Prometheus `armageddon_admin_requests_total` counter |
//! | [`filters::websocket_proxy`] | WS upgrade intercept on `/ws/admin/approval` and `/ws/terroir/sync` |
//! | [`settings_cache`] | `AdminSettingsCache` + consumers for `admin.settings.changed` and `terroir.settings.changed` |
//! | [`metrics`] | All Prometheus metrics for admin + TERROIR streams |
//!
//! ## Admin filter chain (Pingora order)
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │  Pingora request pipeline for /api/admin/* routes            │
//! │                                                              │
//! │  1. JWT filter (armageddon-forge)                            │
//! │     → populates ctx.user_id / ctx.trace_id / ctx.roles       │
//! │                                                              │
//! │  2. KetoAuthzFilter (this crate, AdminRole namespace)        │
//! │     → POST /relation-tuples/check/openapi to Keto :4466      │
//! │     → 403 on deny or Keto unreachable (fail-closed)          │
//! │                                                              │
//! │  3. OtpRateLimitFilter (this crate)                          │
//! │     → INCR armageddon:rl:otp:{userId} in KAYA                │
//! │     → 429 if count > settings[otp.rate_limit_per_user_5min]  │
//! │                                                              │
//! │  4. SecurityHeadersFilter (this crate)                       │
//! │     → on_response: inject HSTS / X-Frame / Cache-Control     │
//! │                                                              │
//! │  5. AdminAccessLogFilter (this crate)                        │
//! │     → on_logging: increment armageddon_admin_requests_total  │
//! │                                                              │
//! │  6. Upstream: bff_admin (poulets-platform/bff :4800)         │
//! │     Timeout: 30 s                                            │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## TERROIR filter chain (Pingora order)
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │  Pingora request pipeline for /api/terroir/* routes              │
//! │                                                                  │
//! │  1. JWT filter (armageddon-forge)                                │
//! │     → populates ctx.user_id / ctx.trace_id / ctx.roles           │
//! │     → 401 if token absent or expired                             │
//! │                                                                  │
//! │  2. KetoAuthzFilter (this crate, namespace-aware)                │
//! │     → resolves (namespace, object, relation) from path + method  │
//! │     → POST /relation-tuples/check/openapi to Keto :4466          │
//! │     → JWT-only for /ws/terroir/sync and buyer/lots/*/contract    │
//! │     → 403 on deny or Keto unreachable (fail-closed)              │
//! │                                                                  │
//! │  3. SecurityHeadersFilter (this crate)                           │
//! │     → on_response: inject HSTS / Cache-Control                   │
//! │                                                                  │
//! │  4. AdminAccessLogFilter (this crate)                            │
//! │     → on_logging: increment armageddon_terroir_requests_total    │
//! │     → on_logging: check X-Eudr-Cache-Status for EUDR paths       │
//! │                                                                  │
//! │  5. Upstream: terroir_{core,eudr,payment,mobile_bff,ussd,buyer}  │
//! │     Timeouts: 10 / 60 / 30 / 15 / 5 / 30 s respectively         │
//! └──────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Redpanda integration
//!
//! - `admin.settings.changed` → [`settings_cache::start_consumer`] → admin keys
//! - `terroir.settings.changed` → [`settings_cache::start_terroir_consumer`] → `terroir.*` keys
//!
//! Both consumers write into the same `AdminSettingsCache` under distinct key
//! prefixes, so the hot-path filters can always find the latest value.
//!
//! ## Failure modes (leader loss / quorum / network partition)
//!
//! Since this module does not participate in Raft, the relevant failure modes
//! concern external service dependencies:
//!
//! | Service | Failure | Behaviour |
//! |---------|---------|-----------|
//! | Keto :4466 | unreachable | Fail-closed: 403 `keto_unavailable` on all routes |
//! | KAYA :6380 | unreachable | OTP rate-limit fails open (logged warning) |
//! | Redpanda | unreachable | Settings cache retains last known values; consumers retry |
//! | BFF :4800 | unreachable | Pingora upstream selection + circuit breaker handles |
//! | terroir-eudr :8831 | unreachable | Pingora 502 surfaced to client; EUDR cache miss recorded |
//! | terroir-ussd :8834 | unreachable | 502; USSD provider drops session after 7 s anyway |

pub mod filters;
pub mod metrics;
pub mod routes;
pub mod settings_cache;

pub use filters::{
    AdminAccessLogFilter, KetoAuthzConfig, KetoAuthzFilter, OtpRateLimitFilter,
    SecurityHeadersFilter, WebSocketProxyFilter, WsProxyConfig,
};
pub use routes::{
    admin_timeout_overrides, all_terroir_clusters, build_admin_route_table,
    build_terroir_route_table, terroir_buyer_cluster, terroir_core_cluster, terroir_eudr_cluster,
    terroir_mobile_bff_cluster, terroir_payment_cluster, terroir_timeout_overrides,
    terroir_ussd_cluster, ws_approval_cluster, ClusterConfig,
};
#[cfg(feature = "rdkafka-consumer")]
pub use settings_cache::{start_consumer, start_terroir_consumer};
pub use settings_cache::{
    AdminSettingsCache, SettingsConsumerConfig, TerroirSettingsConsumerConfig,
};
