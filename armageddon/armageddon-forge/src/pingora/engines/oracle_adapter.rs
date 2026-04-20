// SPDX-License-Identifier: AGPL-3.0-or-later
//! ORACLE (ONNX anomaly detection, 22-feature model) adapter — stub.
//!
//! M3 sub-issue #104 · see tracker #108.
//! TODO(#104): port `armageddon_oracle::Oracle` behind this type.

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter for the ORACLE AI engine.
///
/// First-wave M3 scope provides a type-level stub returning
/// [`EngineVerdict::Skipped`].  Real ONNX runtime wiring is a larger
/// port and lands in its own PR.
pub struct OracleAdapter {
    // TODO(#104): hold `Arc<armageddon_oracle::Oracle>` here.
}

impl OracleAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for OracleAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EngineAdapter for OracleAdapter {
    fn name(&self) -> &'static str {
        "oracle"
    }

    async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
        // TODO(#104): invoke ONNX runtime with the 22-feature request
        // vector and threshold the anomaly score.
        EngineVerdict::Skipped
    }
}
