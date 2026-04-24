// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # armageddon-admin-api
//!
//! Envoy-style admin API for ARMAGEDDON on a dedicated loopback port
//! (default `127.0.0.1:9903`, admin-api range per port-policy.yaml). Exposes introspection endpoints:
//!
//! | Method | Path            | Purpose                                               |
//! |--------|-----------------|-------------------------------------------------------|
//! | GET    | `/stats`        | Prometheus exposition (or JSON via `?format=json`)   |
//! | GET    | `/clusters`     | Upstream clusters + endpoint health + breaker state  |
//! | GET    | `/config_dump`  | JSON/YAML dump of the loaded configuration           |
//! | GET    | `/runtime`      | Runtime feature-flag values                          |
//! | GET    | `/server_info`  | Version, build SHA, hostname, uptime                 |
//! | GET    | `/listeners`    | Active listeners (port, protocol, TLS)               |
//! | GET    | `/health`       | Aggregated health                                    |
//! | POST   | `/logging`      | Dynamic log-level change (`{"level":"debug"}`)      |
//!
//! ## Security
//!
//! - Bound on loopback by default. Binding on non-loopback REQUIRES a
//!   bearer token sourced from an env var (default `ARMAGEDDON_ADMIN_TOKEN`).
//! - Token comparison is constant-time (`subtle`).
//! - When bound on loopback without a token, a warning is logged at startup.
//!
//! ## Wiring
//!
//! The crate does NOT depend on other ARMAGEDDON engines directly. Real
//! data is fed through the provider traits in [`providers`]. Default
//! `Null*` impls are supplied so this crate is usable stand-alone.

pub mod auth;
pub mod metrics;
pub mod providers;
pub mod routes;
pub mod state;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::io;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::auth::AuthState;
use crate::metrics::track_request;
use crate::providers::{
    ClusterProvider, ConfigDumper, HealthProvider, NullClusterProvider, NullConfigDumper,
    NullHealthProvider, NullRuntimeProvider, NullShadowProvider, NullStatsProvider,
    RuntimeProvider, ShadowProvider, StatsProvider,
};
use crate::state::{AdminApiState, ServerInfo};

pub use armageddon_common::types::AdminApiConfig;

// Re-exports for downstream crates.
pub use state::AdminApiState as State;

/// Error type for admin-api startup failures.
#[derive(Debug, thiserror::Error)]
pub enum AdminApiError {
    #[error("admin API disabled via config")]
    Disabled,
    #[error("invalid bind address '{addr}': {source}")]
    InvalidBindAddr {
        addr: String,
        #[source]
        source: std::net::AddrParseError,
    },
    #[error(
        "admin API must be authenticated when binding on a non-loopback address \
         ({addr}); set env var {env}"
    )]
    MissingTokenForPublicBind { addr: String, env: String },
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Top-level admin API runtime.
pub struct AdminApi {
    cfg: AdminApiConfig,
    state: Arc<AdminApiState>,
    auth: Arc<AuthState>,
    bind: std::net::SocketAddr,
}

impl std::fmt::Debug for AdminApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminApi")
            .field("bind", &self.bind)
            .field("enabled", &self.cfg.enabled)
            .field("auth_enabled", &self.auth.enabled())
            .finish()
    }
}

impl AdminApi {
    /// Build an `AdminApi` from a config and a bundle of providers.
    ///
    /// Returns `Err(AdminApiError::Disabled)` when `cfg.enabled == false`.
    /// Returns `Err(AdminApiError::MissingTokenForPublicBind)` when the
    /// bind address is non-loopback and no bearer token is present in the
    /// configured env var.
    ///
    /// The shadow provider defaults to `NullShadowProvider`. Use
    /// [`AdminApi::build_with_shadow`] to wire in a live provider.
    pub fn build(
        cfg: AdminApiConfig,
        stats: Arc<dyn StatsProvider>,
        clusters: Arc<dyn ClusterProvider>,
        config: Arc<dyn ConfigDumper>,
        runtime: Arc<dyn RuntimeProvider>,
        health: Arc<dyn HealthProvider>,
    ) -> Result<Self, AdminApiError> {
        Self::build_with_shadow(
            cfg,
            stats,
            clusters,
            config,
            runtime,
            health,
            Arc::new(NullShadowProvider),
        )
    }

