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

/// GET cached `ValidationResponse` for a polygon hash.
#[instrument(skip(kaya))]
pub async fn get_cached(
    kaya: &mut impl AsyncCommands,
    hash: &str,
) -> Result<Option<ValidationResponse>> {
    let key = cache_key(hash);
    let raw: Option<String> = kaya.get(&key).await.context("KAYA GET cache")?;
    match raw {
        Some(s) => {
            let parsed: ValidationResponse =
                serde_json::from_str(&s).context("decode cached ValidationResponse")?;
            debug!(hash = hash, "EUDR cache HIT");
            Ok(Some(parsed))
        }
        None => {
            debug!(hash = hash, "EUDR cache MISS");
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
    let payload = match serde_json::to_string(value) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "encode ValidationResponse for cache failed");
            return;
        }
    };
    if let Err(e) = kaya.set_ex::<_, _, ()>(&key, payload, ttl_secs).await {
        tracing::warn!(hash = hash, error = %e, "KAYA SET EUDR cache failed (non-fatal)");
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
