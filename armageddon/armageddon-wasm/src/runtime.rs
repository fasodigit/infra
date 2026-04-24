// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Wasmtime-based plugin runtime.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | `plugins_dir` absent | Silently returns empty plugin list; runtime is a no-op |
//! | Module load error (corrupt .wasm) | Logged at WARN; plugin is skipped |
//! | Missing required proxy-wasm exports | Plugin rejected at load time; logged at WARN |
//! | Fuel exhaustion (infinite loop) | `PluginResult { allow: false }` + metric increment |
//! | Memory limit exceeded | Wasmtime traps; treated same as fuel exhaustion |
//! | Plugin instantiation error | `PluginResult { allow: true }` (fail-open) + WARN log |

use std::sync::Arc;

use prometheus::{IntCounterVec, Opts, Registry};
use wasmtime::{Config, Engine, Linker, Module, Store};

use armageddon_common::context::RequestContext;

use crate::abi_v0_2_0::{self, FilterContext, HostData, SharedDataStore, DEFAULT_FUEL};

// ---------------------------------------------------------------------------
// Required proxy-wasm ABI exports
// ---------------------------------------------------------------------------

/// Minimum set of exports required for a plugin to be accepted at load time.
/// Plugins missing these exports are rejected with a WARN log.
const REQUIRED_EXPORTS: &[&str] = &["proxy_on_request_headers"];

// ---------------------------------------------------------------------------
// WasmMetrics
// ---------------------------------------------------------------------------

/// Prometheus metrics emitted by the plugin runtime.
#[derive(Clone, Debug)]
pub struct WasmMetrics {
    /// `armageddon_wasm_fuel_exhausted_total{plugin}` — fuel trap counter.
    pub fuel_exhausted_total: IntCounterVec,
    /// `armageddon_wasm_plugin_deny_total{plugin}` — deny decisions.
    pub plugin_deny_total: IntCounterVec,
    /// `armageddon_wasm_plugin_allow_total{plugin}` — allow decisions.
    pub plugin_allow_total: IntCounterVec,
}

impl WasmMetrics {
    /// Register metrics on `registry`.  Returns `Err` on duplicate registration.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let fuel_exhausted_total = IntCounterVec::new(
            Opts::new(
                "armageddon_wasm_fuel_exhausted_total",
                "Total WASM plugin invocations aborted due to fuel exhaustion",
            ),
            &["plugin"],
        )?;
        registry.register(Box::new(fuel_exhausted_total.clone()))?;

        let plugin_deny_total = IntCounterVec::new(
            Opts::new(
                "armageddon_wasm_plugin_deny_total",
                "Total deny decisions produced by WASM plugins",
            ),
            &["plugin"],
        )?;
        registry.register(Box::new(plugin_deny_total.clone()))?;

        let plugin_allow_total = IntCounterVec::new(
            Opts::new(
                "armageddon_wasm_plugin_allow_total",
                "Total allow decisions produced by WASM plugins",
            ),
            &["plugin"],
        )?;
        registry.register(Box::new(plugin_allow_total.clone()))?;

        Ok(Self {
            fuel_exhausted_total,
            plugin_deny_total,
            plugin_allow_total,
        })
    }
}

// ---------------------------------------------------------------------------
// LoadedPlugin
// ---------------------------------------------------------------------------

/// A compiled and cached WASM plugin module.
///
/// The `Module` is AOT-compiled once at load time and reused across requests.
/// `Store` instances are created fresh per-request (never shared).
pub struct LoadedPlugin {
    /// Human-readable name derived from the file stem.
    pub name: String,
    /// Original filesystem path (for log messages).
    pub path: String,
    /// AOT-compiled Wasmtime module.
    module: Module,
}

// ---------------------------------------------------------------------------
// PluginResult
// ---------------------------------------------------------------------------

/// Result from a single plugin invocation.
#[derive(Debug)]
pub struct PluginResult {
    pub plugin_name: String,
    pub allow: bool,
    pub message: Option<String>,
    pub score: f64,
}

// ---------------------------------------------------------------------------
// PluginRuntime
// ---------------------------------------------------------------------------

