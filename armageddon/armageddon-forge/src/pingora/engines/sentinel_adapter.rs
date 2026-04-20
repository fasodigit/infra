// SPDX-License-Identifier: AGPL-3.0-or-later
//! SENTINEL (IPS / DLP / GeoIP / JA3 / rate-limit) adapter — stub.
//!
//! M3 sub-issue #104 · see tracker #108.
//! TODO(#104): port `armageddon_sentinel::Sentinel` behind this type.

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter for the SENTINEL engine.
///
/// First-wave M3 scope (this commit) provides a type-level stub that
/// always returns [`EngineVerdict::Skipped`].  The second wave wires
/// the real Aho-Corasick / GeoIP / JA3 path.
pub struct SentinelAdapter {
    // TODO(#104): hold `Arc<armageddon_sentinel::Sentinel>` here.
}

impl SentinelAdapter {
    /// Construct a stub adapter.  Returns `Skipped` on every request
    /// until the real port lands.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SentinelAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EngineAdapter for SentinelAdapter {
    fn name(&self) -> &'static str {
        "sentinel"
    }

    async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
        // TODO(#104): port SENTINEL analysis (IPS signatures, DLP
        // patterns, GeoIP blocking, JA3 denylist, rate-limit token
        // bucket) into this adapter.
        EngineVerdict::Skipped
    }
}
