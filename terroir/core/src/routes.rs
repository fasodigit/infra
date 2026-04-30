// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axum router for terroir-core REST API.
//!
//! All handlers extract `TenantContext` via the custom Axum extractor which
//! validates the JWT / X-Tenant-Slug header before any business logic runs.
//!
//! # Tenant isolation
//! Every service call sets `SET LOCAL search_path` inside a transaction,
//! ensuring RLS and schema isolation even with pgbouncer transaction pooling.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post},
};
use uuid::Uuid;

use crate::{
    dto::{
        AgronomyNoteCreateRequest, HouseholdCreateRequest, PaginationParams, ParcelCreateRequest,
        ParcelPatchRequest, PartsSocialesCreateRequest, PolygonUpdateRequest,
        ProducerCreateRequest, ProducerPatchRequest,
    },
    errors::AppError,
    service::{
        audit::AuditService,
        household,
        idempotency::{is_duplicate, mark_processed},
        parcel, parts_sociales, producer,
    },
    state::AppState,
    tenant_context::TenantContext,
};

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the Axum `Router` for all REST endpoints.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Health
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        // Producers (Module 1)
        .route("/producers", post(create_producer))
        .route("/producers", get(list_producers))
        .route("/producers/{id}", get(get_producer))
        .route("/producers/{id}", patch(patch_producer))
        .route("/producers/{id}", delete(delete_producer))
        // Households
        .route("/households", post(create_household))
        // Parts sociales
        .route("/parts-sociales", post(create_parts_sociales))
        // Parcels (Module 2)
        .route("/parcels", post(create_parcel))
        .route("/parcels", get(list_parcels))
        .route("/parcels/{id}", get(get_parcel))
        .route("/parcels/{id}", patch(patch_parcel))
        // Parcel polygon CRDT
        .route("/parcels/{id}/polygon", post(update_polygon))
        .route("/parcels/{id}/polygon", get(get_polygon))
        // Agronomy notes
        .route("/parcels/{id}/agronomy-notes", post(create_agronomy_note))
        .route("/parcels/{id}/agronomy-notes", get(list_agronomy_notes))
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
// Producer handlers
// ---------------------------------------------------------------------------

async fn create_producer(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(req): Json<ProducerCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Idempotency check (KAYA). ConnectionManager is cheaply cloneable.
    let idem_key = format!(
        "producer:{}:{}:{}",
        tenant.slug, req.cooperative_id, req.nin
    );
    {
        let mut kaya = state.kaya.clone();
        if is_duplicate(&mut kaya, &idem_key).await.unwrap_or(false) {
            return Err(AppError::Conflict(
                "duplicate producer create request".into(),
            ));
        }
    }

    let resp = producer::create_producer(&state.pg, &state.vault, &tenant, &req).await?;

    // Mark idempotency key (best-effort).
    {
        let mut kaya = state.kaya.clone();
        mark_processed(&mut kaya, &idem_key).await;
    }

    // Audit log (best-effort).
    let _ = AuditService::log(
        &state.pg,
        &tenant.audit_schema_name(),
        "producer.created",
        &tenant.user_id,
        &resp.id.to_string(),
        serde_json::json!({ "cooperative_id": resp.cooperative_id }),
        None,
    )
    .await;

    // Publish event (best-effort).
    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            "terroir.member.created",
            &resp.id.to_string(),
            &crate::events::MemberCreatedEvent {
                producer_id: resp.id,
                cooperative_id: resp.cooperative_id,
                tenant_slug: tenant.slug.clone(),
                actor_id: tenant.user_id.clone(),
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(resp)))
}

async fn list_producers(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let page = producer::list_producers(&state.pg, &state.vault, &tenant, &params).await?;
    Ok(Json(page))
}

async fn get_producer(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let resp = producer::get_producer(&state.pg, &state.vault, &tenant, id).await?;
    Ok(Json(resp))
}

