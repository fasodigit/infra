// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared application state injected into every Axum handler via `State<Arc<AppState>>`.

use std::sync::Arc;

#[allow(unused_imports)]
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::service::vault_transit::VaultTransitService;
use crate::tenant_context::JwksCache;

#[cfg(feature = "kafka")]
use crate::events::EventProducer;

/// Shared state for all handlers.
pub struct AppState {
    /// PostgreSQL connection pool (pgbouncer transaction mode, no prepared stmts).
    pub pg: Arc<PgPool>,
    /// KAYA RESP3 connection manager (DEK cache + idempotency keys).
    /// `ConnectionManager` is cheaply cloneable (wraps an Arc internally).
    pub kaya: ConnectionManager,
    /// Vault Transit client for PII encryption/decryption.
    pub vault: Arc<VaultTransitService>,
    /// JWKS cache for JWT validation (auth-ms :8801).
    pub jwks_cache: Arc<JwksCache>,
    /// Shared HTTP client (used for Vault Transit + JWKS fetches).
    pub http_client: reqwest::Client,
    /// Redpanda event producer (optional if kafka feature is disabled).
    #[cfg(feature = "kafka")]
    pub events: Arc<EventProducer>,
}
