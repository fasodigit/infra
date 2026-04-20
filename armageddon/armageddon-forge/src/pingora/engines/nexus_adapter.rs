// SPDX-License-Identifier: AGPL-3.0-or-later
//! NEXUS (brain — composite-score aggregator) adapter — stub.
//!
//! M3 sub-issue #104 · see tracker #108.
//! TODO(#104): port `armageddon_nexus::Nexus` behind this type.
//!
//! NEXUS is conceptually downstream of every other engine — it
//! aggregates their individual decisions.  The pipeline orchestrator in
//! `pipeline.rs` already performs a `max()` aggregation; the NEXUS
//! adapter will later layer on weighted scoring, correlation windows,
//! and challenge/throttle actions.

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

pub struct NexusAdapter {
    // TODO(#104): hold `Arc<armageddon_nexus::Nexus>` here.
}

impl NexusAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for NexusAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EngineAdapter for NexusAdapter {
    fn name(&self) -> &'static str {
        "nexus"
    }

    async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
        // TODO(#104): port the NEXUS composite scorer (weighted sum of
        // per-engine decisions, correlation-window de-duplication).
        EngineVerdict::Skipped
    }
}
