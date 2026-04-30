// SPDX-License-Identifier: AGPL-3.0-or-later
//! Service layer for terroir-mobile-bff.
//!
//! - `idempotency`  : KAYA `terroir:mobile:idempotent:{batch_id}` (TTL 24h)
//! - `rate_limit`   : KAYA `terroir:mobile:rl:{user_id}` (60 rpm)
//! - `sync_engine`  : dispatch a sync batch by item type → terroir-core gRPC

pub mod idempotency;
pub mod rate_limit;
pub mod sync_engine;
