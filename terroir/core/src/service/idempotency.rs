// SPDX-License-Identifier: AGPL-3.0-or-later
//! Idempotency key service backed by KAYA RESP3.
//!
//! KAYA key format: `terroir:idempotent:{key}` — TTL 24h.
//! On first POST the key is SET NX (SET if Not eXists).
//! If the key already exists → 409 Conflict with the cached body.
//!
//! Callers:
//!   - `POST /producers` (idempotency key = `X-Idempotency-Key` header or body UUID)
//!   - `POST /parcels`
//!   - `POST /parts-sociales` (ACID, extra safety)

use anyhow::Result;
use redis::AsyncCommands;
use tracing::{debug, instrument};

const IDEMPOTENCY_TTL_SECS: u64 = 86_400; // 24 hours

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn idempotency_key(key: &str) -> String {
    format!("terroir:idempotent:{key}")
}

/// Check if an idempotency key was already processed.
/// Returns `true` if already processed (→ return cached response / 409).
#[instrument(skip(kaya))]
pub async fn is_duplicate(kaya: &mut impl AsyncCommands, key: &str) -> Result<bool> {
    let kaya_key = idempotency_key(key);
    let exists: bool = kaya
        .exists(&kaya_key)
        .await
        .map_err(|e| anyhow::anyhow!("KAYA exists: {e}"))?;
    debug!(key = key, duplicate = exists, "idempotency check");
    Ok(exists)
}

/// Mark an idempotency key as processed (SET EX 24h).
/// Silently ignores KAYA errors (non-fatal for correctness — DB uniqueness
/// constraints are the hard barrier).
#[instrument(skip(kaya))]
pub async fn mark_processed(kaya: &mut impl AsyncCommands, key: &str) {
    let kaya_key = idempotency_key(key);
    // Use raw SET with EX option for compatibility with KAYA RESP3.
    let res: redis::RedisResult<()> = redis::cmd("SET")
        .arg(&kaya_key)
        .arg("1")
        .arg("EX")
        .arg(IDEMPOTENCY_TTL_SECS)
        .query_async(kaya)
        .await;
    if let Err(e) = res {
        tracing::warn!(key = key, error = %e, "KAYA idempotency SET failed (non-fatal)");
    }
}