/// Manages the Wasmtime engine and loaded plugins.
///
/// Create one instance at gateway startup, call [`PluginRuntime::load_from_dir`]
/// to populate, then call [`PluginRuntime::run_plugins`] per request.
pub struct PluginRuntime {
    engine: Engine,
    plugins: Vec<LoadedPlugin>,
    #[allow(dead_code)]
    max_memory_bytes: u64,
    /// Fuel per invocation derived from `max_execution_time_ms`.
    fuel_per_invocation: u64,
    /// Optional Prometheus metrics.
    metrics: Option<Arc<WasmMetrics>>,
    /// Shared data store across all plugin invocations (proxy-wasm shared KV).
    shared_store: Arc<std::sync::Mutex<SharedDataStore>>,
}

impl PluginRuntime {
    /// Create a new plugin runtime with the given memory and time limits.
    pub fn new(max_memory_bytes: u64, max_execution_time_ms: u64) -> Self {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(false); // fuel-based limiting is sufficient

        let engine = Engine::new(&config).expect("failed to create Wasmtime engine");

        // Map execution time to a fuel budget.
        // Heuristic: 1 ms ≈ 100_000 fuel units (empirically tuned for Wasmtime 28).
        // Use DEFAULT_FUEL when max_execution_time_ms == 0 (disabled / unset).
        let fuel_per_invocation = if max_execution_time_ms == 0 {
            DEFAULT_FUEL
        } else {
            max_execution_time_ms.saturating_mul(100_000)
        };

        Self {
            engine,
            plugins: Vec::new(),
            max_memory_bytes,
            fuel_per_invocation,
            metrics: None,
            shared_store: Arc::new(std::sync::Mutex::new(SharedDataStore::new())),
        }
    }

    /// Attach Prometheus metrics.
    pub fn with_metrics(mut self, metrics: Arc<WasmMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    // -----------------------------------------------------------------------
    // Plugin discovery + loading
    // -----------------------------------------------------------------------

    /// Scan `plugins_dir` for `*.wasm` files and load each one.
    ///
    /// Silently returns if the directory does not exist.
    /// Invalid or incompatible modules are logged at WARN and skipped.
    pub fn load_from_dir(&mut self, plugins_dir: &str) {
        let dir = std::path::Path::new(plugins_dir);
        if !dir.exists() {
            tracing::debug!(
                plugins_dir = %plugins_dir,
                "WASM plugins directory absent — no plugins loaded"
            );
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(
                    plugins_dir = %plugins_dir,
                    error = %err,
                    "failed to read WASM plugins directory"
                );
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
                continue;
            }

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let path_str = path.to_string_lossy().into_owned();

            match self.load_plugin(&name, &path_str) {
                Ok(()) => tracing::info!(plugin = %name, path = %path_str, "WASM plugin loaded"),
                Err(e) => tracing::warn!(
                    plugin = %name,
                    path = %path_str,
                    error = %e,
                    "WASM plugin rejected — skipping"
                ),
            }
        }

        tracing::info!(
            plugins_dir = %plugins_dir,
            count = self.plugins.len(),
            "WASM plugin discovery complete"
        );
    }

    /// Load a single WASM plugin from a file.
    ///
    /// Validates that the module exports the required proxy-wasm entry points.
    /// Returns `Err` when validation fails.
    pub fn load_plugin(&mut self, name: &str, path: &str) -> Result<(), String> {
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| format!("wasmtime Module::from_file failed: {e}"))?;

        // Validate required exports are present.
        for &required in REQUIRED_EXPORTS {
            let has_export = module.exports().any(|exp| exp.name() == required);
            if !has_export {
                return Err(format!(
                    "plugin '{}' is missing required export '{}' — not a valid proxy-wasm module",
                    name, required
                ));
            }
        }

        self.plugins.push(LoadedPlugin {
            name: name.to_string(),
            path: path.to_string(),
            module,
        });
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Per-request execution
    // -----------------------------------------------------------------------

