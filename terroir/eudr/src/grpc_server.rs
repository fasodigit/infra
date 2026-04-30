// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tonic `EudrService` server implementation (proto/eudr.proto).
//!
//! Binds :8731 (cf. `INFRA/port-policy.yaml`).
//! Tenant slug travels in the request body (`tenant_slug` field). mTLS is
//! deferred to P2 once the SPIRE/ARMAGEDDON mesh is wired (currently plain
//! TCP, loopback only in dev).

use std::sync::Arc;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::eudr_proto::{
    Dds, GenerateDdsRequest as ProtoGenerateDdsRequest, HealthReply, HealthRequest,
    ValidateRequest as ProtoValidateRequest, ValidationResult,
    eudr_service_server::{EudrService, EudrServiceServer},
};
use crate::service::validator;
use crate::state::AppState;
use crate::tenant_context::{TenantContext, is_valid_slug};

pub struct EudrServiceImpl {
    pub state: Arc<AppState>,
}

#[tonic::async_trait]
impl EudrService for EudrServiceImpl {
    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<HealthReply>, Status> {
        Ok(Response::new(HealthReply {
            status: "ready".to_owned(),
            version: crate::version().to_owned(),
        }))
    }

    async fn validate_parcel(
        &self,
        req: Request<ProtoValidateRequest>,
    ) -> Result<Response<ValidationResult>, Status> {
        let inner = req.into_inner();
        if !is_valid_slug(&inner.tenant_slug) {
            return Err(Status::invalid_argument("invalid tenant_slug"));
        }
        let parcel_id = Uuid::parse_str(&inner.parcel_id)
            .map_err(|_| Status::invalid_argument("invalid parcel_id UUID"))?;
        let polygon: serde_json::Value = serde_json::from_str(&inner.polygon_geojson)
            .map_err(|e| Status::invalid_argument(format!("polygon JSON: {e}")))?;

        let tenant = TenantContext {
            slug: inner.tenant_slug,
            user_id: "grpc".to_owned(),
            role: "service".to_owned(),
        };

        let outcome = validator::validate_parcel(&self.state, &tenant, parcel_id, &polygon)
            .await
            .map_err(|e| Status::internal(format!("validate: {e}")))?;
        let cache_status = if outcome.from_cache { "HIT" } else { "MISS" };

        Ok(Response::new(ValidationResult {
            validation_id: outcome.response.validation_id.to_string(),
            parcel_id: outcome.response.parcel_id.to_string(),
            status: outcome.response.status.as_db_str().to_owned(),
            evidence_url: outcome.response.evidence_url.unwrap_or_default(),
            dds_draft_id: outcome
                .response
                .dds_draft_id
                .map(|u| u.to_string())
                .unwrap_or_default(),
            deforestation_overlap_ha: outcome.response.deforestation_overlap_ha,
            dataset_version: outcome.response.dataset_version,
            polygon_hash: outcome.response.polygon_hash,
            cache_status: cache_status.to_owned(),
            computed_at: outcome.response.computed_at.to_rfc3339(),
        }))
    }

    async fn generate_dds(
        &self,
        _req: Request<ProtoGenerateDdsRequest>,
    ) -> Result<Response<Dds>, Status> {
        // The full generate flow uses the REST handler (HTTP-only in P1).
        // Exposing it via gRPC is tracked for P2 once admin tooling needs it.
        Err(Status::unimplemented(
            "GenerateDds gRPC is not implemented in P1.B — use POST /eudr/dds",
        ))
    }
}

pub fn build_server(state: Arc<AppState>) -> EudrServiceServer<EudrServiceImpl> {
    EudrServiceServer::new(EudrServiceImpl { state })
}
