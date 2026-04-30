// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-core — bibliothèque
//
// Service principal TERROIR (registre membres + parcelles + ménages).
// HTTP Axum :8830 + gRPC Tonic :8730.
//
// Contrats publics :
//   - REST : POST /producers, GET /producers, PATCH /producers/{id}, ...
//   - gRPC : terroir.core.v1.CoreService (proto: ../proto/core.proto)
//
// Dépendances runtime :
//   - PostgreSQL+PostGIS (schema-per-tenant `terroir_t_<slug>`)
//   - KAYA (cache producteurs + idempotency keys)
//   - Kratos JWT (validation agent terrain)
//   - Keto (ABAC namespace `Tenant`/`Cooperative`/`Parcel`)
//   - Redpanda (publish `terroir.member.*`)
//   - audit-lib (append-only `audit_t_<slug>.audit_log`)
//
// Ce skeleton P0 expose uniquement `version()` + un health endpoint.

#![forbid(unsafe_code)]

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
}
