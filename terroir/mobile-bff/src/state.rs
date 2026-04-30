// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared application state for terroir-mobile-bff.
//!
//! Injected into every Axum handler via `State<Arc<AppState>>`.

use std::sync::Arc;

use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::{grpc_client::CoreGrpcPool, tenant_context::JwksCache, ws::registry::WsRegistry};

/// Shared state for all REST + WebSocket handlers.
pub struct AppState {
    /// Read-replica PostgreSQL pool (compact reads for `/m/producers`, `/m/parcels`).
    /// Writes always go through the gRPC client to terroir-core.
    pub pg: Arc<PgPool>,
    /// KAYA RESP3 connection manager (idempotency batch + per-userId rate-limit).
    pub kaya: ConnectionManager,
    /// JWKS cache for JWT validation (auth-ms :8801).
    pub jwks_cache: Arc<JwksCache>,
    /// Shared HTTP client (used for JWKS fetches).
    pub http_client: reqwest::Client,
    /// gRPC client pool to terroir-core :8730.
    pub core_grpc: CoreGrpcPool,
    /// WebSocket registry — connected clients per `(tenant, userId)`.
    pub ws_registry: Arc<WsRegistry>,
}
