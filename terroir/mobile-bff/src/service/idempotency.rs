// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-batch idempotency for `POST /m/sync/batch` — KAYA-backed.
//!
//! Key format `terroir:mobile:idempotent:{batch_id}` — TTL 24h.
//! Mobile clients may retry a batch upload over flaky networks; without
//! idempotency this would double-apply Yjs deltas (still convergent for
//! Yjs but wasteful) and double-bump LWW versions (incorrect).

use anyhow::Result;
use redis::AsyncCommands;
use tracing::{debug, instrument};
use uuid::Uuid;

const IDEMPOTENCY_TTL_SECS: u64 = 86_400;

fn key(batch_id: &Uuid) -> String {
    format!("terroir:mobile:idempotent:{batch_id}")
}

/// Returns `true` if this batch was already processed.
#[instrument(skip(kaya))]
pub async fn is_duplicate(kaya: &mut impl AsyncCommands, batch_id: &Uuid) -> Result<bool> {
    let exists: bool = kaya
        .exists(key(batch_id))
        .await
        .map_err(|e| anyhow::anyhow!("KAYA exists: {e}"))?;
    debug!(batch_id = %batch_id, duplicate = exists, "idempotency check");
    Ok(exists)
}

/// Mark a batch as processed (SET EX 24h). Best-effort.
#[instrument(skip(kaya))]
pub async fn mark_processed(kaya: &mut impl AsyncCommands, batch_id: &Uuid) {
    if let Err(e) = kaya
        .set_ex::<_, _, ()>(key(batch_id), "1", IDEMPOTENCY_TTL_SECS)
        .await
    {
        tracing::warn!(batch_id = %batch_id, error = %e, "KAYA idempotency SET failed (non-fatal)");
    }
}
