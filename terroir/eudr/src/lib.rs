// SPDX-License-Identifier: AGPL-3.0-or-later
//! terroir-eudr — bibliothèque.
//!
//! Validation parcelles vs Hansen Global Forest Change v1.11 (mirror MinIO)
//! et JRC Tropical Moist Forest v1_2024 (mirror MinIO). Génère les DDS
//! (Due Diligence Statement) signées via Vault PKI EORI exportateur,
//! soumises à TRACES NT.
//!
//! HTTP :8831, gRPC :8731 (cf. INFRA/port-policy.yaml).
//!
//! # Modules
//! - `dto`             : DTOs Serde request/response
//! - `errors`          : `AppError` + `IntoResponse`
//! - `events`          : Redpanda producer (terroir.parcel.eudr.* + terroir.dds.*)
//! - `grpc_client`     : Tonic client vers terroir-core :8730
//! - `grpc_server`     : Tonic `EudrService` server
//! - `repository`      : sqlx queries `terroir_t_<slug>.{eudr_validation, dds, dds_submission}`
//! - `routes`          : Axum router
//! - `service`         : logique métier (validator, hansen_reader, jrc_reader, dds_*)
//! - `state`           : `AppState` partagé
//! - `tenant_context`  : extracteur JWT / X-Tenant-Slug (mirror terroir-core)

#![forbid(unsafe_code)]

pub mod dto;
pub mod errors;
pub mod events;
pub mod grpc_client;
pub mod grpc_server;
pub mod repository;
pub mod routes;
pub mod service;
pub mod state;
pub mod tenant_context;

/// Generated tonic types from `proto/eudr.proto`.
pub mod eudr_proto {
    tonic::include_proto!("terroir.eudr.v1");
}

/// Generated tonic client types for `proto/core.proto` (re-used here).
pub mod core_proto {
    tonic::include_proto!("terroir.core.v1");
}

/// Returns the semantic version of the crate.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Default HTTP port (cf. `INFRA/port-policy.yaml`).
pub const HTTP_PORT: u16 = 8831;

/// Default gRPC port (cf. `INFRA/port-policy.yaml`).
pub const GRPC_PORT: u16 = 8731;

/// Default S3 bucket for geo mirror (Hansen + JRC tiles).
pub const GEO_MIRROR_BUCKET: &str = "geo-mirror";

/// Hansen GFC dataset version + path prefix.
pub const HANSEN_VERSION: &str = "v1.11";
pub const HANSEN_PREFIX: &str = "hansen-gfc/v1.11";

/// JRC TMF dataset version + path prefix.
pub const JRC_VERSION: &str = "v1_2024";
pub const JRC_PREFIX: &str = "jrc-tmf/v1_2024";

/// Hansen lossyear cut-off (year - 2000) for EUDR. `>= 21` means post-2020-12-31.
pub const HANSEN_CUTOFF_LOSSYEAR: u8 = 21;

/// Treecover2000 minimum percentage for a pixel to be considered "forest" initially.
pub const HANSEN_TREECOVER_MIN: u8 = 30;

/// Pixel threshold at which a parcel is REJECTED (Hansen 30 m → 100 px ≈ 9 ha).
pub const REJECT_PIXEL_THRESHOLD: u32 = 100;

/// Hansen pixel surface (km²) — 30 m × 30 m at equator (approximation).
pub const HANSEN_PIXEL_KM2: f64 = 0.0009;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }

    #[test]
    fn ports_are_correct() {
        assert_eq!(HTTP_PORT, 8831);
        assert_eq!(GRPC_PORT, 8731);
    }
}
