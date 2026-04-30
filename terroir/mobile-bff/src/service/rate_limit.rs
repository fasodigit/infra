// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-userId rate-limiter — KAYA-backed fixed-window bucket.
//!
//! Key format `terroir:mobile:rl:{user_id}` — value is a counter, expires
//! after `WINDOW_SECS = 60`. The first `INCR` on a new key bumps it to 1
//! and we set `EXPIRE 60`. While the count stays `<= RATE_LIMIT_RPM`, the
//! request is allowed; over the limit, the request is rejected with HTTP
//! 429 Too Many Requests.
//!
//! This is a deliberately simple fixed-window limiter. Sliding-window or
//! leaky-bucket can be substituted later (KAYA streams support both) — the
//! call sites only need `is_allowed(user_id) -> bool` semantics.

use anyhow::Result;
use redis::AsyncCommands;
use tracing::instrument;

const WINDOW_SECS: u64 = 60;

fn key(user_id: &str) -> String {
    format!("terroir:mobile:rl:{user_id}")
}

/// Returns `Ok(true)` if the request is within the budget, `Ok(false)` otherwise.
///
/// On KAYA failure, returns `Ok(true)` (fail-open) — better to serve a few
/// requests over budget than to outage the whole BFF when KAYA flaps.
#[instrument(skip(kaya))]
pub async fn is_allowed(kaya: &mut impl AsyncCommands, user_id: &str) -> Result<bool> {
    let k = key(user_id);
    // INCR returns the new value; KAYA initializes to 1 on first call.
    let count: i64 = match kaya.incr(&k, 1i64).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, user_id, "KAYA INCR failed — fail-open");
            return Ok(true);
        }
    };

    // Set TTL on the first INCR (count == 1).
    if count == 1
        && let Err(e) = kaya.expire::<_, ()>(&k, WINDOW_SECS as i64).await
    {
        tracing::warn!(error = %e, user_id, "KAYA EXPIRE failed — bucket may live forever");
    }

    Ok(count <= crate::RATE_LIMIT_RPM as i64)
}
