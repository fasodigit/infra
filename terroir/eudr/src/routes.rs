// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axum router for terroir-eudr REST API.
//!
//! Endpoints (cf. ULTRAPLAN §6 P1.3):
//!   - `GET  /health/{ready,live}`
//!   - `POST /eudr/validate`
//!   - `POST /eudr/dds/{ddsId}/sign`
//!   - `POST /eudr/dds/{ddsId}/submit`
//!   - `GET  /eudr/dds/{ddsId}/download`
//!   - `GET  /eudr/parcels/{parcelId}/validations`
//!
//! Tenant context (RLS) is extracted via `TenantContext` (JWT or
//! `X-Tenant-Slug`).
//!
//! `/eudr/validate` adds `X-Eudr-Cache-Status: HIT|MISS` to the response
//! header for the ARMAGEDDON gateway metric
//! `armageddon_terroir_eudr_cache_hit_total`.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, Response, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::dto::{
    DdsResponse, DdsSignRequest, DdsSignResponse, DdsSubmitResponse, GenerateDdsRequest,
    ValidateRequest, ValidationResponse,
};
use crate::errors::AppError;
use crate::events::{DdsDlqEvent, DdsEvent};
use crate::repository;
use crate::service::{dds_generator, dds_signer, traces_nt_submitter, validator};
use crate::state::AppState;
use crate::tenant_context::TenantContext;

fn build_business_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/eudr/validate", post(validate_handler))
        .route("/eudr/dds", post(generate_dds_handler))
        .route("/eudr/dds/{ddsId}/sign", post(sign_dds_handler))
        .route("/eudr/dds/{ddsId}/submit", post(submit_dds_handler))
        .route("/eudr/dds/{ddsId}/download", get(download_dds_handler))
        .route(
            "/eudr/parcels/{parcelId}/validations",
            get(list_validations_handler),
        )
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health/ready", get(health_ready))
        .route("/health/live", get(health_live))
        .merge(build_business_router())
        // ARMAGEDDON forwards /api/terroir/eudr/* without stripping prefix,
        // and the business router already has /eudr/* as its top-level path.
        // So we nest under /api/terroir (not /api/terroir/eudr) to avoid
        // ending up at /api/terroir/eudr/eudr/*.
        .nest("/api/terroir", build_business_router())
        .with_state(state)
}

async fn health_ready() -> impl IntoResponse {
    (StatusCode::OK, "ready")
}

async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, "live")
}

// ---------------------------------------------------------------------------
// /eudr/validate
// ---------------------------------------------------------------------------

async fn validate_handler(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(req): Json<ValidateRequest>,
) -> Result<Response<Body>, AppError> {
    let outcome = validator::validate_parcel(&state, &tenant, req.parcel_id, &req.polygon_geo_json)
        .await
        .map_err(|e| {
            // Polygon parse failures and missing coordinates → 400 (client
            // error), not 500. Anything containing "polygon" / "coordinates"
            // / "GeoJSON" / "geojson" in the chain is treated as bad input.
            let msg = format!("{e:#}");
            let lower = msg.to_lowercase();
            if lower.contains("polygon")
                || lower.contains("coordinates")
                || lower.contains("geojson")
                || lower.contains("parse")
            {
                AppError::BadRequest(msg)
            } else {
                AppError::Internal(e)
            }
        })?;

    let cache_header = if outcome.from_cache { "HIT" } else { "MISS" };

    let body = serde_json::to_vec(&outcome.response)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("encode validate response: {e}")))?;

    let mut resp = Response::new(Body::from(body));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp.headers_mut().insert(
        "X-Eudr-Cache-Status",
        HeaderValue::from_static(cache_header),
    );

    Ok(resp)
}

// ---------------------------------------------------------------------------
// /eudr/dds   (generate)
// ---------------------------------------------------------------------------

