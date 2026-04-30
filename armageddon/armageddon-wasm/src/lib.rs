// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! armageddon-wasm: WASM plugin runtime via Wasmtime.
//!
//! Allows loading custom security plugins as WebAssembly modules.
//! Plugins run in a sandboxed environment with controlled memory and time limits.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | `plugins_dir` absent | Runtime initialises as no-op; all requests pass through |
//! | Plugin missing required exports | Rejected at load time; logged at WARN |
//! | Fuel exhaustion during request | `Decision::Deny` (HTTP 500) |
//! | Plugin crash / trap | Fail-open: `Decision::Allow` |
//! | `init()` not called | `inspect()` runs with zero plugins (no-op) |

pub mod abi_v0_2_0;
pub mod host;
pub mod plugin;
pub mod runtime;

/// Proxy-wasm ABI v0.2.1 host runtime.
///
/// Used to load the Coraza WAF guest (`coraza-waf.wasm`).  Compiled only
/// when the `coraza-wasm` Cargo feature is enabled — in default builds
/// this module is absent and zero proxy-wasm v0.2.1 code is linked in.
///
/// See `armageddon/coraza/PROXY-WASM-HOST-DESIGN.md` for the architectural
/// rationale and the host-function bring-up roadmap.
#[cfg(feature = "coraza-wasm")]
pub mod proxy_wasm_v0_2_1;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::{Decision, Severity};
use armageddon_common::engine::SecurityEngine;
use armageddon_common::error::Result;
use armageddon_config::security::WasmConfig;
use async_trait::async_trait;

/// The WASM plugin runtime engine.
pub struct WasmRuntime {
    config: WasmConfig,
    runtime: runtime::PluginRuntime,
    ready: bool,
}

impl WasmRuntime {
    pub fn new(config: WasmConfig) -> Self {
        let runtime = runtime::PluginRuntime::new(
            config.max_memory_bytes,
            config.max_execution_time_ms,
        );
        Self {
            config,
            runtime,
            ready: false,
        }
    }
}

#[async_trait]
impl SecurityEngine for WasmRuntime {
    fn name(&self) -> &'static str {
        "WASM"
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!(
            plugins_dir = %self.config.plugins_dir,
            max_memory_bytes = self.config.max_memory_bytes,
            "WASM runtime initializing"
        );

        // Scan plugins_dir and load .wasm modules.
        // Silently succeeds when the directory is absent (no-op runtime).
        self.runtime.load_from_dir(&self.config.plugins_dir);

        tracing::info!(
            plugin_count = self.runtime.plugin_count(),
            "WASM runtime ready"
        );
        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        let results = self.runtime.run_plugins(ctx);

        // A single deny from any plugin → deny the request.
        for result in &results {
            if !result.allow {
                let reason = result
                    .message
                    .as_deref()
                    .unwrap_or("wasm_plugin_deny");
                tracing::warn!(
                    plugin = %result.plugin_name,
                    reason = %reason,
                    "WASM plugin denied request"
                );
                return Ok(Decision::deny(
                    self.name(),
                    "wasm_plugin_deny",
                    reason,
                    Severity::High,
                    start.elapsed().as_micros() as u64,
                ));
            }
        }

        Ok(Decision::allow(self.name(), start.elapsed().as_micros() as u64))
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("WASM runtime shutting down");
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }
}
