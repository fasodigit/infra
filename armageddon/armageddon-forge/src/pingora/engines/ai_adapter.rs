// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI (threat-intel + prompt-injection detection) adapter — stub.
//!
//! M3 sub-issue #104 · see tracker #108.
//! TODO(#104): port `armageddon_ai::Ai` behind this type.

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter for the AI engine (LLM-driven threat intel + prompt
/// injection scanning).
pub struct AiAdapter {
    // TODO(#104): hold `Arc<armageddon_ai::Ai>` here.
}

impl AiAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EngineAdapter for AiAdapter {
    fn name(&self) -> &'static str {
        "ai"
    }

    async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
        // TODO(#104): port threat-intel lookups (IoC feeds) and
        // prompt-injection classifier.
        EngineVerdict::Skipped
    }
}
