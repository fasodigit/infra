// SPDX-License-Identifier: AGPL-3.0-or-later
//! WASM (Wasmtime plugin runtime) adapter — stub.
//!
//! M3 sub-issue #104 · see tracker #108.
//! TODO(#104): port `armageddon_wasm::WasmEngine` behind this type.

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

/// Pipeline adapter exposing user-supplied WASM plugins as a single
/// engine.
pub struct WasmAdapter {
    // TODO(#104): hold `Arc<armageddon_wasm::WasmEngine>` here.
}

impl WasmAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for WasmAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EngineAdapter for WasmAdapter {
    fn name(&self) -> &'static str {
        "wasm"
    }

    async fn analyze(&self, _ctx: &mut RequestCtx) -> EngineVerdict {
        // TODO(#104): run registered Wasmtime plugins against the
        // request and aggregate their verdicts.
        EngineVerdict::Skipped
    }
}
