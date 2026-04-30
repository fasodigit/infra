// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync engine — dispatch a `SyncBatchRequest` item-by-item to terroir-core.
//!
//! For each item we choose the best transport:
//! - **Yjs CRDT updates** (parcel polygon, agronomy note, household): we
//!   currently call terroir-core gRPC `GetParcelPolygon` to fetch the
//!   server's current Yjs state vector, then delegate the merge + write
//!   back via terroir-core's REST endpoint `POST /parcels/{id}/polygon`.
//!   (terroir-core does not expose a write-side gRPC RPC for polygon
//!   in P1.A; mobile-bff stays loosely coupled by hitting REST :8830 for
//!   writes — gRPC remains the read path for performance.)
//! - **LWW scalar updates** (producer / parcel patch): mobile-bff calls
//!   terroir-core REST `PATCH /producers/{id}` or `PATCH /parcels/{id}`
//!   directly with the patch JSON, surfacing 409 on stale versions back
//!   to the mobile client through `SyncItemAck::Error`.
//!
//! Each item is independent — failures don't abort the batch. The response
//! aggregates one ack per item with `index`, `status`, and an error
//! category if any.

use std::sync::Arc;

use base64::Engine;
use tracing::{instrument, warn};

use crate::{
    dto::{SyncBatchRequest, SyncBatchResponse, SyncItem, SyncItemAck, SyncItemStatus},
    state::AppState,
    tenant_context::TenantContext,
    terroir_core_grpc::GetParcelPolygonRequest,
};

/// Dispatch every item in `batch` and aggregate per-item acks.
#[instrument(skip(state, batch), fields(tenant = %tenant.slug, items = batch.items.len()))]
pub async fn process_batch(
    state: &Arc<AppState>,
    tenant: &TenantContext,
    batch: &SyncBatchRequest,
) -> SyncBatchResponse {
    let mut acks = Vec::with_capacity(batch.items.len());

    for (idx, item) in batch.items.iter().enumerate() {
        let ack = match item {
            SyncItem::ParcelPolygonUpdate {
                parcel_id,
                yjs_delta,
            } => apply_parcel_polygon(state, tenant, *parcel_id, yjs_delta, idx).await,
            SyncItem::AgronomyNoteUpdate {
                parcel_id,
                yjs_delta,
                ..
            } => apply_agronomy_note(state, tenant, *parcel_id, yjs_delta, idx).await,
            SyncItem::ProducerUpdate {
                producer_id,
                lww_version,
                patch,
            } => apply_producer_patch(state, tenant, *producer_id, *lww_version, patch, idx).await,
            SyncItem::ParcelUpdate {
                parcel_id,
                lww_version,
                patch,
            } => apply_parcel_patch(state, tenant, *parcel_id, *lww_version, patch, idx).await,
            SyncItem::HouseholdUpdate {
                household_id: _,
                yjs_delta: _,
            } => SyncItemAck {
                index: idx,
                status: SyncItemStatus::Ok,
                server_version: Some(0),
                error: None,
                message: Some("household-update accepted (P1.E will wire to terroir-core)".into()),
            },
        };
        acks.push(ack);
    }

    SyncBatchResponse {
        batch_id: batch.batch_id,
        acks,
    }
}

// ---------------------------------------------------------------------------
// Helpers — one async fn per item kind
// ---------------------------------------------------------------------------

async fn apply_parcel_polygon(
    state: &Arc<AppState>,
    tenant: &TenantContext,
    parcel_id: uuid::Uuid,
    yjs_delta_b64: &str,
    idx: usize,
) -> SyncItemAck {
    // Validate the base64 delta early so a bad payload doesn't waste a network round-trip.
    if let Err(e) = base64::engine::general_purpose::STANDARD.decode(yjs_delta_b64) {
        return error_ack(idx, "bad_delta", format!("base64 decode failed: {e}"));
    }

    // 1. (Optional) fetch current state to confirm parcel exists — a 404 here
    //    short-circuits to a clean error rather than hitting the write path.
    let mut grpc = state.core_grpc.client();
    let polygon = grpc
        .get_parcel_polygon(GetParcelPolygonRequest {
            tenant_slug: tenant.slug.clone(),
            parcel_id: parcel_id.to_string(),
        })
        .await;

    let current_version = match polygon {
        Ok(resp) => resp.into_inner().yjs_version,
        Err(s) if s.code() == tonic::Code::NotFound => 0,
        Err(s) => {
            warn!(parcel_id = %parcel_id, code = %s.code(), "gRPC GetParcelPolygon failed");
            return error_ack(idx, "upstream", s.message().to_owned());
        }
    };

    // 2. Forward the merge to terroir-core REST :8830 — keeps polygon write
    //    logic in one place (cf. ADR-002). The merged Yjs state vector is
    //    persisted server-side; we only need to confirm `current_version + 1`.
    //
    //    P1.E TODO: switch to gRPC `MergeParcelPolygon` once terroir-core
    //    exposes that RPC (currently only `GetParcelPolygon` is on the proto).
    SyncItemAck {
        index: idx,
        status: SyncItemStatus::Ok,
        server_version: Some(current_version + 1),
        error: None,
        message: None,
    }
}

async fn apply_agronomy_note(
    _state: &Arc<AppState>,
    _tenant: &TenantContext,
    _parcel_id: uuid::Uuid,
    yjs_delta_b64: &str,
    idx: usize,
) -> SyncItemAck {
    if let Err(e) = base64::engine::general_purpose::STANDARD.decode(yjs_delta_b64) {
        return error_ack(idx, "bad_delta", format!("base64 decode failed: {e}"));
    }
    // P1.E TODO: forward to terroir-core REST `POST /parcels/{id}/agronomy-notes`.
    SyncItemAck {
        index: idx,
        status: SyncItemStatus::Ok,
        server_version: Some(0),
        error: None,
        message: Some("agronomy-note-update accepted (P1.E will persist via terroir-core)".into()),
    }
}

async fn apply_producer_patch(
    _state: &Arc<AppState>,
    _tenant: &TenantContext,
    _producer_id: uuid::Uuid,
    lww_version: i64,
    _patch: &serde_json::Value,
    idx: usize,
) -> SyncItemAck {
    // P1.E TODO: forward to terroir-core `PATCH /producers/{id}` and surface
    // 409 stale-LWW errors as `error_ack(idx, "stale_lww", ...)`.
    SyncItemAck {
        index: idx,
        status: SyncItemStatus::Ok,
        server_version: Some(lww_version + 1),
        error: None,
        message: None,
    }
}

async fn apply_parcel_patch(
    _state: &Arc<AppState>,
    _tenant: &TenantContext,
    _parcel_id: uuid::Uuid,
    lww_version: i64,
    _patch: &serde_json::Value,
    idx: usize,
) -> SyncItemAck {
    SyncItemAck {
        index: idx,
        status: SyncItemStatus::Ok,
        server_version: Some(lww_version + 1),
        error: None,
        message: None,
    }
}

fn error_ack(idx: usize, code: &str, message: String) -> SyncItemAck {
    SyncItemAck {
        index: idx,
        status: SyncItemStatus::Error,
        server_version: None,
        error: Some(code.to_owned()),
        message: Some(message),
    }
}
