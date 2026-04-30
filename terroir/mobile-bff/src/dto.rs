// SPDX-License-Identifier: AGPL-3.0-or-later
//! Serde DTOs for terroir-mobile-bff.
//!
//! All DTOs are **mobile-friendly**: smaller payloads than terroir-core's
//! REST surface (omit photo URL on list, compact GeoJSON, no decrypted PII
//! beyond `full_name`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Mobile pagination query — defaults `page=0`, `size=20`, max `size=100`.
#[derive(Debug, Deserialize)]
pub struct MobilePaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_size")]
    pub size: u64,
    pub cooperative_id: Option<Uuid>,
    pub producer_id: Option<Uuid>,
}

fn default_page() -> u64 {
    0
}
fn default_size() -> u64 {
    crate::PAGE_SIZE_DEFAULT
}

impl MobilePaginationParams {
    /// Clamp `size` to `[1, PAGE_SIZE_MAX]`.
    pub fn clamped_size(&self) -> u64 {
        self.size.clamp(1, crate::PAGE_SIZE_MAX)
    }
}

/// Paginated response wrapper.
#[derive(Debug, Serialize)]
pub struct MobilePageResponse<T: Serialize> {
    pub items: Vec<T>,
    pub page: u64,
    pub size: u64,
}

// ---------------------------------------------------------------------------
// Compact Producer / Parcel for list endpoints
// ---------------------------------------------------------------------------

/// Compact producer row for `GET /m/producers` (omit photo_url, gps, nin).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactProducer {
    pub id: Uuid,
    pub cooperative_id: Uuid,
    pub full_name: String,
    pub primary_crop: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
}

/// Compact parcel row for `GET /m/parcels`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactParcel {
    pub id: Uuid,
    pub producer_id: Uuid,
    pub crop_type: Option<String>,
    pub surface_hectares: Option<f64>,
    /// WKT geometry (compact text) — omit GeoJSON wrapper to save bytes.
    pub geom_wkt: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
}

// ---------------------------------------------------------------------------
// Sync batch DTOs
// ---------------------------------------------------------------------------

/// Body of `POST /m/sync/batch`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBatchRequest {
    /// Client-generated UUID — used by `service::idempotency` (KAYA, TTL 24h).
    pub batch_id: Uuid,
    /// Items to apply (max `SYNC_BATCH_MAX_ITEMS = 100`).
    pub items: Vec<SyncItem>,
}

/// One item in a sync batch — discriminated by `type` field on the wire.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SyncItem {
    /// Update parcel polygon via Yjs binary delta (base64).
    ParcelPolygonUpdate {
        parcel_id: Uuid,
        /// Base64-encoded Yjs v1 binary update.
        yjs_delta: String,
    },
    /// Update agronomy note via Yjs binary delta (base64).
    AgronomyNoteUpdate {
        parcel_id: Uuid,
        /// Optional note id — None when the note is being created for the first time.
        note_id: Option<Uuid>,
        yjs_delta: String,
    },
    /// LWW-style scalar producer patch (full name / phone / etc.).
    ProducerUpdate {
        producer_id: Uuid,
        lww_version: i64,
        /// Free-form JSON of fields to patch — passed through to terroir-core.
        patch: serde_json::Value,
    },
    /// LWW-style scalar parcel patch (crop type / surface / planted date).
    ParcelUpdate {
        parcel_id: Uuid,
        lww_version: i64,
        patch: serde_json::Value,
    },
    /// Update household via Yjs binary delta (base64).
    HouseholdUpdate {
        household_id: Uuid,
        yjs_delta: String,
    },
}

/// Response body for `POST /m/sync/batch` — one ack per submitted item.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBatchResponse {
    pub batch_id: Uuid,
    pub acks: Vec<SyncItemAck>,
}

/// Acknowledgement for a single batch item.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncItemAck {
    /// Index in the original `items` array (so the client can match acks).
    pub index: usize,
    /// `"ok"` on success, `"error"` otherwise.
    pub status: SyncItemStatus,
    /// Server-side LWW or Yjs version after merge — present when `status="ok"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_version: Option<i64>,
    /// Error category — present when `status="error"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Human-readable message — present when `status="error"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncItemStatus {
    Ok,
    Error,
}

// ---------------------------------------------------------------------------
// WebSocket frame DTOs
// ---------------------------------------------------------------------------

/// Message envelope sent over the WebSocket — text frames only (binary is
/// reserved for future Yjs raw bytes if/when the client opts in).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WsFrame {
    /// Lightweight ping the client may send anytime — server replies with `pong`.
    Ping,
    /// Server response to `ping`.
    Pong,
    /// Client → server (or server → other clients): a Yjs delta for a parcel polygon.
    YjsUpdate {
        parcel_id: Uuid,
        /// Base64-encoded Yjs v1 binary update.
        yjs_delta: String,
    },
    /// Server → client error frame (recoverable — connection stays open).
    Error { code: String, message: String },
}