async fn generate_dds_handler(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Json(req): Json<GenerateDdsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let validation = repository::get_validation(&state.pg, &tenant, req.validation_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("validation not found".into()))?;

    if validation.status != "VALIDATED" {
        return Err(AppError::BadRequest(format!(
            "cannot generate DDS for status={}",
            validation.status
        )));
    }

    let operator_eori = req
        .operator_eori
        .clone()
        .unwrap_or_else(|| state.settings.default_eori.clone());

    let dds_id = Uuid::now_v7();
    let generated = dds_generator::generate(dds_id, &validation, &req, &operator_eori)
        .map_err(AppError::Internal)?;

    let row = repository::insert_dds(
        &state.pg,
        &tenant,
        validation.id,
        validation.parcel_id,
        &operator_eori,
        &req.hs_code,
        req.quantity,
        &req.unit,
        &req.country_iso2,
        &req.harvest_period,
        generated.payload.clone(),
        &generated.payload_sha256,
        validation.evidence_url.as_deref(),
    )
    .await
    .map_err(AppError::Internal)?;

    // Note: in P2 we'll PUT generated.evidence_pdf into MinIO. For P1 we keep
    // the bytes in-memory for /download.
    let _evidence_pdf_size = generated.evidence_pdf.len();

    #[cfg(feature = "kafka")]
    state
        .events
        .publish(
            "terroir.dds.generated",
            &row.id.to_string(),
            &DdsEvent {
                dds_id: row.id,
                validation_id: row.validation_id,
                tenant_slug: tenant.slug.clone(),
                status: row.status.clone(),
                payload_sha256: row.payload_sha256.clone(),
            },
        )
        .await;
    #[cfg(not(feature = "kafka"))]
    {
        let _ = DdsEvent {
            dds_id: row.id,
            validation_id: row.validation_id,
            tenant_slug: tenant.slug.clone(),
            status: row.status.clone(),
            payload_sha256: row.payload_sha256.clone(),
        };
    }

    let resp = DdsResponse {
        dds_id: row.id,
        validation_id: row.validation_id,
        parcel_id: row.parcel_id,
        status: row.status,
        operator_eori: operator_eori.clone(),
        hs_code: req.hs_code,
        country_iso2: req.country_iso2,
        evidence_url: row.evidence_url,
        payload_sha256: row.payload_sha256,
        created_at: row.created_at,
    };
    Ok((StatusCode::CREATED, Json(resp)))
}

// ---------------------------------------------------------------------------
// /eudr/dds/{id}/sign
// ---------------------------------------------------------------------------

async fn sign_dds_handler(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(dds_id): Path<Uuid>,
    Json(req): Json<DdsSignRequest>,
) -> Result<impl IntoResponse, AppError> {
    let row = repository::get_dds(&state.pg, &tenant, dds_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("dds not found".into()))?;

    let operator_eori = req
        .operator_eori
        .clone()
        .or(row.operator_eori.clone())
        .unwrap_or_else(|| state.settings.default_eori.clone());

    let signed = dds_signer::sign(
        &state.http_client,
        &state.settings.vault_addr,
        &state.settings.vault_token,
        &state.settings.vault_pki_role,
        &operator_eori,
        &row.payload_sha256,
    )
    .await
    .map_err(|e| AppError::Upstream(format!("Vault PKI: {e}")))?;

    repository::update_dds_signature(
        &state.pg,
        &tenant,
        dds_id,
        &signed.fingerprint,
        &signed.cert_pem,
    )
    .await
    .map_err(AppError::Internal)?;

    Ok(Json(DdsSignResponse {
        dds_id,
        signature_fingerprint: signed.fingerprint,
        status: "signed".into(),
        signed_at: Utc::now(),
    }))
}

// ---------------------------------------------------------------------------
// /eudr/dds/{id}/submit
// ---------------------------------------------------------------------------

