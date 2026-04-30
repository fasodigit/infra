// SPDX-License-Identifier: AGPL-3.0-or-later
//! KAYA RESP3 cache for EUDR validation outcomes.
//!
//! Key format: `terroir:eudr:result:{polygon_hash}` — 64-char SHA-256 hex.
//! Value: JSON-serialized `ValidationResponse`.
//! TTL: configurable (default 30 days, cf. ULTRAPLAN §6 P1.3).

use anyhow::{Context, Result};
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use tracing::{debug, instrument};

use crate::dto::ValidationResponse;

/// Compute SHA-256 of a normalized GeoJSON value.
///
/// Normalization: serialize via `serde_json::to_string` after parsing
/// (canonicalisation: object keys sorted by `serde_json::Value::Object`
/// already in insertion order — to keep determinism we re-serialize
/// from a tree where we recursively sort maps).
pub fn polygon_hash(geojson: &serde_json::Value) -> String {
    let canonical = canonicalize(geojson);
    let s = serde_json::to_string(&canonical).unwrap_or_else(|_| String::new());
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map
                .iter()
                .map(|(k, vv)| (k.clone(), canonicalize(vv)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(entries.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize).collect())
        }
        other => other.clone(),
    }
}

fn cache_key(hash: &str) -> String {
    format!("terroir:eudr:result:{hash}")
}

/// Cache key scoped per (tenant, parcel) so the MISS/HIT semantics align with
/// "first call for this specific parcel".  Two parcels with the same polygon
/// shape are still hashed identically → datasetVersion stable → polygonHash
/// stable, but the cache key is not shared.
pub fn parcel_cache_key(tenant_slug: &str, parcel_id: &uuid::Uuid, hash: &str) -> String {
    format!("terroir:eudr:result:{tenant_slug}:{parcel_id}:{hash}")
}

/// GET cached `ValidationResponse` for a polygon hash.
#[instrument(skip(kaya))]
pub async fn get_cached(
    kaya: &mut impl AsyncCommands,
    hash: &str,
) -> Result<Option<ValidationResponse>> {
    let key = cache_key(hash);
    get_cached_by_key(kaya, &key).await
}

/// GET cached using a fully-qualified key (already prefixed with namespace).
pub async fn get_cached_by_key(
    kaya: &mut impl AsyncCommands,
    key: &str,
) -> Result<Option<ValidationResponse>> {
    let raw: Option<String> = kaya.get(key).await.context("KAYA GET cache")?;
    match raw {
        Some(s) => {
            let parsed: ValidationResponse =
                serde_json::from_str(&s).context("decode cached ValidationResponse")?;
            debug!(key = key, "EUDR cache HIT");
            Ok(Some(parsed))
        }
        None => {
            debug!(key = key, "EUDR cache MISS");
            Ok(None)
        }
    }
}

/// SET cached `ValidationResponse` with TTL (best-effort).
#[instrument(skip(kaya, value, ttl_secs))]
pub async fn put_cached(
    kaya: &mut impl AsyncCommands,
    hash: &str,
    value: &ValidationResponse,
    ttl_secs: u64,
) {
    let key = cache_key(hash);
    put_cached_by_key(kaya, &key, value, ttl_secs).await;
}

/// SET cached using a fully-qualified key (already prefixed with namespace).
pub async fn put_cached_by_key(
    kaya: &mut impl AsyncCommands,
    key: &str,
    value: &ValidationResponse,
    ttl_secs: u64,
) {
    let payload = match serde_json::to_string(value) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "encode ValidationResponse for cache failed");
            return;
        }
    };
    // Use raw SET with EX option (more widely supported than SETEX, which
    // KAYA RESP3 may reject). Falls back to SET without TTL if EX rejected.
    let res: redis::RedisResult<()> = redis::cmd("SET")
        .arg(&key)
        .arg(&payload)
        .arg("EX")
        .arg(ttl_secs)
        .query_async(kaya)
        .await;
    if let Err(e) = res {
        tracing::warn!(key = key, error = %e, "KAYA SET EUDR cache failed (non-fatal)");
        // Fallback: plain SET without TTL.
        let _: redis::RedisResult<()> = kaya.set::<_, _, ()>(&key, payload).await.map(|_| ());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_hash_is_deterministic() {
        let p1 = serde_json::json!({"type":"Polygon","coordinates":[[[1,2],[3,4]]]});
        let p2 = serde_json::json!({"coordinates":[[[1,2],[3,4]]],"type":"Polygon"});
        assert_eq!(polygon_hash(&p1), polygon_hash(&p2));
        assert_eq!(polygon_hash(&p1).len(), 64);
    }
}