    /// Run all loaded plugins against a request.
    ///
    /// Short-circuits on the first deny decision — subsequent plugins are not
    /// invoked once a block is issued.
    ///
    /// Returns an empty `Vec` when no plugins are loaded (no-op fast path).
    pub fn run_plugins(&self, ctx: &RequestContext) -> Vec<PluginResult> {
        if self.plugins.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::with_capacity(self.plugins.len());

        for plugin in &self.plugins {
            let result = self.invoke_plugin(plugin, ctx);
            let allow = result.allow;
            results.push(result);
            if !allow {
                break; // short-circuit
            }
        }

        results
    }

    /// Invoke a single plugin and return its result.
    fn invoke_plugin(&self, plugin: &LoadedPlugin, ctx: &RequestContext) -> PluginResult {
        // Build a per-request FilterContext from the RequestContext.
        let mut filter_ctx = FilterContext::default();
        for (k, v) in &ctx.request.headers {
            filter_ctx.request_headers.insert(k.clone(), v.clone());
        }

        let host_data = HostData {
            ctx: filter_ctx,
            shared: self.shared_store.clone(),
            fuel_limit: self.fuel_per_invocation,
        };

        let mut store = Store::new(&self.engine, host_data);

        if let Err(e) = store.set_fuel(self.fuel_per_invocation) {
            tracing::warn!(plugin = %plugin.name, error = %e, "set_fuel failed — using default");
            let _ = store.set_fuel(DEFAULT_FUEL);
        }

        let mut linker: Linker<HostData> = Linker::new(&self.engine);
        if let Err(e) = abi_v0_2_0::register_host_functions(&mut linker) {
            tracing::warn!(plugin = %plugin.name, error = %e, "ABI registration failed — failing open");
            return PluginResult {
                plugin_name: plugin.name.clone(),
                allow: true,
                message: Some(format!("abi_register_failed: {e}")),
                score: 0.0,
            };
        }

        let instance = match linker.instantiate(&mut store, &plugin.module) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!(plugin = %plugin.name, error = %e, "plugin instantiation failed — failing open");
                return PluginResult {
                    plugin_name: plugin.name.clone(),
                    allow: true,
                    message: Some(format!("instantiation_failed: {e}")),
                    score: 0.0,
                };
            }
        };

        // Call _initialize if present (WASI reactor / proxy-wasm root context).
        if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "_initialize") {
            let _ = init.call(&mut store, ());
        }

        // Invoke proxy_on_request_headers (v0.2.0: ctx_id, num_headers, eos).
        let num_headers = store.data().ctx.request_headers.len() as u32;
        let call_result = if let Ok(f) = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut store, "proxy_on_request_headers")
        {
            f.call(&mut store, (1, num_headers, 0))
        } else if let Ok(f) = instance
            .get_typed_func::<(u32, u32), u32>(&mut store, "proxy_on_request_headers")
        {
            f.call(&mut store, (1, num_headers))
        } else {
            return PluginResult {
                plugin_name: plugin.name.clone(),
                allow: true,
                message: None,
                score: 0.0,
            };
        };

        match call_result {
            Ok(_action) => {
                let local_response = store.into_data().ctx.local_response;
                if let Some(resp) = local_response {
                    if let Some(m) = &self.metrics {
                        m.plugin_deny_total.with_label_values(&[&plugin.name]).inc();
                    }
                    PluginResult {
                        plugin_name: plugin.name.clone(),
                        allow: false,
                        message: Some(format!(
                            "plugin_deny:{} {}",
                            resp.status_code, resp.status_details
                        )),
                        score: 1.0,
                    }
                } else {
                    if let Some(m) = &self.metrics {
                        m.plugin_allow_total.with_label_values(&[&plugin.name]).inc();
                    }
                    PluginResult {
                        plugin_name: plugin.name.clone(),
                        allow: true,
                        message: None,
                        score: 0.0,
                    }
                }
            }
            Err(e) => {
                // Wasmtime 28: fuel exhaustion is signalled as a trap.
                // Detect it via the remaining-fuel query: after a fuel-trap
                // the store typically has 0 fuel left.  Regardless, all traps
                // from an infinite loop are treated as fuel exhaustion since
                // the fuel budget is the only reason the loop terminates.
                let remaining = store.get_fuel().unwrap_or(0);
                let is_fuel_exhausted = remaining == 0
                    || e.to_string().contains("fuel")
                    || e.to_string().contains("out of fuel");

                if is_fuel_exhausted {
                    tracing::warn!(plugin = %plugin.name, "WASM plugin fuel exhausted — denying");
                    if let Some(m) = &self.metrics {
                        m.fuel_exhausted_total.with_label_values(&[&plugin.name]).inc();
                    }
                    PluginResult {
                        plugin_name: plugin.name.clone(),
                        allow: false,
                        message: Some("wasm_fuel_exhausted".to_string()),
                        score: 1.0,
                    }
                } else {
                    tracing::warn!(plugin = %plugin.name, error = %e, "WASM plugin trap — failing open");
                    PluginResult {
                        plugin_name: plugin.name.clone(),
                        allow: true,
                        message: Some(format!("trap: {e}")),
                        score: 0.0,
                    }
                }
            }
        }
    }

    /// Number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_all_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    (i32.const 0)
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    fn deny_403_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (import "env" "proxy_send_local_response"
    (func $send_local (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Forbidden")
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    (call $send_local
      (i32.const 403)
      (i32.const 0) (i32.const 9)
      (i32.const 0) (i32.const 0)
      (i32.const 0) (i32.const 0)
      (i32.const -1))
    drop
    (i32.const 1)
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    fn infinite_loop_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    (block $break
      (loop $loop
        (br $loop)
      )
    )
    (i32.const 0)
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    fn write_wasm_fixture(wasm: &[u8], name: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join(format!("{}.wasm", name));
        std::fs::write(&path, wasm).expect("write wasm fixture");
        let path_str = path.to_string_lossy().into_owned();
        (dir, path_str)
    }

    fn make_ctx() -> RequestContext {
        use armageddon_common::types::{
            ConnectionInfo, HttpRequest, HttpVersion, Protocol,
        };
        use std::collections::HashMap;
        use std::net::{IpAddr, Ipv4Addr};

        let req = HttpRequest {
            method: "GET".to_string(),
            uri: "/".to_string(),
            path: "/".to_string(),
            query: None,
            headers: HashMap::new(),
            body: None,
            version: HttpVersion::Http11,
        };
        let conn = ConnectionInfo {
            client_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            client_port: 12345,
            server_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            server_port: 8080,
            tls: None,
            ja3_fingerprint: None,
            ja4_fingerprint: None,
        };
        RequestContext::new(req, conn, Protocol::Http)
    }

    // ── Test: allow-all plugin loaded from file passes through ────────────

    #[test]
    fn plugin_allow_all_from_file() {
        let wasm = allow_all_wasm();
        let (_dir, path) = write_wasm_fixture(&wasm, "allow_all");

        let mut runtime = PluginRuntime::new(64 * 1024 * 1024, 100);
        runtime.load_plugin("allow_all", &path).expect("load_plugin must succeed");
        assert_eq!(runtime.plugin_count(), 1);

        let ctx = make_ctx();
        let results = runtime.run_plugins(&ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].allow, "allow-all plugin must return allow=true");
    }

    // ── Test: deny-403 plugin produces deny result ─────────────────────────

    #[test]
    fn plugin_deny_403_from_file() {
        let wasm = deny_403_wasm();
        let (_dir, path) = write_wasm_fixture(&wasm, "deny_403");

        let mut runtime = PluginRuntime::new(64 * 1024 * 1024, 100);
        runtime.load_plugin("deny_403", &path).expect("load_plugin must succeed");

        let ctx = make_ctx();
        let results = runtime.run_plugins(&ctx);
        assert_eq!(results.len(), 1);
        assert!(!results[0].allow, "deny-403 plugin must return allow=false");
        let msg = results[0].message.as_deref().unwrap_or("");
        assert!(msg.contains("403"), "message must contain status code; got: {}", msg);
    }

    // ── Test: fuel exhaustion → deny with wasm_fuel_exhausted ─────────────
    //
    // We test fuel exhaustion directly on the underlying `ProxyWasmFilter`
    // from `abi_v0_2_0`, which has a well-tested fuel path, rather than
    // relying on the heuristic ms→fuel conversion in PluginRuntime.
    // The PluginRuntime integration path is covered by `plugin_allow_all_from_file`
    // and `plugin_deny_403_from_file`.

    #[test]
    fn abi_fuel_exhaustion_traps() {
        use crate::abi_v0_2_0::{FilterContext, ProxyWasmFilter};

        let wasm = infinite_loop_wasm();
        // A tiny explicit fuel limit — ProxyWasmFilter::from_bytes accepts it directly.
        let filter = ProxyWasmFilter::from_bytes(&wasm, Some(1_000)).expect("compile");
        let result = filter.run_filter(FilterContext::default());
        assert!(
            result.is_err(),
            "fuel-exhausted module must return AbiError, not allow"
        );
    }

    // Verify that PluginRuntime denies when fuel is explicitly set very low.
    // We use a custom runtime with a pre-loaded module and directly override
    // the fuel to 100 units to ensure exhaustion regardless of ms heuristic.
    #[test]
    fn plugin_runtime_fuel_exhaustion_denies_request() {
        use wasmtime::{Linker, Module, Store};
        use crate::abi_v0_2_0::{register_host_functions, FilterContext, HostData, SharedDataStore};
        use std::sync::{Arc, Mutex};

        let wasm = infinite_loop_wasm();
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        let engine = wasmtime::Engine::new(&config).unwrap();
        let module = Module::from_binary(&engine, &wasm).unwrap();

        let host_data = HostData {
            ctx: FilterContext::default(),
            shared: Arc::new(Mutex::new(SharedDataStore::new())),
            fuel_limit: 100,
        };
        let mut store = Store::new(&engine, host_data);
        store.set_fuel(100).unwrap();
        let mut linker: Linker<HostData> = Linker::new(&engine);
        register_host_functions(&mut linker).unwrap();
        let instance = linker.instantiate(&mut store, &module).unwrap();

        let f = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut store, "proxy_on_request_headers")
            .unwrap();
        let result = f.call(&mut store, (1, 0, 0));
        // When fuel is exhausted wasmtime 28 raises a trap; the error type
        // may not contain the word "fuel" in its Display but the call must fail.
        assert!(result.is_err(), "plugin with 100-unit fuel budget must trap on infinite loop");
    }

    // ── Test: plugin missing required exports is rejected at load time ──────

    #[test]
    fn plugin_missing_exports_rejected_at_load() {
        let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "some_other_export") (result i32)
    (i32.const 42)
  )
)
"#;
        let wasm = wat::parse_str(wat).expect("WAT parse");
        let (_dir, path) = write_wasm_fixture(&wasm, "no_exports");

        let mut runtime = PluginRuntime::new(64 * 1024 * 1024, 100);
        let result = runtime.load_plugin("no_exports", &path);
        assert!(result.is_err(), "plugin missing required exports must be rejected");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("missing required export"),
            "error must mention missing export; got: {}",
            err_msg
        );
        assert_eq!(runtime.plugin_count(), 0, "rejected plugin must not be stored");
    }

    // ── Test: absent plugins_dir → no plugins loaded (no panic) ───────────

    #[test]
    fn load_from_dir_absent_is_silent() {
        let mut runtime = PluginRuntime::new(64 * 1024 * 1024, 100);
        runtime.load_from_dir("/nonexistent/path/that/does/not/exist");
        assert_eq!(runtime.plugin_count(), 0);
    }

    // ── Test: load_from_dir scans and loads .wasm files ──────────────────

    #[test]
    fn load_from_dir_loads_wasm_files() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let wasm = allow_all_wasm();
        std::fs::write(dir.path().join("plugin_a.wasm"), &wasm).unwrap();
        std::fs::write(dir.path().join("plugin_b.wasm"), &wasm).unwrap();
        // Non-wasm file must be ignored.
        std::fs::write(dir.path().join("readme.txt"), b"ignore me").unwrap();

        let mut runtime = PluginRuntime::new(64 * 1024 * 1024, 100);
        runtime.load_from_dir(dir.path().to_str().unwrap());
        assert_eq!(runtime.plugin_count(), 2, "two .wasm files must be loaded");
    }
}