async fn submit_dds_handler(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(dds_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let row = repository::get_dds(&state.pg, &tenant, dds_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("dds not found".into()))?;
    if row.status != "signed" {
        return Err(AppError::BadRequest(format!(
            "cannot submit dds in status={}",
            row.status
        )));
    }

    let attempt_no = repository::next_attempt_no(&state.pg, &tenant, dds_id)
        .await
        .map_err(AppError::Internal)?;

    let submission = traces_nt_submitter::submit(
        &state.http_client,
        &state.settings.traces_nt_url,
        dds_id,
        &row.payload_json,
    )
    .await;

    match submission {
        Ok(ack) => {
            let outcome = if ack.http_status < 300 {
                "submitted"
            } else {
                "rejected"
            };
            repository::insert_submission(
                &state.pg,
                &tenant,
                dds_id,
                attempt_no,
                outcome,
                ack.reference.as_deref(),
                Some(ack.http_status as i32),
                Some(&ack.body),
            )
            .await
            .map_err(AppError::Internal)?;

            let new_status = if outcome == "submitted" {
                "submitted"
            } else {
                "rejected"
            };
            repository::update_dds_status(&state.pg, &tenant, dds_id, new_status)
                .await
                .map_err(AppError::Internal)?;

            #[cfg(feature = "kafka")]
            state
                .events
                .publish(
                    if outcome == "submitted" {
                        "terroir.dds.submitted"
                    } else {
                        "terroir.dds.rejected"
                    },
                    &dds_id.to_string(),
                    &DdsEvent {
                        dds_id,
                        validation_id: row.validation_id,
                        tenant_slug: tenant.slug.clone(),
                        status: new_status.to_owned(),
                        payload_sha256: row.payload_sha256.clone(),
                    },
                )
                .await;

            Ok(Json(DdsSubmitResponse {
                dds_id,
                status: new_status.into(),
                traces_nt_ref: ack.reference,
                attempt_no,
            }))
        }
        Err(e) => {
            repository::insert_submission(
                &state.pg,
                &tenant,
                dds_id,
                attempt_no,
                "dlq",
                None,
                None,
                Some(&format!("{e}")),
            )
            .await
            .map_err(AppError::Internal)?;

            #[cfg(feature = "kafka")]
            state
                .events
                .publish(
                    "terroir.dds.submitted.dlq",
                    &dds_id.to_string(),
                    &DdsDlqEvent {
                        dds_id,
                        tenant_slug: tenant.slug.clone(),
                        attempt_no,
                        reason: e.to_string(),
                    },
                )
                .await;
            #[cfg(not(feature = "kafka"))]
            {
                let _ = DdsDlqEvent {
                    dds_id,
                    tenant_slug: tenant.slug.clone(),
                    attempt_no,
                    reason: e.to_string(),
                };
            }

            Err(AppError::Upstream(format!("TRACES NT: {e}")))
        }
    }
}

// ---------------------------------------------------------------------------
// /eudr/dds/{id}/download
// ---------------------------------------------------------------------------

async fn download_dds_handler(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(dds_id): Path<Uuid>,
) -> Result<Response<Body>, AppError> {
    let row = repository::get_dds(&state.pg, &tenant, dds_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("dds not found".into()))?;
    let validation = repository::get_validation(&state.pg, &tenant, row.validation_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("validation not found".into()))?;

    let regenerated = dds_generator::generate(
        dds_id,
        &validation,
        &GenerateDdsRequest {
            validation_id: row.validation_id,
            operator_eori: row.operator_eori.clone(),
            hs_code: row.hs_code.clone().unwrap_or_default(),
            quantity: 0.0,
            unit: "kg".into(),
            country_iso2: row.country_iso2.clone().unwrap_or_default(),
            harvest_period: "n/a".into(),
        },
        &row.operator_eori
            .clone()
            .unwrap_or_else(|| state.settings.default_eori.clone()),
    )
    .map_err(AppError::Internal)?;

    let mut response = Response::new(Body::from(regenerated.evidence_pdf));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"dds-{dds_id}.pdf\""))
            .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid header: {e}")))?,
    );
    Ok(response)
}

// ---------------------------------------------------------------------------
// /eudr/parcels/{parcelId}/validations
// ---------------------------------------------------------------------------

async fn list_validations_handler(
    State(state): State<Arc<AppState>>,
    tenant: TenantContext,
    Path(parcel_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let list = repository::list_validations(&state.pg, &tenant, parcel_id)
        .await
        .map_err(AppError::Internal)?;
    let resp: Vec<ValidationResponse> = list;
    Ok(Json(json!({ "items": resp })))
}
