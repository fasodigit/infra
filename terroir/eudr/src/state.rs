// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared application state for terroir-eudr.

use std::sync::Arc;

use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::service::hansen_reader::HansenReader;
use crate::service::jrc_reader::JrcReader;
use crate::tenant_context::JwksCache;

#[cfg(feature = "kafka")]
use crate::events::EventProducer;

/// Configuration values resolved at boot time.
#[derive(Debug, Clone)]
pub struct EudrSettings {
    /// KAYA TTL for `terroir:eudr:result:{hash}` (seconds).
    pub cache_ttl_secs: u64,
    /// Vault address (e.g. `http://localhost:8200`).
    pub vault_addr: String,
    /// Vault token used by the EUDR service (PKI write + KV read).
    pub vault_token: String,
    /// Vault PKI path that issues EORI exporter certs.
    /// Default: `pki-terroir/issue/eori-exporter`.
    pub vault_pki_role: String,
    /// Default EORI used when callers omit it.
    pub default_eori: String,
    /// MinIO/S3 evidence bucket prefix template — `terroir-evidence-<slug>`.
    pub evidence_bucket_prefix: String,
    /// TRACES NT submission URL (mock in P1).
    pub traces_nt_url: String,
    /// terroir-core gRPC URL (e.g. `http://localhost:8730`).
    pub core_grpc_url: String,
}

impl EudrSettings {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            cache_ttl_secs: std::env::var("TERROIR_EUDR_CACHE_TTL_DAYS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(30)
                * 24
                * 3600,
            vault_addr: std::env::var("VAULT_ADDR")
                .unwrap_or_else(|_| "http://localhost:8200".into()),
            vault_token: std::env::var("VAULT_TOKEN").unwrap_or_default(),
            vault_pki_role: std::env::var("VAULT_PKI_ROLE")
                .unwrap_or_else(|_| "pki-terroir/issue/eori-exporter".into()),
            default_eori: std::env::var("EUDR_DEFAULT_EORI")
                .unwrap_or_else(|_| "BF1234567890".into()),
            evidence_bucket_prefix: std::env::var("EUDR_EVIDENCE_BUCKET_PREFIX")
                .unwrap_or_else(|_| "terroir-evidence".into()),
            traces_nt_url: std::env::var("TRACES_NT_URL")
                .unwrap_or_else(|_| "http://localhost:9999/mock-traces-nt".into()),
            core_grpc_url: std::env::var("TERROIR_CORE_GRPC_URL")
                .unwrap_or_else(|_| "http://localhost:8730".into()),
        })
    }
}

/// Shared state for all handlers.
pub struct AppState {
    /// PostgreSQL connection pool.
    pub pg: Arc<PgPool>,
    /// KAYA RESP3 connection manager.
    pub kaya: ConnectionManager,
    /// Hansen GFC tile reader (S3/MinIO + LRU cache).
    pub hansen: Arc<HansenReader>,
    /// JRC TMF tile reader (S3/MinIO + LRU cache).
    pub jrc: Arc<JrcReader>,
    /// JWKS cache for JWT validation (auth-ms :8801).
    pub jwks_cache: Arc<JwksCache>,
    /// Shared HTTP client (Vault PKI + TRACES NT + JWKS).
    pub http_client: reqwest::Client,
    /// Resolved boot settings.
    pub settings: EudrSettings,
    /// Redpanda event producer (optional if kafka feature is disabled).
    #[cfg(feature = "kafka")]
    pub events: Arc<EventProducer>,
}
