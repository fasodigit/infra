// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-admin — Axum router.
//
// Endpoints:
//   GET  /health/ready
//   GET  /health/live
//   POST /admin/tenants              create + provision tenant
//   GET  /admin/tenants              list (keyset paginated)
//   GET  /admin/tenants/:slug        get one tenant
//   POST /admin/tenants/:slug/suspend suspend tenant
//
// Security: loopback :9904 (enforced at bind level in main.rs).
// No auth in P0 — admin trusted network. Keto ABAC check added in P1.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response as AxumResponse},
    routing::{get, post},
};
use sqlx::PgPool;
use tracing::{error, info};

use crate::dto::{CreateTenantRequest, ErrorResponse, ListTenantsQuery};
use crate::tenant_service;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<PgPool>,
    pub template_dir: Arc<PathBuf>,
}

// ---------------------------------------------------------------------------
// Router factory
// ---------------------------------------------------------------------------

pub fn build_router(state: AppState) -> Router {
    // Health routes have no state dependency.
    let health_router: Router<AppState> = Router::new()
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live));

    // Admin routes all require AppState.
    let admin_router: Router<AppState> = Router::new()
        .route("/admin/tenants", post(create_tenant).get(list_tenants))
        .route("/admin/tenants/{slug}", get(get_tenant))
        .route("/admin/tenants/{slug}/suspend", post(suspend_tenant));

    Router::new()
        .merge(health_router)
        .merge(admin_router)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn health_ready() -> &'static str {
    "ready"
}

async fn health_live() -> &'static str {
    "live"
}

// ---------------------------------------------------------------------------
// POST /admin/tenants
// ---------------------------------------------------------------------------

async fn create_tenant(
    State(state): State<AppState>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    // Dispatch to inner async fn to work around Rust 2024 Send capture issue.
    // The inner function signature uses owned Arc args, ensuring the future is Send.
    create_tenant_inner(state, req).await
}

async fn create_tenant_inner(state: AppState, req: CreateTenantRequest) -> AxumResponse {
    // Early validation before touching Postgres.
    if let Err(msg) = req.validate() {
        let body = Json(ErrorResponse::new("validation_error", msg));
        return (StatusCode::UNPROCESSABLE_ENTITY, body).into_response();
    }

    info!(slug = %req.slug, "POST /admin/tenants — provisioning");

    let pool = Arc::clone(&state.pool);
    let template_dir = Arc::clone(&state.template_dir);
    let slug = req.slug.clone();

    let result = tenant_service::provision_tenant(pool, template_dir, req).await;

    match result {
        Ok(tenant) => (StatusCode::CREATED, Json(tenant)).into_response(),
        Err(e) => {
            // anyhow::Error's Display only shows the top-level context. The
            // sqlx error (with `code=23505 unique_violation`) is in the
            // source chain — use {:#} to flatten the chain into one line.
            let msg = format!("{:#}", e);
            if msg.contains("duplicate key")
                || msg.contains("unique constraint")
                || msg.contains("23505")
                || msg.contains("uq_")
                || msg.contains("already exists")
            {
                let body = Json(ErrorResponse::new(
                    "already_exists",
                    format!("tenant '{}' already exists", slug),
                ));
                return (StatusCode::CONFLICT, body).into_response();
            }
            error!(slug = %slug, error = %msg, "provision_tenant failed");
            let body = Json(ErrorResponse::new(
                "provisioning_error",
                "tenant provisioning failed — check logs",
            ));
            (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// GET /admin/tenants
// ---------------------------------------------------------------------------

async fn list_tenants(
    State(state): State<AppState>,
    Query(params): Query<ListTenantsQuery>,
) -> impl IntoResponse {
    match tenant_service::list_tenants(&state.pool, params.limit, params.cursor.as_deref()).await {
        Ok(list) => (StatusCode::OK, Json(list)).into_response(),
        Err(e) => {
            error!(error = %e, "list_tenants failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("list_error", "failed to list tenants")),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// GET /admin/tenants/:slug
// ---------------------------------------------------------------------------

async fn get_tenant(State(state): State<AppState>, Path(slug): Path<String>) -> impl IntoResponse {
    match tenant_service::get_tenant(&state.pool, &slug).await {
        Ok(Some(tenant)) => (StatusCode::OK, Json(tenant)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "not_found",
                format!("tenant '{}' not found", slug),
            )),
        )
            .into_response(),
        Err(e) => {
            error!(slug = %slug, error = %e, "get_tenant failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("get_error", "failed to fetch tenant")),
            )
                .into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// POST /admin/tenants/:slug/suspend
// ---------------------------------------------------------------------------

async fn suspend_tenant(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match tenant_service::suspend_tenant(&state.pool, &slug).await {
        Ok(Some(tenant)) => (StatusCode::OK, Json(tenant)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "not_found",
                format!("tenant '{}' not found", slug),
            )),
        )
            .into_response(),
        Err(e) => {
            error!(slug = %slug, error = %e, "suspend_tenant failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "suspend_error",
                    "failed to suspend tenant",
                )),
            )
                .into_response()
        }
    }
}
