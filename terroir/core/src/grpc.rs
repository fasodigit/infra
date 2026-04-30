// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tonic gRPC service implementation for `terroir.core.v1.CoreService`.
//!
//! gRPC server binds :8730 (cf. INFRA/port-policy.yaml).
//!
//! # Consumers
//! - `terroir-eudr` (P1.B): `GetProducer`, `ListParcelsByCooperative`,
//!   `GetParcelPolygon`.
//! - `terroir-mobile-bff` (P1.D): same three RPCs for sync batch operations.
//!
//! # Tenant isolation
//! The `X-Tenant-Slug` metadata key (Tonic calls it "metadata") is required
//! on every RPC. Without it the call is rejected with `UNAUTHENTICATED`.
//!
//! # TODO P1.B
//! - Add mTLS interceptor (SPIRE SVID) once the ARMAGEDDON/SPIRE mesh is
//!   wired (currently plain TCP on :8730, loopback only in dev).

use std::sync::Arc;

use base64::Engine;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::{
    service::{
        parcel::{get_polygon_raw, list_parcels_by_producer_ids},
        producer::{get_producer_raw, list_producer_ids_for_coop},
        vault_transit::pii_context,
    },
    state::AppState,
};

// Include the generated Tonic code.
pub mod terroir_core {
    tonic::include_proto!("terroir.core.v1");
}

use terroir_core::{
    GetParcelPolygonRequest, GetProducerRequest, HealthReply, HealthRequest, ListParcelsRequest,
    Parcel, ParcelPolygon, Producer,
    core_service_server::{CoreService, CoreServiceServer},
};

// ---------------------------------------------------------------------------
// CoreServiceImpl
// ---------------------------------------------------------------------------

pub struct CoreServiceImpl {
    pub state: Arc<AppState>,
}

#[tonic::async_trait]
impl CoreService for CoreServiceImpl {
    // -- Health --

    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<HealthReply>, Status> {
        Ok(Response::new(HealthReply {
            status: "ready".to_owned(),
            version: crate::version().to_owned(),
        }))
    }

    // -- GetProducer --

    async fn get_producer(
        &self,
        req: Request<GetProducerRequest>,
    ) -> Result<Response<Producer>, Status> {
        let inner = req.into_inner();
        let tenant_slug = validate_slug(&inner.tenant_slug)?;
        let producer_id = Uuid::parse_str(&inner.producer_id)
            .map_err(|_| Status::invalid_argument("invalid producer_id UUID"))?;

        let row = get_producer_raw(&self.state.pg, tenant_slug, producer_id)
            .await
            .map_err(|e| match e {
                crate::errors::AppError::NotFound(msg) => Status::not_found(msg),
                other => Status::internal(other.to_string()),
            })?;

        // Decrypt PII fields — call vault directly to avoid lifetime issues with closures.
        let vault = &self.state.vault;
        let full_name = match &row.full_name_encrypted {
            Some(b) => vault
                .decrypt(b, &pii_context(tenant_slug, "full_name", &producer_id))
                .await
                .unwrap_or_default(),
            None => String::new(),
        };
        let nin = match &row.nin_encrypted {
            Some(b) => vault
                .decrypt(b, &pii_context(tenant_slug, "nin", &producer_id))
                .await
                .unwrap_or_default(),
            None => String::new(),
        };
        let phone = match &row.phone_encrypted {
            Some(b) => vault
                .decrypt(b, &pii_context(tenant_slug, "phone", &producer_id))
                .await
                .unwrap_or_default(),
            None => String::new(),
        };
        let photo_url = match &row.photo_url_encrypted {
            Some(b) => vault
                .decrypt(b, &pii_context(tenant_slug, "photo_url", &producer_id))
                .await
                .unwrap_or_default(),
            None => String::new(),
        };
        let lat_str = match &row.gps_domicile_lat_encrypted {
            Some(b) => vault
                .decrypt(
                    b,
                    &pii_context(tenant_slug, "gps_domicile_lat", &producer_id),
                )
                .await
                .unwrap_or_default(),
            None => String::new(),
        };
        let lon_str = match &row.gps_domicile_lon_encrypted {
            Some(b) => vault
                .decrypt(
                    b,
                    &pii_context(tenant_slug, "gps_domicile_lon", &producer_id),
                )
                .await
                .unwrap_or_default(),
            None => String::new(),
        };

        let lat: f64 = lat_str.parse().unwrap_or(0.0);
        let lon: f64 = lon_str.parse().unwrap_or(0.0);

        Ok(Response::new(Producer {
            producer_id: row.id.to_string(),
            cooperative_id: row.cooperative_id.to_string(),
            external_id: row.external_id.unwrap_or_default(),
            full_name,
            nin,
            phone,
            photo_url,
            gps_domicile_lat: lat,
            gps_domicile_lon: lon,
            household_id: row.household_id.map(|u| u.to_string()).unwrap_or_default(),
            primary_crop: row.primary_crop.unwrap_or_default(),
            registered_at: row.registered_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            lww_version: row.lww_version,
            deleted: row.deleted_at.is_some(),
        }))
    }

