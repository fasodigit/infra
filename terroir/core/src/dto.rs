// SPDX-License-Identifier: AGPL-3.0-or-later
//! Serde DTOs for terroir-core REST API.
//!
//! Request types carry raw user input.
//! Response types carry decrypted, formatted data ready for the client.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Standard keyset-pagination query params.
///
/// Accepts both camelCase (`cooperativeId`, `producerId`) and snake_case
/// (`cooperative_id`, `producer_id`) variants for client convenience.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_size")]
    pub size: u64,
    pub sort: Option<String>,
    /// Cooperative filter (for /producers).
    #[serde(alias = "cooperativeId")]
    pub cooperative_id: Option<Uuid>,
    /// Producer filter (for /parcels).
    #[serde(alias = "producerId")]
    pub producer_id: Option<Uuid>,
}

fn default_page() -> u64 {
    0
}
fn default_size() -> u64 {
    20
}

/// Paginated response wrapper.
#[derive(Debug, Serialize)]
pub struct PageResponse<T: Serialize> {
    pub items: Vec<T>,
    pub page: u64,
    pub size: u64,
    pub total: Option<i64>,
}

// ---------------------------------------------------------------------------
// Producer DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /producers`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducerCreateRequest {
    pub cooperative_id: Uuid,
    pub external_id: Option<String>,
    /// PII — will be encrypted before INSERT.
    pub full_name: String,
    pub nin: String,
    pub phone: String,
    pub photo_url: Option<String>,
    pub gps_domicile_lat: f64,
    pub gps_domicile_lon: f64,
    pub household_id: Option<Uuid>,
    pub primary_crop: String,
}

/// Request body for `PATCH /producers/{id}`.
/// All fields optional — only present fields are updated.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducerPatchRequest {
    /// Client must send the current LWW version; server rejects if stale.
    pub lww_version: i64,
    pub full_name: Option<String>,
    pub nin: Option<String>,
    pub phone: Option<String>,
    pub photo_url: Option<String>,
    pub gps_domicile_lat: Option<f64>,
    pub gps_domicile_lon: Option<f64>,
    pub primary_crop: Option<String>,
    pub household_id: Option<Uuid>,
}

/// Response for producer endpoints (PII already decrypted).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducerResponse {
    pub id: Uuid,
    pub cooperative_id: Uuid,
    pub external_id: Option<String>,
    pub full_name: String,
    pub nin: String,
    pub phone: String,
    pub photo_url: Option<String>,
    pub gps_domicile_lat: f64,
    pub gps_domicile_lon: f64,
    pub household_id: Option<Uuid>,
    pub primary_crop: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
}

// ---------------------------------------------------------------------------
// Parcel DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /parcels`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelCreateRequest {
    pub producer_id: Uuid,
    pub crop_type: Option<String>,
    pub planted_at: Option<NaiveDate>,
    pub surface_hectares: Option<f64>,
}

/// Request body for `PATCH /parcels/{id}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelPatchRequest {
    pub lww_version: i64,
    pub crop_type: Option<String>,
    pub planted_at: Option<NaiveDate>,
    pub surface_hectares: Option<f64>,
}

/// Response for parcel endpoints.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParcelResponse {
    pub id: Uuid,
    pub producer_id: Uuid,
    pub crop_type: Option<String>,
    pub planted_at: Option<NaiveDate>,
    pub surface_hectares: Option<f64>,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
}

// ---------------------------------------------------------------------------
// Polygon DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /parcels/{id}/polygon`.
/// `yjs_update` is a base64-encoded Yjs binary update.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolygonUpdateRequest {
    /// Base64-encoded Yjs v1 update bytes from the client.
    pub yjs_update: String,
    /// Full GeoJSON Feature containing the Polygon geometry.
    pub geojson: serde_json::Value,
}

/// Response for polygon endpoints.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolygonResponse {
    pub parcel_id: Uuid,
    /// Base64-encoded merged Yjs state.
    pub yjs_state: String,
    pub geojson: serde_json::Value,
    pub geom_wkt: Option<String>,
    pub yjs_version: i64,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Agronomy Note DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /parcels/{id}/agronomy-notes`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgronomyNoteCreateRequest {
    /// Base64-encoded Yjs text CRDT update.
    pub yjs_update: String,
}

/// Response for agronomy note endpoints.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgronomyNoteResponse {
    pub id: Uuid,
    pub parcel_id: Uuid,
    /// Base64-encoded merged Yjs state.
    pub yjs_state: String,
    pub yjs_version: i64,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Household DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /households`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdCreateRequest {
    pub cooperative_id: Uuid,
    pub head_producer_id: Option<Uuid>,
    /// Optional initial Yjs document (base64). Empty doc if absent.
    pub yjs_update: Option<String>,
}

/// Response for household endpoints.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdResponse {
    pub id: Uuid,
    pub cooperative_id: Uuid,
    pub head_producer_id: Option<Uuid>,
    /// Base64-encoded Yjs state.
    pub yjs_state: String,
    pub yjs_version: i64,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Parts Sociales DTOs
// ---------------------------------------------------------------------------

/// Request body for `POST /parts-sociales`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartsSocialesCreateRequest {
    pub producer_id: Uuid,
    pub cooperative_id: Uuid,
    pub nb_parts: i32,
    pub valeur_nominale_xof: f64,
    pub adhesion_date: NaiveDate,
    pub ag_reference: Option<String>,
}

/// Response for parts sociales endpoints.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartsSocialesResponse {
    pub id: Uuid,
    pub producer_id: Uuid,
    pub cooperative_id: Uuid,
    pub nb_parts: i32,
    pub valeur_nominale_xof: f64,
    pub adhesion_date: NaiveDate,
    pub ag_reference: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
}
