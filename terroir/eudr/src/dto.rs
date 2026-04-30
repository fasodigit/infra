// SPDX-License-Identifier: AGPL-3.0-or-later
//! Serde DTOs for terroir-eudr REST API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// /eudr/validate
// ---------------------------------------------------------------------------

/// Request body for `POST /eudr/validate`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRequest {
    pub parcel_id: Uuid,
    /// Inline GeoJSON Polygon (Feature or Geometry) — caller may also rely on
    /// `terroir-core` GetParcelPolygon if absent (NOT implemented in P1.B
    /// MVP — caller passes inline).
    pub polygon_geo_json: serde_json::Value,
}

/// Validation outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationStatus {
    Validated,
    Rejected,
    Escalated,
}

impl ValidationStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Validated => "VALIDATED",
            Self::Rejected => "REJECTED",
            Self::Escalated => "ESCALATED",
        }
    }
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "REJECTED" => Self::Rejected,
            "ESCALATED" => Self::Escalated,
            _ => Self::Validated,
        }
    }
}

/// Response body for `POST /eudr/validate` and `GET /eudr/parcels/{id}/validations`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResponse {
    pub validation_id: Uuid,
    pub parcel_id: Uuid,
    pub status: ValidationStatus,
    pub evidence_url: Option<String>,
    pub dds_draft_id: Option<Uuid>,
    pub deforestation_overlap_ha: f64,
    pub dataset_version: String,
    pub polygon_hash: String,
    /// `"HIT"` or `"MISS"`. Set by the handler from the cache lookup.
    #[serde(default)]
    pub cache_status: String,
    pub computed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// /eudr/dds/{id}/sign
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DdsSignRequest {
    /// Optional EORI override. When absent the service uses `EUDR_DEFAULT_EORI`.
    pub operator_eori: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DdsSignResponse {
    pub dds_id: Uuid,
    pub signature_fingerprint: String,
    pub status: String,
    pub signed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// /eudr/dds/{id}/submit
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DdsSubmitResponse {
    pub dds_id: Uuid,
    pub status: String,
    pub traces_nt_ref: Option<String>,
    pub attempt_no: i32,
}

// ---------------------------------------------------------------------------
// /eudr/dds/{id} generate
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateDdsRequest {
    pub validation_id: Uuid,
    pub operator_eori: Option<String>,
    pub hs_code: String,
    pub quantity: f64,
    pub unit: String,
    pub country_iso2: String,
    pub harvest_period: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DdsResponse {
    pub dds_id: Uuid,
    pub validation_id: Uuid,
    pub parcel_id: Uuid,
    pub status: String,
    pub operator_eori: String,
    pub hs_code: String,
    pub country_iso2: String,
    pub evidence_url: Option<String>,
    pub payload_sha256: String,
    pub created_at: DateTime<Utc>,
}