    // -- ListParcelsByCooperative (server streaming) --

    type ListParcelsByCooperativeStream =
        tokio_stream::wrappers::ReceiverStream<Result<Parcel, Status>>;

    async fn list_parcels_by_cooperative(
        &self,
        req: Request<ListParcelsRequest>,
    ) -> Result<Response<Self::ListParcelsByCooperativeStream>, Status> {
        let inner = req.into_inner();
        let tenant_slug = validate_slug(&inner.tenant_slug)?.to_owned();

        let coop_id = Uuid::parse_str(&inner.cooperative_id)
            .map_err(|_| Status::invalid_argument("invalid cooperative_id UUID"))?;

        // Resolve producer IDs for the cooperative.
        let producer_ids = list_producer_ids_for_coop(&self.state.pg, &tenant_slug, coop_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Fetch all parcels.
        let parcels = list_parcels_by_producer_ids(&self.state.pg, &tenant_slug, &producer_ids)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Stream via channel.
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        tokio::spawn(async move {
            for row in parcels {
                let p = Parcel {
                    parcel_id: row.id.to_string(),
                    producer_id: row.producer_id.to_string(),
                    crop_type: row.crop_type.unwrap_or_default(),
                    planted_at: row.planted_at.map(|d| d.to_string()).unwrap_or_default(),
                    surface_hectares: row.surface_hectares.unwrap_or(0.0),
                    registered_at: row.registered_at.to_rfc3339(),
                    updated_at: row.updated_at.to_rfc3339(),
                    lww_version: row.lww_version,
                };
                if tx.send(Ok(p)).await.is_err() {
                    break; // client disconnected
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    // -- GetParcelPolygon --

    async fn get_parcel_polygon(
        &self,
        req: Request<GetParcelPolygonRequest>,
    ) -> Result<Response<ParcelPolygon>, Status> {
        let inner = req.into_inner();
        let tenant_slug = validate_slug(&inner.tenant_slug)?;
        let parcel_id = Uuid::parse_str(&inner.parcel_id)
            .map_err(|_| Status::invalid_argument("invalid parcel_id UUID"))?;

        let row = get_polygon_raw(&self.state.pg, tenant_slug, parcel_id)
            .await
            .map_err(|e| match e {
                crate::errors::AppError::NotFound(msg) => Status::not_found(msg),
                other => Status::internal(other.to_string()),
            })?;

        let yjs_state_b64 = base64::engine::general_purpose::STANDARD.encode(&row.yjs_doc);

        Ok(Response::new(ParcelPolygon {
            parcel_id: parcel_id.to_string(),
            yjs_state: yjs_state_b64.into_bytes(),
            geojson: row
                .geom_wkt
                .as_deref()
                .map(|w| format!(r#"{{"type":"Feature","geometry":{{"wkt":"{}"}}}}"#, w))
                .unwrap_or_default(),
            geom_wkt: row.geom_wkt.unwrap_or_default(),
            yjs_version: row.yjs_version,
            updated_at: row.updated_at.to_rfc3339(),
        }))
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

#[allow(clippy::result_large_err)]
fn validate_slug(slug: &str) -> Result<&str, Status> {
    if crate::tenant_context::is_valid_slug(slug) {
        Ok(slug)
    } else {
        Err(Status::invalid_argument(format!(
            "invalid tenant_slug: '{slug}'"
        )))
    }
}

// ---------------------------------------------------------------------------
// Server builder
// ---------------------------------------------------------------------------

/// Build the Tonic `Server` with the `CoreService` implementation.
pub fn build_server(state: Arc<AppState>) -> CoreServiceServer<CoreServiceImpl> {
    CoreServiceServer::new(CoreServiceImpl { state })
}
