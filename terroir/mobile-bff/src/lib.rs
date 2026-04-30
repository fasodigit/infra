// SPDX-License-Identifier: AGPL-3.0-or-later
//! terroir-mobile-bff — bibliothèque
//!
//! BFF orienté app mobile RN (Expo) : pagination légère, batch sync,
//! merge Yjs CRDT côté serveur, broadcast aux clients via WebSocket.
//!
//! # Modules
//! - `dto`             : DTOs Serde request/response (compact, mobile-friendly)
//! - `errors`          : `BffError` + `IntoResponse`
//! - `grpc_client`     : Tonic client pool vers terroir-core :8730
//! - `routes`          : Axum router REST `/m/*` + WebSocket `/ws/sync/{producerId}`
//! - `service`         : sync_engine (dispatch batch), idempotency, rate_limit
//! - `state`           : `AppState` partagé entre handlers
//! - `tenant_context`  : extracteur JWT (réutilise pattern terroir-core P1.A)
//! - `ws`              : registry + handler WebSocket avec broadcast tenant
//!
//! # Architecture
//! - Axum HTTP :8833 (REST mobile-optimized payloads + WebSocket upgrade)
//! - Mobile RN ↔ mobile-bff (REST + WS) ↔ terroir-core (gRPC :8730)
//! - KAYA RESP3 :6380 pour idempotency batch + rate-limit per-userId
//! - PostgreSQL read-replica via sqlx (lecture rapide pour `/m/producers`, `/m/parcels`)
//! - Compression Brotli/gzip négociée via `Accept-Encoding` (réseau EDGE/2G)
//!
//! # WebSocket flow (cf. ADR-002 sync conflict resolution)
//! 1. Client RN se connecte `ws://.../ws/sync/{producerId}` avec
//!    `Sec-WebSocket-Protocol: bearer.<jwt>` (pattern ARMAGEDDON).
//! 2. JWT validé → `TenantContext` extrait → registry indexe `(tenant, userId, ws)`.
//! 3. Sur réception update Yjs (frame texte JSON `{type:"yjs-update",...}`)
//!    → merge via gRPC terroir-core `GetParcelPolygon` + apply_update.
//! 4. Broadcast aux autres clients du même tenant.
//! 5. Heartbeat ping/pong 30s, idle timeout 5min.

#![forbid(unsafe_code)]

pub mod dto;
pub mod errors;
pub mod grpc_client;
pub mod routes;
pub mod service;
pub mod state;
pub mod tenant_context;
pub mod ws;

/// Generated Tonic client stubs for `terroir.core.v1.CoreService`.
pub mod terroir_core_grpc {
    tonic::include_proto!("terroir.core.v1");
}

/// Retourne la version sémantique du crate (lue depuis Cargo.toml).
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Port HTTP par défaut (cf. `INFRA/port-policy.yaml`).
pub const HTTP_PORT: u16 = 8833;

/// Max nombre d'items dans un `POST /m/sync/batch` (réseau EDGE/2G).
pub const SYNC_BATCH_MAX_ITEMS: usize = 100;

/// Pagination défaut/max (compactes pour mobile vs web admin 50/200).
pub const PAGE_SIZE_DEFAULT: u64 = 20;
pub const PAGE_SIZE_MAX: u64 = 100;

/// Heartbeat WebSocket — ping toutes les 30s.
pub const WS_HEARTBEAT_SECS: u64 = 30;

/// Idle timeout WebSocket — déconnexion après 5min sans pong.
pub const WS_IDLE_TIMEOUT_SECS: u64 = 300;

/// Rate-limit par userId (KAYA bucket) : 60 req/min.
pub const RATE_LIMIT_RPM: u32 = 60;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }

    #[test]
    fn ports_are_correct() {
        assert_eq!(HTTP_PORT, 8833);
    }

    #[test]
    fn batch_limits_sane() {
        const _: () = assert!(SYNC_BATCH_MAX_ITEMS >= 10 && SYNC_BATCH_MAX_ITEMS <= 1000);
        const _: () = assert!(PAGE_SIZE_MAX >= PAGE_SIZE_DEFAULT);
    }
}
