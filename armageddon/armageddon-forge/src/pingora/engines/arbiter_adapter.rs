// SPDX-License-Identifier: AGPL-3.0-or-later
//! ARBITER (OWASP CRS v4 WAF, Aho-Corasick) adapter — stub.
//!
//! M3 sub-issue #104 · see tracker #108.
//! TODO(#104): port `armageddon_arbiter::Arbiter` behind this type.

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter for the ARBITER WAF engine.
///
/// First-wave M3 scope provides a type-level stub returning
/// [`EngineVerdict::Skipped`].  Real wiring lands in the follow-up PR
/// that ports the Aho-Corasick scanner + CRS ruleset.
pub struct ArbiterAdapter {
    // TODO(#104): hold `Arc<armageddon_arbiter::Arbiter>` here.
}

impl ArbiterAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ArbiterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EngineAdapter for ArbiterAdapter {
    fn name(&self) -> &'static str {
        "arbiter"
    }

    async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
        // TODO(#104): port OWASP CRS v4 evaluation (Aho-Corasick body /
        // header / query scan) into this adapter.
        EngineVerdict::Skipped
    }
}
