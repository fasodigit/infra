// SPDX-License-Identifier: AGPL-3.0-or-later
//! sqlx row types for terroir-core.
//!
//! Each struct maps directly to a DB table row via `sqlx::FromRow`.
//! PII fields are stored as `Option<Vec<u8>>` (bytea) + a DEK kid reference.
//! The service layer calls `VaultTransitService::decrypt` to obtain plaintext.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Producer
// ---------------------------------------------------------------------------

/// Represents a row from `terroir_t_<slug>.producer`.
/// PII columns are stored encrypted (ADR-005).
#[derive(Debug, Clone, FromRow)]
pub struct ProducerRow {
    pub id: Uuid,
    pub cooperative_id: Uuid,
    pub external_id: Option<String>,

    // PII — bytea ciphertext + Vault DEK kid.
    pub full_name_encrypted: Option<Vec<u8>>,
    pub full_name_dek_kid: Option<String>,
    pub nin_encrypted: Option<Vec<u8>>,
    pub nin_dek_kid: Option<String>,
    pub phone_encrypted: Option<Vec<u8>>,
    pub phone_dek_kid: Option<String>,
    pub photo_url_encrypted: Option<Vec<u8>>,
    pub photo_url_dek_kid: Option<String>,
    pub gps_domicile_lat_encrypted: Option<Vec<u8>>,
    pub gps_domicile_lat_dek_kid: Option<String>,
    pub gps_domicile_lon_encrypted: Option<Vec<u8>>,
    pub gps_domicile_lon_dek_kid: Option<String>,

    pub household_id: Option<Uuid>,
    pub primary_crop: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Parcel
// ---------------------------------------------------------------------------

/// Represents a row from `terroir_t_<slug>.parcel`.
#[derive(Debug, Clone, FromRow)]
pub struct ParcelRow {
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
// ParcelPolygon
// ---------------------------------------------------------------------------

/// Represents a row from `terroir_t_<slug>.parcel_polygon`.
/// `geom_wkt` is computed via ST_AsText on read (PostGIS).
#[derive(Debug, Clone, FromRow)]
pub struct ParcelPolygonRow {
    pub parcel_id: Uuid,
    /// Raw Yjs CRDT binary document.
    pub yjs_doc: Vec<u8>,
    pub yjs_version: i64,
    pub updated_at: DateTime<Utc>,
    /// WKT representation, produced by ST_AsText at query time.
    pub geom_wkt: Option<String>,
}

// ---------------------------------------------------------------------------
// Household
// ---------------------------------------------------------------------------

/// Represents a row from `terroir_t_<slug>.household`.
#[derive(Debug, Clone, FromRow)]
pub struct HouseholdRow {
    pub id: Uuid,
    pub cooperative_id: Uuid,
    pub head_producer_id: Option<Uuid>,
    /// Yjs CRDT document encoding member list.
    pub yjs_doc: Vec<u8>,
    pub yjs_version: i64,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// PartsSociales
// ---------------------------------------------------------------------------

/// Represents a row from `terroir_t_<slug>.parts_sociales`.
#[derive(Debug, Clone, FromRow)]
pub struct PartsSocialesRow {
    pub id: Uuid,
    pub producer_id: Uuid,
    pub cooperative_id: Uuid,
    pub nb_parts: i32,
    /// Stored as DECIMAL(12,2) in PG; decoded via sqlx as f64.
    pub valeur_nominale_xof: f64,
    pub adhesion_date: NaiveDate,
    pub ag_reference: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub lww_version: i64,
}

// ---------------------------------------------------------------------------
// AgronomyNote
// ---------------------------------------------------------------------------

/// Represents a row from `terroir_t_<slug>.agronomy_note`.
#[derive(Debug, Clone, FromRow)]
pub struct AgronomyNoteRow {
    pub id: Uuid,
    pub parcel_id: Uuid,
    /// Yjs text CRDT document.
    pub yjs_doc: Vec<u8>,
    pub yjs_version: i64,
    pub updated_at: DateTime<Utc>,
}
