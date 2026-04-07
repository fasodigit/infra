//! armageddon-wasm: WASM plugin runtime via Wasmtime.
//!
//! Allows loading custom security plugins as WebAssembly modules.
//! Plugins run in a sandboxed environment with controlled memory and time limits.

pub mod host;
pub mod plugin;
pub mod runtime;

use armageddon_common::context::RequestContext;
use armageddon_common::decision::Decision;
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
            "WASM runtime initializing (plugins from {}, max memory: {} bytes)",
            self.config.plugins_dir,
            self.config.max_memory_bytes,
        );
        // TODO: scan plugins_dir and load .wasm modules
        self.ready = true;
        Ok(())
    }

    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision> {
        let start = std::time::Instant::now();

        // Run all loaded plugins against the request
        let _results = self.runtime.run_plugins(ctx);

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
