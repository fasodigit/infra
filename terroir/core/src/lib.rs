// SPDX-License-Identifier: AGPL-3.0-or-later
//! terroir-core — bibliothèque principale.
//!
//! Service central TERROIR (registre membres + parcelles + ménages).
//! HTTP Axum :8830 + gRPC Tonic :8730.
//!
//! # Modules
//! - `dto`            : DTOs Serde request/response
//! - `model`          : entités sqlx (rows PG)
//! - `errors`         : `AppError` + `IntoResponse`
//! - `state`          : `AppState` partagé entre handlers
//! - `tenant_context` : extracteur JWT / X-Tenant-Slug
//! - `service`        : logique métier (producer, parcel, vault, audit…)
//! - `events`         : Redpanda producer typé
//! - `routes`         : Axum router
//! - `grpc`           : Tonic `CoreService` implementation
//!
//! # Architecture
//! - PostgreSQL schema-per-tenant `terroir_t_<slug>` (ADR-006)
//! - PII chiffrés via Vault Transit DEK envelope (ADR-005)
//! - LWW pour scalaires producer/parcel, CRDT Yjs pour polygone/notes (ADR-002)
//! - KAYA RESP3 pour cache DEK (TTL 1h) + idempotency keys (TTL 24h)
//! - Redpanda pour events terroir.member.* / terroir.parcel.*

#![forbid(unsafe_code)]

pub mod dto;
pub mod errors;
pub mod events;
pub mod grpc;
pub mod model;
pub mod routes;
pub mod service;
pub mod state;
pub mod tenant_context;

/// Retourne la version sémantique du crate (lue depuis Cargo.toml).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Port HTTP par défaut (cf. `INFRA/port-policy.yaml`).
pub const HTTP_PORT: u16 = 8830;

/// Port gRPC par défaut (cf. `INFRA/port-policy.yaml`).
pub const GRPC_PORT: u16 = 8730;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }

    #[test]
    fn ports_are_correct() {
        assert_eq!(HTTP_PORT, 8830);
        assert_eq!(GRPC_PORT, 8730);
    }
}