    /// Build an `AdminApi` with a live shadow-mode provider.
    ///
    /// All other behaviour is identical to [`AdminApi::build`].
    pub fn build_with_shadow(
        cfg: AdminApiConfig,
        stats: Arc<dyn StatsProvider>,
        clusters: Arc<dyn ClusterProvider>,
        config: Arc<dyn ConfigDumper>,
        runtime: Arc<dyn RuntimeProvider>,
        health: Arc<dyn HealthProvider>,
        shadow: Arc<dyn ShadowProvider>,
    ) -> Result<Self, AdminApiError> {
        if !cfg.enabled {
            return Err(AdminApiError::Disabled);
        }

        let bind: std::net::SocketAddr =
            cfg.bind_addr
                .parse()
                .map_err(|source| AdminApiError::InvalidBindAddr {
                    addr: cfg.bind_addr.clone(),
                    source,
                })?;

        let token = std::env::var(&cfg.token_env_var).ok().filter(|s| !s.is_empty());

        let auth = match (bind.ip().is_loopback(), token.as_deref()) {
            (_, Some(t)) => Arc::new(AuthState::with_token(t)),
            (true, None) => {
                tracing::warn!(
                    addr = %bind,
                    env = %cfg.token_env_var,
                    "admin-api: no bearer token configured — \
                     permitted only because bind is loopback"
                );
                Arc::new(AuthState::disabled())
            }
            (false, None) => {
                return Err(AdminApiError::MissingTokenForPublicBind {
                    addr: bind.to_string(),
                    env: cfg.token_env_var.clone(),
                });
            }
        };

        let server_info = ServerInfo::from_env();
        let state = crate::state::AdminApiState::new_with_shadow(
            stats,
            clusters,
            config,
            runtime,
            health,
            shadow,
            server_info,
            default_log_level(),
        );

        Ok(Self {
            cfg,
            state,
            auth,
            bind,
        })
    }

    /// Build an AdminApi with all-null providers. Handy for tests and
    /// stand-alone verification of the crate's routing.
    pub fn build_with_nulls(cfg: AdminApiConfig) -> Result<Self, AdminApiError> {
        Self::build(
            cfg,
            Arc::new(NullStatsProvider),
            Arc::new(NullClusterProvider),
            Arc::new(NullConfigDumper),
            Arc::new(NullRuntimeProvider),
            Arc::new(NullHealthProvider),
        )
    }

    /// Socket address the API will bind to.
    pub fn bind_addr(&self) -> std::net::SocketAddr {
        self.bind
    }

    /// Build the axum router (public for test drivers).
    pub fn router(&self) -> Router {
        build_router(Arc::clone(&self.state), Arc::clone(&self.auth), &self.cfg)
    }

    /// Run the server until `shutdown` fires.
    pub async fn run(self, mut shutdown: broadcast::Receiver<()>) -> Result<(), AdminApiError> {
        let listener = TcpListener::bind(self.bind).await?;
        tracing::info!(addr = %self.bind, "admin-api: listening");
        let router = self.router();
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown.recv().await;
                tracing::info!("admin-api: shutting down");
            })
            .await?;
        Ok(())
    }
}

fn default_log_level() -> &'static str {
    "info"
}

/// Build the admin API router. Public so tests and integrations can drive
/// it via `tower::ServiceExt::oneshot`.
///
/// ## CORS
///
/// The admin API is NOT browser-driven: it is consumed by `curl`, Prometheus
/// scrapers and SDK clients that do not enforce CORS. We deliberately do NOT
/// install any CORS layer here — no `Access-Control-Allow-Origin` header is
/// ever emitted. This prevents a malicious website (visited by an operator
/// whose workstation has the admin API bound on loopback without a token)
/// from reading `/config_dump`, `/clusters`, `/server_info`, or mutating
/// `/logging` via a simple `fetch()` call.
///
/// The `cors_allowed_origins` field on [`AdminApiConfig`] is therefore
/// intentionally ignored here and kept only for YAML backwards-compatibility.
pub fn build_router(
    state: Arc<AdminApiState>,
    auth: Arc<AuthState>,
    _cfg: &AdminApiConfig,
) -> Router {
    Router::new()
        .route("/stats", get(routes::get_stats))
        .route("/clusters", get(routes::get_clusters))
        .route("/config_dump", get(routes::get_config_dump))
        .route("/runtime", get(routes::get_runtime))
        .route("/server_info", get(routes::get_server_info))
        .route("/listeners", get(routes::get_listeners))
        .route("/health", get(routes::get_health))
        .route("/logging", post(routes::post_logging))
        // Shadow mode ramp-up control endpoints.
        .route("/admin/shadow/rate", post(routes::post_shadow_rate))
        .route("/admin/shadow/state", get(routes::get_shadow_state))
        .route("/admin/shadow/gate", post(routes::post_shadow_gate))
        .fallback(routes::not_found)
        // Track requests for Prometheus.
        .layer(middleware::from_fn(track_request))
        // Bearer-token auth (always present; becomes a no-op when
        // `auth.enabled() == false`).
        .layer(middleware::from_fn_with_state(
            Arc::clone(&auth),
            auth::bearer_auth,
        ))
        .with_state(state)
}
