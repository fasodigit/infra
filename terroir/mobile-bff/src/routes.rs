// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axum router for terroir-mobile-bff.
//!
//! All `/m/*` REST handlers extract `TenantContext` (JWT-validated) and
//! enforce per-userId rate-limit (60 rpm via KAYA). Compression negotiation
//! (Brotli or gzip) is added globally as a Tower layer for mobile-friendly
//! payloads.
//!
//! WebSocket route `/ws/sync/{producerId}` handles its own auth via the
//! `Sec-WebSocket-Protocol: bearer.<jwt>` header (Axum extractors don't
//! cleanly compose with WebSocket upgrade — see `ws::handler`).

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use sqlx::Row;
use tower_http::compression::CompressionLayer;

use crate::{
    dto::{
        CompactParcel, CompactProducer, MobilePageResponse, MobilePaginationParams,
        SyncBatchRequest, SyncBatchResponse,
    },
    errors::BffError,
    service::{idempotency, rate_limit, sync_engine},
    state::AppState,
    tenant_context::TenantContext,
    ws::ws_sync_handler,
};

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the Axum `Router`.
///
/// REST surface mounted under `/m/*` — ARMAGEDDON proxies
/// `/api/terroir/mobile-bff/m/*` here after stripping the prefix.
/// WebSocket endpoint `/ws/sync/{producerId}` matches what ARMAGEDDON
/// forwards from `/ws/terroir/sync/{producerId}`.
pub fn build_router(state: Arc<AppState>) -> Router {
    let compression = CompressionLayer::new().br(true).gzip(true);

    Router::new()
        // Health
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        .route("/m/health", get(health_ready)) // mobile alias
        // Mobile-optimized REST
        .route("/m/producers", get(list_producers_compact))
        .route("/m/parcels", get(list_parcels_compact))
        .route("/m/sync/batch", post(post_sync_batch))
        // WebSocket
        .route("/ws/sync/{producer_id}", get(ws_sync_handler))
        .layer(compression)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

async fn health_ready() -> impl IntoResponse {
    (StatusCode::OK, "ready")
}

async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, "live")
}

// ---------------------------------------------------------------------------
// Compact list endpoints — direct PG read-replica
// ---------------------------------------------------------------------------