async fn patch_producer(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<ProducerPatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = producer::patch_producer(&state.pg, &state.vault, &tenant, id, &req).await?;

    let _ = AuditService::log(
        &state.pg,
        &tenant.audit_schema_name(),
        "producer.updated",
        &tenant.user_id,
        &id.to_string(),
        serde_json::json!({ "lww_version": resp.lww_version }),
        None,
    )
    .await;

    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            "terroir.member.updated",
            &id.to_string(),
            &crate::events::MemberUpdatedEvent {
                producer_id: id,
                tenant_slug: tenant.slug.clone(),
                actor_id: tenant.user_id.clone(),
                lww_version: resp.lww_version,
            },
        )
        .await;

    Ok(Json(resp))
}

async fn delete_producer(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    producer::delete_producer(&state.pg, &tenant, id).await?;

    let _ = AuditService::log(
        &state.pg,
        &tenant.audit_schema_name(),
        "producer.deleted",
        &tenant.user_id,
        &id.to_string(),
        serde_json::json!({}),
        None,
    )
    .await;

    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            "terroir.member.deleted",
            &id.to_string(),
            &crate::events::MemberDeletedEvent {
                producer_id: id,
                tenant_slug: tenant.slug.clone(),
                actor_id: tenant.user_id.clone(),
            },
        )
        .await;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Household handler
// ---------------------------------------------------------------------------

async fn create_household(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(req): Json<HouseholdCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = household::create_household(&state.pg, &tenant, &req).await?;
    Ok((StatusCode::CREATED, Json(resp)))
}

// ---------------------------------------------------------------------------
// Parts sociales handler
// ---------------------------------------------------------------------------

async fn create_parts_sociales(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(req): Json<PartsSocialesCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parts_sociales::create_parts_sociales(&state.pg, &tenant, &req).await?;
    Ok((StatusCode::CREATED, Json(resp)))
}

// ---------------------------------------------------------------------------
// Parcel handlers
// ---------------------------------------------------------------------------

async fn create_parcel(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(req): Json<ParcelCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parcel::create_parcel(&state.pg, &tenant, &req).await?;

    let _ = AuditService::log(
        &state.pg,
        &tenant.audit_schema_name(),
        "parcel.created",
        &tenant.user_id,
        &resp.id.to_string(),
        serde_json::json!({ "producer_id": resp.producer_id }),
        None,
    )
    .await;

    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            "terroir.parcel.created",
            &resp.id.to_string(),
            &crate::events::ParcelCreatedEvent {
                parcel_id: resp.id,
                producer_id: resp.producer_id,
                tenant_slug: tenant.slug.clone(),
                actor_id: tenant.user_id.clone(),
            },
        )
        .await;

    Ok((StatusCode::CREATED, Json(resp)))
}

async fn list_parcels(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let page = parcel::list_parcels(&state.pg, &tenant, &params).await?;
    Ok(Json(page))
}

async fn get_parcel(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parcel::get_parcel(&state.pg, &tenant, id).await?;
    Ok(Json(resp))
}

async fn patch_parcel(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<ParcelPatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parcel::patch_parcel(&state.pg, &tenant, id, &req).await?;

    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            "terroir.parcel.updated",
            &id.to_string(),
            &crate::events::ParcelUpdatedEvent {
                parcel_id: id,
                tenant_slug: tenant.slug.clone(),
                actor_id: tenant.user_id.clone(),
                lww_version: resp.lww_version,
            },
        )
        .await;

    Ok(Json(resp))
}

async fn update_polygon(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<PolygonUpdateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parcel::update_polygon(&state.pg, &tenant, id, &req).await?;
    Ok(Json(resp))
}

async fn get_polygon(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parcel::get_polygon(&state.pg, &tenant, id).await?;
    Ok(Json(resp))
}

async fn create_agronomy_note(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<AgronomyNoteCreateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let resp = parcel::create_agronomy_note(&state.pg, &tenant, id, &req).await?;
    Ok((StatusCode::CREATED, Json(resp)))
}

async fn list_agronomy_notes(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let notes = parcel::list_agronomy_notes(&state.pg, &tenant, id).await?;
    Ok(Json(notes))
}
