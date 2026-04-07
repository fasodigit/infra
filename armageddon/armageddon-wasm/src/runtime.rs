//! Wasmtime-based plugin runtime.

use armageddon_common::context::RequestContext;
use wasmtime::{Config, Engine};

/// A loaded WASM plugin.
pub struct LoadedPlugin {
    pub name: String,
    pub path: String,
    // module: wasmtime::Module,
}

/// Plugin execution result.
#[derive(Debug)]
pub struct PluginResult {
    pub plugin_name: String,
    pub allow: bool,
    pub message: Option<String>,
    pub score: f64,
}

/// Manages the Wasmtime engine and loaded plugins.
pub struct PluginRuntime {
    engine: Engine,
    plugins: Vec<LoadedPlugin>,
    max_memory_bytes: u64,
    max_execution_time_ms: u64,
}

impl PluginRuntime {
    pub fn new(max_memory_bytes: u64, max_execution_time_ms: u64) -> Self {
        let mut config = Config::new();
        config.consume_fuel(true); // Enable fuel-based execution limits
        config.epoch_interruption(true); // Enable epoch-based interruption

        let engine = Engine::new(&config).expect("failed to create Wasmtime engine");

        Self {
            engine,
            plugins: Vec::new(),
            max_memory_bytes,
            max_execution_time_ms,
        }
    }

    /// Load a WASM plugin from a file.
    pub fn load_plugin(&mut self, name: &str, path: &str) -> Result<(), String> {
        tracing::info!("loading WASM plugin '{}' from {}", name, path);
        // TODO: wasmtime::Module::from_file(&self.engine, path)
        self.plugins.push(LoadedPlugin {
            name: name.to_string(),
            path: path.to_string(),
        });
        Ok(())
    }

    /// Run all loaded plugins against a request.
    pub fn run_plugins(&self, _ctx: &RequestContext) -> Vec<PluginResult> {
        // TODO: for each plugin, create a Store with fuel limits,
        // instantiate the module, call the inspect function
        Vec::new()
    }

    /// Number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}