/// `GET /m/producers?cooperativeId=&page=&size=` — compact list.
async fn list_producers_compact(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Query(params): Query<MobilePaginationParams>,
) -> Result<impl IntoResponse, BffError> {
    rate_limit_check(&state, &tenant).await?;

    let size = params.clamped_size();
    let offset = params.page.saturating_mul(size) as i64;
    let limit = size as i64;
    let schema = tenant.schema_name();

    // Compact projection — no PII beyond `full_name_encrypted` (rendered as
    // a single placeholder for now; full-name decryption is delegated to
    // terroir-core REST when the user opens the detail screen).
    let mut tx = state.pg.begin().await?;
    sqlx::query(&format!("SET LOCAL search_path TO {schema}"))
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(anyhow::anyhow!("set search_path: {e}")))?;

    let rows = match params.cooperative_id {
        Some(coop_id) => {
            sqlx::query(
                r#"
            SELECT id, cooperative_id, primary_crop, updated_at, lww_version
            FROM producer
            WHERE cooperative_id = $1 AND deleted_at IS NULL
            ORDER BY updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
            )
            .bind(coop_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?
        }
        None => {
            sqlx::query(
                r#"
            SELECT id, cooperative_id, primary_crop, updated_at, lww_version
            FROM producer
            WHERE deleted_at IS NULL
            ORDER BY updated_at DESC
            LIMIT $1 OFFSET $2
            "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?
        }
    };
    tx.commit().await?;

    let items = rows
        .into_iter()
        .map(|r| CompactProducer {
            id: r.get("id"),
            cooperative_id: r.get("cooperative_id"),
            // Full name is decrypted only on detail endpoint. List shows a placeholder
            // to keep compact (mobile UI displays initials from `id` until detail loads).
            full_name: String::new(),
            primary_crop: r.try_get("primary_crop").ok(),
            updated_at: r.get("updated_at"),
            lww_version: r.get("lww_version"),
        })
        .collect();

    Ok(Json(MobilePageResponse {
        items,
        page: params.page,
        size,
    }))
}

/// `GET /m/parcels?producerId=&page=&size=` — compact list with WKT geometry.
async fn list_parcels_compact(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Query(params): Query<MobilePaginationParams>,
) -> Result<impl IntoResponse, BffError> {
    rate_limit_check(&state, &tenant).await?;

    let size = params.clamped_size();
    let offset = params.page.saturating_mul(size) as i64;
    let limit = size as i64;
    let schema = tenant.schema_name();

    let mut tx = state.pg.begin().await?;
    sqlx::query(&format!("SET LOCAL search_path TO {schema}"))
        .execute(&mut *tx)
        .await
        .map_err(|e| BffError::Internal(anyhow::anyhow!("set search_path: {e}")))?;

    let rows = match params.producer_id {
        Some(prod_id) => {
            sqlx::query(
                r#"
            SELECT p.id, p.producer_id, p.crop_type, p.surface_hectares,
                   p.updated_at, p.lww_version,
                   ST_AsText(pp.geom) AS geom_wkt
            FROM parcel p
            LEFT JOIN parcel_polygon pp ON pp.parcel_id = p.id
            WHERE p.producer_id = $1 AND p.deleted_at IS NULL
            ORDER BY p.updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
            )
            .bind(prod_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?
        }
        None => {
            sqlx::query(
                r#"
            SELECT p.id, p.producer_id, p.crop_type, p.surface_hectares,
                   p.updated_at, p.lww_version,
                   ST_AsText(pp.geom) AS geom_wkt
            FROM parcel p
            LEFT JOIN parcel_polygon pp ON pp.parcel_id = p.id
            WHERE p.deleted_at IS NULL
            ORDER BY p.updated_at DESC
            LIMIT $1 OFFSET $2
            "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?
        }
    };
    tx.commit().await?;

    let items = rows
        .into_iter()
        .map(|r| CompactParcel {
            id: r.get("id"),
            producer_id: r.get("producer_id"),
            crop_type: r.try_get("crop_type").ok(),
            surface_hectares: r.try_get("surface_hectares").ok(),
            geom_wkt: r.try_get("geom_wkt").ok(),
            updated_at: r.get("updated_at"),
            lww_version: r.get("lww_version"),
        })
        .collect();

    Ok(Json(MobilePageResponse {
        items,
        page: params.page,
        size,
    }))
}

// ---------------------------------------------------------------------------
// Sync batch
// ---------------------------------------------------------------------------

/// `POST /m/sync/batch` — apply a batch of CRDT/LWW updates from the mobile app.
async fn post_sync_batch(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(batch): Json<SyncBatchRequest>,
) -> Result<impl IntoResponse, BffError> {
    rate_limit_check(&state, &tenant).await?;

    if batch.items.is_empty() {
        return Err(BffError::BadRequest("batch.items is empty".into()));
    }
    if batch.items.len() > crate::SYNC_BATCH_MAX_ITEMS {
        return Err(BffError::BadRequest(format!(
            "batch.items > {} (got {})",
            crate::SYNC_BATCH_MAX_ITEMS,
            batch.items.len()
        )));
    }

    // Idempotency check (KAYA).
    {
        let mut kaya = state.kaya.clone();
        if idempotency::is_duplicate(&mut kaya, &batch.batch_id)
            .await
            .unwrap_or(false)
        {
            return Err(BffError::Conflict(format!(
                "batch_id {} already processed",
                batch.batch_id
            )));
        }
    }

    let resp: SyncBatchResponse = sync_engine::process_batch(&state, &tenant, &batch).await;

    // Mark idempotency key (best-effort).
    {
        let mut kaya = state.kaya.clone();
        idempotency::mark_processed(&mut kaya, &batch.batch_id).await;
    }

    Ok(Json(resp))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn rate_limit_check(state: &Arc<AppState>, tenant: &TenantContext) -> Result<(), BffError> {
    let mut kaya = state.kaya.clone();
    let allowed = rate_limit::is_allowed(&mut kaya, &tenant.user_id)
        .await
        .unwrap_or(true);
    if !allowed {
        return Err(BffError::RateLimited {
            user_id: tenant.user_id.clone(),
        });
    }
    Ok(())
}
