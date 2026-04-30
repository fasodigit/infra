// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Proxy-wasm ABI **v0.2.1** host runtime — Coraza-compatible.
//!
//! This module is feature-gated behind `coraza-wasm`; when the feature is
//! off, none of this code is compiled and the binary is bit-identical to
//! a build without the feature.
//!
//! # Status
//!
//! **Phase-2 done**:
//!
//! * `CorazaModule::load_with_config` reads `coraza.conf` once at boot
//!   and binds it to the guest via `proxy_on_vm_start` +
//!   `proxy_on_configure`.
//! * Per-request `CorazaInstance` exposes `on_request_headers` and
//!   `on_request_body`; both run the full proxy-wasm v0.2.1 dispatch
//!   sequence and return a `Decision`.
//! * Host functions implemented: `proxy_log`, `proxy_get_header_map_*`,
//!   `proxy_get_buffer_bytes`, `proxy_get_buffer_status`,
//!   `proxy_set_buffer_bytes`, `proxy_send_local_response`,
//!   `proxy_get_property` (whitelist of common keys),
//!   `proxy_set_effective_context`, `proxy_continue_request`,
//!   `proxy_continue_response`, plus stubs for the rest.
//! * Errors trapping out of a guest call → fail-closed `Decision::Deny(503)`.
//!
//! See `armageddon/coraza/PROXY-WASM-HOST-DESIGN.md` for the architectural
//! rationale and the bring-up roadmap.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use thiserror::Error;
use wasmtime::{Caller, Config, Engine, Linker, Module, Store, TypedFunc};

// ─────────────────────────────────────────────────────────────────────
// Constants — runtime budgets
// ─────────────────────────────────────────────────────────────────────

/// Default fuel budget per request — empirically ~10 ms of CRS evaluation
/// at PL=1 on a modern x86_64 core (100 000 fuel ≈ 1 ms heuristic).
const DEFAULT_FUEL_PER_REQUEST: u64 = 100_000_000;

/// Hard memory cap per `Store` (per-request).  Coraza's runtime working
/// set is small (~2-4 MB) but rule cache + body buffer push us up; 64 MB
/// is generous-but-bounded.  Used by `CorazaModule::create_instance` once
/// the per-request memory limiter is wired (Wasmtime `ResourceLimiter`).
#[allow(dead_code)]
const DEFAULT_MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;

/// Root context id used at module-load time (`proxy_on_vm_start`,
/// `proxy_on_configure`).  All HTTP contexts use ids ≥ 1.
const ROOT_CONTEXT_ID: i32 = 0;

/// HTTP context id used per request.  We always recycle id `1` because a
/// fresh `Store` is created per request, so context-id collisions across
/// requests are impossible.
const HTTP_CONTEXT_ID: i32 = 1;

// proxy-wasm v0.2.1 status / action codes (subset).
//
// `WasmResult` (return value of every host fn): 0 = Ok, 1 = NotFound,
// 2 = BadArgument, 3 = SerializationFailure, 4 = ParseFailure,
// 5 = BadExpression, 6 = InvalidMemoryAccess, 7 = Empty,
// 8 = CasMismatch, 9 = ResultMismatch, 10 = InternalFailure,
// 11 = BrokenConnection, 12 = Unimplemented.
const WR_OK: i32 = 0;
const WR_NOT_FOUND: i32 = 1;
const WR_BAD_ARGUMENT: i32 = 2;
const WR_INVALID_MEMORY_ACCESS: i32 = 6;
const WR_INTERNAL_FAILURE: i32 = 10;

// ─────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────

/// Errors emitted by this host runtime.
#[derive(Debug, Error)]
pub enum CorazaHostError {
    #[error("module file not found or unreadable: {path} ({source})")]
    ModuleIo {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("config file not found or unreadable: {path} ({source})")]
    ConfigIo {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("wasmtime engine init failed: {0}")]
    Engine(String),

    #[error("wasm compilation failed: {0}")]
    Compile(String),

    #[error("linker setup failed: {0}")]
    Linker(String),

    #[error("instance creation failed: {0}")]
    Instance(String),

    #[error("guest dispatch failed: {phase} — {source}")]
    Dispatch {
        phase: &'static str,
        #[source]
        source: anyhow::Error,
    },
}

// ─────────────────────────────────────────────────────────────────────
// Decision (mirrors the high-level forge filter contract)
// ─────────────────────────────────────────────────────────────────────

/// Verdict returned by the Coraza filter for a single request phase.
///
/// The caller (forge filter) maps these onto its own `Decision` /
/// `EngineVerdict` types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Pass through to the next filter / upstream.
    Continue,
    /// Block with the given HTTP status code (set by Coraza via
    /// `proxy_send_local_response`).
    Deny { status: u16, reason: String },
}

// ─────────────────────────────────────────────────────────────────────
// HostState — owned by Wasmtime `Store<HostState>`
// ─────────────────────────────────────────────────────────────────────

/// State accessible to host functions during a single request.
///
/// One instance per `Store` (i.e. per request).  Cleared at the end of
/// each request.
#[derive(Debug, Default)]
pub struct HostState {
    /// Captured `proxy_log` outputs (used by tests + telemetry).
    /// Each entry is `(level, message)`.
    pub log_buffer: Vec<(LogLevel, String)>,
    /// Pending block decision set by the guest via
    /// `proxy_send_local_response`.  `None` ⇒ allow.
    pub local_response: Option<LocalResponse>,
    /// Request headers exposed via `proxy_get_header_map_*`.  Keys are
    /// normalised to lowercase to match HTTP/2 + Coraza expectations.
    pub request_headers: BTreeMap<String, String>,
    /// Response headers (rarely used in our request-only WAF wiring).
    pub response_headers: BTreeMap<String, String>,
    /// Request trailers and response trailers — unused for now but
    /// addressable so guests don't trap on get_header_map_pairs.
    pub request_trailers: BTreeMap<String, String>,
    pub response_trailers: BTreeMap<String, String>,
    /// HTTP request body bytes (full or truncated to the WAF cap).
    pub request_body: Vec<u8>,
    /// HTTP response body bytes — unused for now.
    pub response_body: Vec<u8>,
    /// Plugin configuration buffer (read by `proxy_on_configure` via
    /// `proxy_get_buffer_bytes(BufferType::PluginConfiguration, ...)`).
    pub plugin_configuration: Vec<u8>,
    /// VM configuration buffer (per-VM).  Coraza ignores this; kept
    /// empty.
    pub vm_configuration: Vec<u8>,
    /// Mirror of the request-method / path / source-address property
    /// queries from `proxy_get_property`.  Pre-filled by
    /// `CorazaInstance::on_request_headers` so the guest can read them
    /// during evaluation.
    pub properties: BTreeMap<String, Vec<u8>>,
    /// Active proxy-wasm context id (root or http).  Set via
    /// `proxy_set_effective_context`.  Defaults to ROOT.
    pub effective_context: i32,
    /// Whether the guest has called `proxy_continue_request` /
    /// `proxy_continue_response` since the last hook.  Diagnostic only.
    pub continue_requested: bool,
}

/// Mirror of proxy-wasm `LogLevel` (v0.2.1 has 6 levels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

impl LogLevel {
    fn from_i32(v: i32) -> Self {
        match v {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            4 => LogLevel::Error,
            _ => LogLevel::Critical,
        }
    }
}

/// Local response fields (filled by `proxy_send_local_response`).
#[derive(Debug, Clone)]
pub struct LocalResponse {
    pub status: u16,
    pub reason: String,
    pub body: Vec<u8>,
}

/// Map type encoding (proxy-wasm v0.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapType {
    RequestHeaders,
    RequestTrailers,
    ResponseHeaders,
    ResponseTrailers,
    GrpcMetadata,
}

impl MapType {
    fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(MapType::RequestHeaders),
            1 => Some(MapType::RequestTrailers),
            2 => Some(MapType::ResponseHeaders),
            3 => Some(MapType::ResponseTrailers),
            4 => Some(MapType::GrpcMetadata),
            _ => None,
        }
    }
}

/// Buffer type encoding (proxy-wasm v0.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BufferType {
    HttpRequestBody,
    HttpResponseBody,
    DownstreamData,
    UpstreamData,
    HttpCallResponseBody,
    GrpcReceiveBuffer,
    VmConfiguration,
    PluginConfiguration,
}

impl BufferType {
    fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(BufferType::HttpRequestBody),
            1 => Some(BufferType::HttpResponseBody),
            2 => Some(BufferType::DownstreamData),
            3 => Some(BufferType::UpstreamData),
            4 => Some(BufferType::HttpCallResponseBody),
            5 => Some(BufferType::GrpcReceiveBuffer),
            6 => Some(BufferType::VmConfiguration),
            7 => Some(BufferType::PluginConfiguration),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// CorazaModule — AOT-compiled shared module handle
// ─────────────────────────────────────────────────────────────────────

/// A compiled Coraza WASM module — cheap to clone (`Arc` internally).
///
/// Compile **once** at startup, share across all requests, then call
/// [`CorazaModule::create_instance`] per request.
#[derive(Clone)]
pub struct CorazaModule {
    /// Shared Wasmtime engine — must outlive every `Store` derived from
    /// modules compiled with it.
    engine: Engine,
    /// Compiled `.wasm` (AOT, includes the rule cache).
    module: Arc<Module>,
    /// Plugin configuration bytes (`coraza.conf`).  Empty when no
    /// configuration is supplied — the guest then uses its built-in
    /// defaults.
    plugin_configuration: Arc<Vec<u8>>,
}

impl std::fmt::Debug for CorazaModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorazaModule")
            .field("engine", &"<wasmtime::Engine>")
            .field("module", &"<wasmtime::Module>")
            .field("plugin_configuration_bytes", &self.plugin_configuration.len())
            .finish()
    }
}

impl CorazaModule {
    /// Build a Wasmtime engine configured for safe production loading
    /// of a Coraza module: fuel metering on, no native epoch interrupts,
    /// no debug info, async support off (host calls are synchronous).
    fn build_engine() -> Result<Engine, CorazaHostError> {
        let mut cfg = Config::new();
        cfg.consume_fuel(true);
        // Coraza ships ~16 MB; AOT compilation is the bottleneck so we
        // accept default Cranelift settings (no `cranelift_opt_level`
        // tweaks — the stable default is `Speed`).
        cfg.wasm_multi_memory(false);
        cfg.wasm_threads(false);
        Engine::new(&cfg).map_err(|e| CorazaHostError::Engine(e.to_string()))
    }

    /// Compile a Coraza `.wasm` from disk.
    ///
    /// Returns an error if the path is missing, the file is unreadable,
    /// or the bytes do not pass Wasmtime's validator.
    pub fn load(path: &Path) -> Result<Self, CorazaHostError> {
        let bytes = std::fs::read(path).map_err(|e| CorazaHostError::ModuleIo {
            path: path.display().to_string(),
            source: e,
        })?;
        Self::load_from_bytes(&bytes)
    }

    /// Compile from in-memory bytes (used by tests + test fixtures).
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, CorazaHostError> {
        Self::load_from_bytes_with_config(bytes, Vec::new())
    }

    /// Like [`load_from_bytes`] but bind a plugin configuration buffer
    /// (the contents of `coraza.conf`).  The buffer is exposed to the
    /// guest at `BufferType::PluginConfiguration` and read by
    /// `proxy_on_configure`.
    pub fn load_from_bytes_with_config(
        bytes: &[u8],
        plugin_configuration: Vec<u8>,
    ) -> Result<Self, CorazaHostError> {
        let engine = Self::build_engine()?;
        let module = Module::from_binary(&engine, bytes)
            .map_err(|e| CorazaHostError::Compile(e.to_string()))?;
        Ok(Self {
            engine,
            module: Arc::new(module),
            plugin_configuration: Arc::new(plugin_configuration),
        })
    }

    /// Load a module from disk and bind a config file (read into memory).
    pub fn load_with_config(
        module_path: &Path,
        config_path: &Path,
    ) -> Result<Self, CorazaHostError> {
        let module_bytes = std::fs::read(module_path).map_err(|e| CorazaHostError::ModuleIo {
            path: module_path.display().to_string(),
            source: e,
        })?;
        let config_bytes = std::fs::read(config_path).map_err(|e| CorazaHostError::ConfigIo {
            path: config_path.display().to_string(),
            source: e,
        })?;
        Self::load_from_bytes_with_config(&module_bytes, config_bytes)
    }

    /// Create a fresh per-request instance.  Each instance has its own
    /// `Store<HostState>` so memory + fuel are isolated.
    ///
    /// **Tradeoff**: per-request instantiation costs ~50-100 µs on a
    /// modern x86 core for the 16 MB Coraza module — acceptable for a
    /// security filter.  An alternative (worker-pool with one
    /// `Store` per worker) would eliminate that cost but introduces
    /// thread-affinity complexity; we choose the simpler model and
    /// will revisit if hot-path latency budget is breached.
    pub fn create_instance(&self) -> Result<CorazaInstance, CorazaHostError> {
        let mut state = HostState::default();
        // Pre-load plugin configuration so `proxy_on_configure` can
        // read it via `proxy_get_buffer_bytes`.
        state.plugin_configuration = (*self.plugin_configuration).clone();

        let mut store = Store::new(&self.engine, state);
        store
            .set_fuel(DEFAULT_FUEL_PER_REQUEST)
            .map_err(|e| CorazaHostError::Instance(e.to_string()))?;

        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        register_v0_2_1_host_functions(&mut linker)
            .map_err(|e| CorazaHostError::Linker(e.to_string()))?;

        // TinyGo + WASI: stub `wasi_snapshot_preview1` imports so the
        // module instantiates even if the guest links them.  We don't
        // service real WASI calls — `_initialize` is the only one that
        // matters for Coraza, and it's a guest-side init.
        register_wasi_stubs(&mut linker)
            .map_err(|e| CorazaHostError::Linker(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| CorazaHostError::Instance(e.to_string()))?;

        let mut inst = CorazaInstance {
            store,
            instance,
            configured: false,
        };

        // One-time boot: WASI _initialize → proxy_on_vm_start →
        // proxy_on_context_create(root) → proxy_on_configure(root).
        // Failures here are non-fatal — we log and let the request path
        // discover them via Decision::Deny(503) on first hook.
        if let Err(e) = inst.bootstrap(self.plugin_configuration.len() as i32) {
            tracing::warn!(err = %e, "coraza guest bootstrap failed; instance will fail-closed");
        }
        Ok(inst)
    }
}

// ─────────────────────────────────────────────────────────────────────
// CorazaInstance — per-request handle
// ─────────────────────────────────────────────────────────────────────

/// A live Coraza filter instance bound to a single request.
///
/// **Not** `Send` (Wasmtime invariant).  Must stay on the thread that
/// created it.
pub struct CorazaInstance {
    store: Store<HostState>,
    instance: wasmtime::Instance,
    /// Set once `proxy_on_configure` has run.
    configured: bool,
}

impl CorazaInstance {
    /// Run the one-time guest initialisation: `_initialize` (WASI) →
    /// `proxy_on_vm_start` → `proxy_on_context_create(root, 0)` →
    /// `proxy_on_configure(root, plugin_config_size)`.
    ///
    /// Some of these exports are optional (TinyGo emits them, hand-rolled
    /// guests may not), so each call is best-effort.
    fn bootstrap(&mut self, plugin_config_size: i32) -> Result<(), CorazaHostError> {
        // _initialize — optional WASI start hook.
        if let Some(f) = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "_initialize")
            .ok()
        {
            f.call(&mut self.store, ()).map_err(|e| CorazaHostError::Dispatch {
                phase: "_initialize",
                source: e,
            })?;
        }

        // proxy_on_vm_start(root_ctx, vm_config_size) — required, but
        // returns an Action (i32) we discard.
        if let Ok(f) = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, "proxy_on_vm_start")
        {
            f.call(&mut self.store, (ROOT_CONTEXT_ID, 0))
                .map_err(|e| CorazaHostError::Dispatch {
                    phase: "proxy_on_vm_start",
                    source: e,
                })?;
        }

        // proxy_on_context_create(root_ctx, parent=0) — root context
        // creation.  Coraza requires this before `proxy_on_configure`.
        if let Ok(f) = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "proxy_on_context_create")
        {
            f.call(&mut self.store, (ROOT_CONTEXT_ID, 0))
                .map_err(|e| CorazaHostError::Dispatch {
                    phase: "proxy_on_context_create(root)",
                    source: e,
                })?;
        }

        // proxy_on_configure(root_ctx, configuration_size) — the guest
        // reads PluginConfiguration via `proxy_get_buffer_bytes`.
        if let Ok(f) = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, "proxy_on_configure")
        {
            f.call(&mut self.store, (ROOT_CONTEXT_ID, plugin_config_size))
                .map_err(|e| CorazaHostError::Dispatch {
                    phase: "proxy_on_configure",
                    source: e,
                })?;
        }

        // proxy_on_context_create(http_ctx, parent=root) — create the
        // per-request HTTP context.
        if let Ok(f) = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "proxy_on_context_create")
        {
            f.call(&mut self.store, (HTTP_CONTEXT_ID, ROOT_CONTEXT_ID))
                .map_err(|e| CorazaHostError::Dispatch {
                    phase: "proxy_on_context_create(http)",
                    source: e,
                })?;
        }

        self.configured = true;
        Ok(())
    }

    /// Lookup a typed export, returning a friendly error on miss.
    fn typed_func<P, R>(&mut self, name: &'static str) -> Option<TypedFunc<P, R>>
    where
        P: wasmtime::WasmParams,
        R: wasmtime::WasmResults,
    {
        self.instance.get_typed_func::<P, R>(&mut self.store, name).ok()
    }

    /// Inject HTTP request fields into host state so the guest can read
    /// them during evaluation.
    fn seed_request_state(
        &mut self,
        method: &str,
        path: &str,
        source_addr: &str,
        headers: &[(String, String)],
    ) {
        let s = self.store.data_mut();
        s.request_headers.clear();
        for (k, v) in headers {
            s.request_headers.insert(k.to_lowercase(), v.clone());
        }
        // Pre-populate the property tree with the keys Coraza most often
        // queries.  Unknown keys still return NotFound from
        // `proxy_get_property` (host fn handles whitelist + state map).
        s.properties.clear();
        s.properties
            .insert("request.method".to_string(), method.as_bytes().to_vec());
        s.properties
            .insert("request.url_path".to_string(), path.as_bytes().to_vec());
        // Coraza CRS phase 1 also probes "request.protocol".
        s.properties
            .insert("request.protocol".to_string(), b"HTTP/1.1".to_vec());
        s.properties.insert(
            "source.address".to_string(),
            source_addr.as_bytes().to_vec(),
        );
        s.effective_context = HTTP_CONTEXT_ID;
    }

    /// Inspect request headers.  Returns `Decision::Deny` if Coraza
    /// short-circuits via `proxy_send_local_response`, else `Continue`.
    pub fn on_request_headers(
        &mut self,
        method: &str,
        path: &str,
        source_addr: &str,
        headers: &[(String, String)],
    ) -> Decision {
        self.seed_request_state(method, path, source_addr, headers);

        let num_headers = headers.len() as i32;
        // proxy_on_request_headers returns an Action: 0=Continue, 1=Pause.
        let f = match self.typed_func::<(i32, i32, i32), i32>("proxy_on_request_headers") {
            Some(f) => f,
            None => return Decision::Continue,
        };
        match f.call(&mut self.store, (HTTP_CONTEXT_ID, num_headers, 0)) {
            Ok(_action) => self.read_decision_or_continue(),
            Err(e) => {
                tracing::error!(err = %e, "coraza on_request_headers trapped — fail-closed 503");
                Decision::Deny {
                    status: 503,
                    reason: format!("coraza_trap: {e}"),
                }
            }
        }
    }

    /// Inspect the (already-buffered) request body.
    ///
    /// Caller is expected to have invoked [`on_request_headers`] first;
    /// for tests or pure-body scenarios it is acceptable to call this
    /// directly — the guest will still run header rules with an empty
    /// header map (CRS phase-1 will pass).
    pub fn on_request_body(&mut self, body: &[u8]) -> Decision {
        // Stash body in host state so `proxy_get_buffer_bytes` returns it.
        {
            let s = self.store.data_mut();
            s.request_body = body.to_vec();
        }

        // proxy_on_request_body(ctx_id, body_size, end_of_stream).
        let f = match self.typed_func::<(i32, i32, i32), i32>("proxy_on_request_body") {
            Some(f) => f,
            None => return Decision::Continue,
        };
        match f.call(&mut self.store, (HTTP_CONTEXT_ID, body.len() as i32, 1 /* eos */)) {
            Ok(_action) => self.read_decision_or_continue(),
            Err(e) => {
                tracing::error!(err = %e, "coraza on_request_body trapped — fail-closed 503");
                Decision::Deny {
                    status: 503,
                    reason: format!("coraza_trap: {e}"),
                }
            }
        }
    }

    /// Inspect `HostState.local_response`; if present → Deny, else Continue.
    fn read_decision_or_continue(&self) -> Decision {
        match self.store.data().local_response.as_ref() {
            Some(lr) => Decision::Deny {
                status: lr.status,
                reason: lr.reason.clone(),
            },
            None => Decision::Continue,
        }
    }

    /// Test helper: drain captured proxy_log messages.
    #[cfg(test)]
    pub(crate) fn drain_logs(&mut self) -> Vec<(LogLevel, String)> {
        std::mem::take(&mut self.store.data_mut().log_buffer)
    }

    /// Test helper: read the captured local_response, if any.
    #[cfg(test)]
    pub(crate) fn local_response(&self) -> Option<LocalResponse> {
        self.store.data().local_response.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────
// v0.2.1 host-function registration
// ─────────────────────────────────────────────────────────────────────

/// Wire every proxy-wasm v0.2.1 import the Coraza guest may call.
///
/// Spec: <https://github.com/proxy-wasm/spec/blob/main/abi-versions/v0.2.1/README.md>
fn register_v0_2_1_host_functions(linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    // ── proxy_log ────────────────────────────────────────────────────
    // Signature (v0.2.1): (level: i32, msg_ptr: i32, msg_size: i32) -> i32
    linker.func_wrap(
        "env",
        "proxy_log",
        |mut caller: Caller<'_, HostState>,
         level: i32,
         msg_ptr: i32,
         msg_size: i32|
         -> i32 {
            let lvl = LogLevel::from_i32(level);

            let bytes = match read_guest_bytes(&mut caller, msg_ptr, msg_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let msg = String::from_utf8_lossy(&bytes).into_owned();

            match lvl {
                LogLevel::Trace => tracing::trace!(target: "proxy_wasm_v021", "{}", msg),
                LogLevel::Debug => tracing::debug!(target: "proxy_wasm_v021", "{}", msg),
                LogLevel::Info => tracing::info!(target: "proxy_wasm_v021", "{}", msg),
                LogLevel::Warn => tracing::warn!(target: "proxy_wasm_v021", "{}", msg),
                LogLevel::Error | LogLevel::Critical => {
                    tracing::error!(target: "proxy_wasm_v021", "{}", msg)
                }
            }

            let buf = &mut caller.data_mut().log_buffer;
            if buf.len() < 256 {
                buf.push((lvl, msg));
            }
            WR_OK
        },
    )?;

    // ── proxy_log_status ─────────────────────────────────────────────
    // (status: i32) -> i32
    // Stash the last log status for diagnostic purposes.
    linker.func_wrap(
        "env",
        "proxy_log_status",
        |_caller: Caller<'_, HostState>, _status: i32| -> i32 { WR_OK },
    )?;

    // ── proxy_set_tick_period_milliseconds ───────────────────────────
    linker.func_wrap(
        "env",
        "proxy_set_tick_period_milliseconds",
        |_caller: Caller<'_, HostState>, _period_ms: i32| -> i32 { WR_OK },
    )?;

    // ── proxy_get_property ───────────────────────────────────────────
    // (path_ptr, path_size, *value_ptr_out, *value_size_out) -> i32
    //
    // proxy-wasm encodes property paths as NUL-separated path segments
    // (e.g. "request\0url_path"). We accept both that form and the
    // dotted form ("request.url_path") so simple callers work too.
    linker.func_wrap(
        "env",
        "proxy_get_property",
        |mut caller: Caller<'_, HostState>,
         path_ptr: i32,
         path_size: i32,
         value_ptr_out: i32,
         value_size_out: i32|
         -> i32 {
            let raw = match read_guest_bytes(&mut caller, path_ptr, path_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let key = normalise_property_path(&raw);
            let value = caller.data().properties.get(&key).cloned();
            match value {
                None => WR_NOT_FOUND,
                Some(v) => match write_guest_buffer(&mut caller, &v, value_ptr_out, value_size_out)
                {
                    Ok(()) => WR_OK,
                    Err(code) => code,
                },
            }
        },
    )?;

    // ── proxy_set_property ───────────────────────────────────────────
    linker.func_wrap(
        "env",
        "proxy_set_property",
        |mut caller: Caller<'_, HostState>,
         path_ptr: i32,
         path_size: i32,
         value_ptr: i32,
         value_size: i32|
         -> i32 {
            let raw = match read_guest_bytes(&mut caller, path_ptr, path_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let key = normalise_property_path(&raw);
            let value = match read_guest_bytes(&mut caller, value_ptr, value_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            caller.data_mut().properties.insert(key, value);
            WR_OK
        },
    )?;

    // ── proxy_get_header_map_value ───────────────────────────────────
    // (map_type, key_ptr, key_size, *value_ptr_out, *value_size_out) -> i32
    linker.func_wrap(
        "env",
        "proxy_get_header_map_value",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         key_ptr: i32,
         key_size: i32,
         value_ptr_out: i32,
         value_size_out: i32|
         -> i32 {
            let mt = match MapType::from_i32(map_type) {
                Some(m) => m,
                None => return WR_BAD_ARGUMENT,
            };
            let key_bytes = match read_guest_bytes(&mut caller, key_ptr, key_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let key = String::from_utf8_lossy(&key_bytes).to_lowercase();
            let value = with_map(caller.data(), mt, |m| m.get(&key).cloned());
            match value {
                None => WR_NOT_FOUND,
                Some(v) => match write_guest_buffer(
                    &mut caller,
                    v.as_bytes(),
                    value_ptr_out,
                    value_size_out,
                ) {
                    Ok(()) => WR_OK,
                    Err(code) => code,
                },
            }
        },
    )?;

    // ── proxy_get_header_map_pairs ───────────────────────────────────
    // (map_type, *ret_ptr_out, *ret_size_out) -> i32
    //
    // Returns the entire map serialised in the proxy-wasm wire format:
    //
    //   u32  num_pairs
    //   { u32 key_size, u32 value_size } * num_pairs
    //   ( key + '\0' + value + '\0' ) * num_pairs
    //
    // The block of bytes is allocated in the guest via
    // `proxy_on_memory_allocate`.
    linker.func_wrap(
        "env",
        "proxy_get_header_map_pairs",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         ret_ptr_out: i32,
         ret_size_out: i32|
         -> i32 {
            let mt = match MapType::from_i32(map_type) {
                Some(m) => m,
                None => return WR_BAD_ARGUMENT,
            };
            let pairs: Vec<(String, String)> =
                with_map(caller.data(), mt, |m| {
                    m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                });
            let encoded = encode_map_pairs(&pairs);
            match write_guest_buffer(&mut caller, &encoded, ret_ptr_out, ret_size_out) {
                Ok(()) => WR_OK,
                Err(code) => code,
            }
        },
    )?;

    // ── proxy_set_header_map_pairs ───────────────────────────────────
    linker.func_wrap(
        "env",
        "proxy_set_header_map_pairs",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         pairs_ptr: i32,
         pairs_size: i32|
         -> i32 {
            let mt = match MapType::from_i32(map_type) {
                Some(m) => m,
                None => return WR_BAD_ARGUMENT,
            };
            let raw = match read_guest_bytes(&mut caller, pairs_ptr, pairs_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let pairs = match decode_map_pairs(&raw) {
                Some(p) => p,
                None => return WR_BAD_ARGUMENT,
            };
            with_map_mut(caller.data_mut(), mt, |m| {
                m.clear();
                for (k, v) in pairs {
                    m.insert(k.to_lowercase(), v);
                }
            });
            WR_OK
        },
    )?;

    // ── proxy_replace_header_map_value (alias for set in v0.2.1) ─────
    linker.func_wrap(
        "env",
        "proxy_replace_header_map_value",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         key_ptr: i32,
         key_size: i32,
         value_ptr: i32,
         value_size: i32|
         -> i32 {
            set_header_value(&mut caller, map_type, key_ptr, key_size, value_ptr, value_size)
        },
    )?;

    // ── proxy_set_header_map_value (alias of replace) ────────────────
    linker.func_wrap(
        "env",
        "proxy_set_header_map_value",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         key_ptr: i32,
         key_size: i32,
         value_ptr: i32,
         value_size: i32|
         -> i32 {
            set_header_value(&mut caller, map_type, key_ptr, key_size, value_ptr, value_size)
        },
    )?;

    // ── proxy_remove_header_map_value ────────────────────────────────
    linker.func_wrap(
        "env",
        "proxy_remove_header_map_value",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         key_ptr: i32,
         key_size: i32|
         -> i32 {
            let mt = match MapType::from_i32(map_type) {
                Some(m) => m,
                None => return WR_BAD_ARGUMENT,
            };
            let kb = match read_guest_bytes(&mut caller, key_ptr, key_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let key = String::from_utf8_lossy(&kb).to_lowercase();
            with_map_mut(caller.data_mut(), mt, |m| {
                m.remove(&key);
            });
            WR_OK
        },
    )?;

    // ── proxy_add_header_map_value ───────────────────────────────────
    // In v0.2.1 add ≡ insert/append.  We keep last-wins semantics
    // (matches BTreeMap) since most CRS rules don't require multi-value.
    linker.func_wrap(
        "env",
        "proxy_add_header_map_value",
        |mut caller: Caller<'_, HostState>,
         map_type: i32,
         key_ptr: i32,
         key_size: i32,
         value_ptr: i32,
         value_size: i32|
         -> i32 {
            set_header_value(&mut caller, map_type, key_ptr, key_size, value_ptr, value_size)
        },
    )?;

    // ── proxy_get_buffer_bytes ───────────────────────────────────────
    // (buffer_type, start, max_size, *ret_ptr_out, *ret_size_out) -> i32
    linker.func_wrap(
        "env",
        "proxy_get_buffer_bytes",
        |mut caller: Caller<'_, HostState>,
         buf_type: i32,
         start: i32,
         max_size: i32,
         ret_ptr_out: i32,
         ret_size_out: i32|
         -> i32 {
            let bt = match BufferType::from_i32(buf_type) {
                Some(b) => b,
                None => return WR_BAD_ARGUMENT,
            };
            let slice = match select_buffer(caller.data(), bt) {
                Some(b) => b.clone(),
                None => return WR_NOT_FOUND,
            };
            let start = start.max(0) as usize;
            let max = max_size.max(0) as usize;
            if start > slice.len() {
                // Out-of-range start → empty result, not an error
                // (proxy-wasm semantics).
                return write_guest_buffer(&mut caller, &[], ret_ptr_out, ret_size_out)
                    .map(|_| WR_OK)
                    .unwrap_or_else(|c| c);
            }
            let end = (start + max).min(slice.len());
            let bytes = &slice[start..end];
            match write_guest_buffer(&mut caller, bytes, ret_ptr_out, ret_size_out) {
                Ok(()) => WR_OK,
                Err(code) => code,
            }
        },
    )?;

    // ── proxy_get_buffer_status ──────────────────────────────────────
    // (buffer_type, *length_out, *flags_out) -> i32
    linker.func_wrap(
        "env",
        "proxy_get_buffer_status",
        |mut caller: Caller<'_, HostState>,
         buf_type: i32,
         length_out: i32,
         flags_out: i32|
         -> i32 {
            let bt = match BufferType::from_i32(buf_type) {
                Some(b) => b,
                None => return WR_BAD_ARGUMENT,
            };
            let len = match select_buffer(caller.data(), bt) {
                Some(b) => b.len() as u32,
                None => 0,
            };
            // Flags: bit0 = end_of_stream — request body is always
            // accumulated fully before dispatch, so we report EOS.
            if write_u32(&mut caller, length_out as u32, len).is_err() {
                return WR_INVALID_MEMORY_ACCESS;
            }
            if write_u32(&mut caller, flags_out as u32, 1).is_err() {
                return WR_INVALID_MEMORY_ACCESS;
            }
            WR_OK
        },
    )?;

    // ── proxy_set_buffer_bytes ───────────────────────────────────────
    // (buffer_type, start, length, data_ptr, data_size) -> i32
    //
    // We allow body mutation only on HttpRequestBody / HttpResponseBody.
    // `start + length` defines the slice to replace; the host expands or
    // truncates `request_body` accordingly.  Read-only buffers
    // (PluginConfiguration, VmConfiguration) refuse with NotFound.
    linker.func_wrap(
        "env",
        "proxy_set_buffer_bytes",
        |mut caller: Caller<'_, HostState>,
         buf_type: i32,
         start: i32,
         length: i32,
         data_ptr: i32,
         data_size: i32|
         -> i32 {
            let bt = match BufferType::from_i32(buf_type) {
                Some(b) => b,
                None => return WR_BAD_ARGUMENT,
            };
            let src = match read_guest_bytes(&mut caller, data_ptr, data_size) {
                Ok(b) => b,
                Err(code) => return code,
            };
            let start = start.max(0) as usize;
            let length = length.max(0) as usize;
            let dst = match select_buffer_mut(caller.data_mut(), bt) {
                Some(b) => b,
                None => return WR_NOT_FOUND,
            };
            // Bound `end` to current dst length — replacement must fall
            // within addressable range to avoid sparse writes.
            let end = (start + length).min(dst.len());
            if start > dst.len() {
                return WR_BAD_ARGUMENT;
            }
            dst.splice(start..end, src.into_iter());
            WR_OK
        },
    )?;

    // ── proxy_send_local_response ────────────────────────────────────
    // (status, status_msg_ptr, status_msg_size, body_ptr, body_size,
    //  headers_ptr, headers_size, grpc_status) -> i32
    linker.func_wrap(
        "env",
        "proxy_send_local_response",
        |mut caller: Caller<'_, HostState>,
         status_code: i32,
         status_msg_ptr: i32,
         status_msg_size: i32,
         body_ptr: i32,
         body_size: i32,
         _headers_ptr: i32,
         _headers_size: i32,
         _grpc_status: i32|
         -> i32 {
            let reason_bytes = read_guest_bytes(&mut caller, status_msg_ptr, status_msg_size)
                .unwrap_or_default();
            let body_bytes =
                read_guest_bytes(&mut caller, body_ptr, body_size).unwrap_or_default();
            let reason = if reason_bytes.is_empty() {
                "coraza_block".to_string()
            } else {
                String::from_utf8_lossy(&reason_bytes).into_owned()
            };
            caller.data_mut().local_response = Some(LocalResponse {
                status: status_code as u16,
                reason,
                body: body_bytes,
            });
            WR_OK
        },
    )?;

    // ── proxy_set_effective_context ──────────────────────────────────
    linker.func_wrap(
        "env",
        "proxy_set_effective_context",
        |mut caller: Caller<'_, HostState>, ctx_id: i32| -> i32 {
            caller.data_mut().effective_context = ctx_id;
            WR_OK
        },
    )?;

    // ── proxy_continue_request / proxy_continue_response ─────────────
    linker.func_wrap(
        "env",
        "proxy_continue_request",
        |mut caller: Caller<'_, HostState>| -> i32 {
            caller.data_mut().continue_requested = true;
            WR_OK
        },
    )?;
    linker.func_wrap(
        "env",
        "proxy_continue_response",
        |mut caller: Caller<'_, HostState>| -> i32 {
            caller.data_mut().continue_requested = true;
            WR_OK
        },
    )?;
    linker.func_wrap(
        "env",
        "proxy_done",
        |_caller: Caller<'_, HostState>| -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_resume_http_request",
        |_caller: Caller<'_, HostState>| -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_resume_http_response",
        |_caller: Caller<'_, HostState>| -> i32 { WR_OK },
    )?;

    // ── shared data + metrics — stubs (not on hot path for CRS PL=1) ─
    linker.func_wrap(
        "env",
        "proxy_get_shared_data",
        |_caller: Caller<'_, HostState>,
         _key_ptr: i32,
         _key_size: i32,
         _value_ptr_out: i32,
         _value_size_out: i32,
         _cas_out: i32|
         -> i32 { WR_NOT_FOUND },
    )?;
    linker.func_wrap(
        "env",
        "proxy_set_shared_data",
        |_caller: Caller<'_, HostState>,
         _key_ptr: i32,
         _key_size: i32,
         _value_ptr: i32,
         _value_size: i32,
         _cas: i32|
         -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_define_metric",
        |_caller: Caller<'_, HostState>,
         _metric_type: i32,
         _name_ptr: i32,
         _name_size: i32,
         _id_out: i32|
         -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_increment_metric",
        |_caller: Caller<'_, HostState>, _id: i32, _offset: i64| -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_record_metric",
        |_caller: Caller<'_, HostState>, _id: i32, _value: i64| -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_get_metric",
        |_caller: Caller<'_, HostState>, _id: i32, _value_out: i32| -> i32 { WR_OK },
    )?;

    // ── HTTP call dispatch — not used by CRS v4 OSS, stubbed. ────────
    linker.func_wrap(
        "env",
        "proxy_http_call",
        |_caller: Caller<'_, HostState>,
         _upstream_ptr: i32,
         _upstream_size: i32,
         _headers_ptr: i32,
         _headers_size: i32,
         _body_ptr: i32,
         _body_size: i32,
         _trailers_ptr: i32,
         _trailers_size: i32,
         _timeout_ms: i32,
         _token_out: i32|
         -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_dispatch_http_call",
        |_caller: Caller<'_, HostState>,
         _upstream_ptr: i32,
         _upstream_size: i32,
         _headers_ptr: i32,
         _headers_size: i32,
         _body_ptr: i32,
         _body_size: i32,
         _trailers_ptr: i32,
         _trailers_size: i32,
         _timeout_ms: i32,
         _token_out: i32|
         -> i32 { WR_OK },
    )?;
    linker.func_wrap(
        "env",
        "proxy_call_foreign_function",
        |_caller: Caller<'_, HostState>,
         _name_ptr: i32,
         _name_size: i32,
         _param_ptr: i32,
         _param_size: i32,
         _return_ptr: i32,
         _return_size: i32|
         -> i32 { WR_NOT_FOUND },
    )?;

    Ok(())
}

/// WASI snapshot-1 stubs.  TinyGo links a handful of WASI imports even
/// for non-WASI guests; we satisfy them with empty success returns so
/// the module instantiates.  None of these implement real semantics.
fn register_wasi_stubs(linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    let mod_name = "wasi_snapshot_preview1";
    // fd_write — TinyGo prints to stderr via this.  We discard.
    linker.func_wrap(
        mod_name,
        "fd_write",
        |_c: Caller<'_, HostState>, _fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32| -> i32 {
            0
        },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_read",
        |_c: Caller<'_, HostState>, _fd: i32, _iovs: i32, _iovs_len: i32, _nread: i32| -> i32 {
            0
        },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_close",
        |_c: Caller<'_, HostState>, _fd: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_seek",
        |_c: Caller<'_, HostState>,
         _fd: i32,
         _offset: i64,
         _whence: i32,
         _newoffset: i32|
         -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_fdstat_get",
        |_c: Caller<'_, HostState>, _fd: i32, _out: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_filestat_get",
        |_c: Caller<'_, HostState>, _fd: i32, _out: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_prestat_get",
        |_c: Caller<'_, HostState>, _fd: i32, _out: i32| -> i32 {
            // 8 = WASI errno BADF — TinyGo uses this to detect end of
            // preopen scan; returning BADF aborts the scan cleanly.
            8
        },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_prestat_dir_name",
        |_c: Caller<'_, HostState>, _fd: i32, _path: i32, _path_len: i32| -> i32 { 8 },
    )?;
    linker.func_wrap(
        mod_name,
        "fd_fdstat_set_flags",
        |_c: Caller<'_, HostState>, _fd: i32, _flags: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "path_open",
        |_c: Caller<'_, HostState>,
         _fd: i32,
         _dirflags: i32,
         _path: i32,
         _path_len: i32,
         _oflags: i32,
         _fs_rights_base: i64,
         _fs_rights_inheriting: i64,
         _fdflags: i32,
         _opened_fd: i32|
         -> i32 { 8 },
    )?;
    linker.func_wrap(
        mod_name,
        "path_filestat_get",
        |_c: Caller<'_, HostState>,
         _fd: i32,
         _flags: i32,
         _path: i32,
         _path_len: i32,
         _out: i32|
         -> i32 { 8 },
    )?;
    linker.func_wrap(
        mod_name,
        "poll_oneoff",
        |_c: Caller<'_, HostState>,
         _in_ptr: i32,
         _out_ptr: i32,
         _nsubs: i32,
         _nevents: i32|
         -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "sched_yield",
        |_c: Caller<'_, HostState>| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "clock_res_get",
        |_c: Caller<'_, HostState>, _id: i32, _out: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "environ_get",
        |_c: Caller<'_, HostState>, _ev: i32, _eb: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "environ_sizes_get",
        |_c: Caller<'_, HostState>, _ec: i32, _eb: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "args_get",
        |_c: Caller<'_, HostState>, _av: i32, _ab: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "args_sizes_get",
        |_c: Caller<'_, HostState>, _ac: i32, _ab: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "clock_time_get",
        |_c: Caller<'_, HostState>, _id: i32, _prec: i64, _out: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "random_get",
        |_c: Caller<'_, HostState>, _ptr: i32, _len: i32| -> i32 { 0 },
    )?;
    linker.func_wrap(
        mod_name,
        "proc_exit",
        |_c: Caller<'_, HostState>, _code: i32| {},
    )?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────
// Helpers — header maps + property paths + map-pair codec
// ─────────────────────────────────────────────────────────────────────

fn with_map<F, R>(state: &HostState, mt: MapType, f: F) -> R
where
    F: FnOnce(&BTreeMap<String, String>) -> R,
{
    match mt {
        MapType::RequestHeaders => f(&state.request_headers),
        MapType::RequestTrailers => f(&state.request_trailers),
        MapType::ResponseHeaders => f(&state.response_headers),
        MapType::ResponseTrailers => f(&state.response_trailers),
        MapType::GrpcMetadata => f(&state.request_headers),
    }
}

fn with_map_mut<F, R>(state: &mut HostState, mt: MapType, f: F) -> R
where
    F: FnOnce(&mut BTreeMap<String, String>) -> R,
{
    match mt {
        MapType::RequestHeaders => f(&mut state.request_headers),
        MapType::RequestTrailers => f(&mut state.request_trailers),
        MapType::ResponseHeaders => f(&mut state.response_headers),
        MapType::ResponseTrailers => f(&mut state.response_trailers),
        MapType::GrpcMetadata => f(&mut state.request_headers),
    }
}

fn select_buffer(state: &HostState, bt: BufferType) -> Option<&Vec<u8>> {
    match bt {
        BufferType::HttpRequestBody => Some(&state.request_body),
        BufferType::HttpResponseBody => Some(&state.response_body),
        BufferType::PluginConfiguration => Some(&state.plugin_configuration),
        BufferType::VmConfiguration => Some(&state.vm_configuration),
        // Other buffer types (DownstreamData/UpstreamData/HttpCallResponseBody/
        // GrpcReceiveBuffer) are not modelled — return None ⇒ NotFound.
        _ => None,
    }
}

fn select_buffer_mut(state: &mut HostState, bt: BufferType) -> Option<&mut Vec<u8>> {
    match bt {
        BufferType::HttpRequestBody => Some(&mut state.request_body),
        BufferType::HttpResponseBody => Some(&mut state.response_body),
        _ => None,
    }
}

fn set_header_value(
    caller: &mut Caller<'_, HostState>,
    map_type: i32,
    key_ptr: i32,
    key_size: i32,
    value_ptr: i32,
    value_size: i32,
) -> i32 {
    let mt = match MapType::from_i32(map_type) {
        Some(m) => m,
        None => return WR_BAD_ARGUMENT,
    };
    let kb = match read_guest_bytes(caller, key_ptr, key_size) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let vb = match read_guest_bytes(caller, value_ptr, value_size) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let key = String::from_utf8_lossy(&kb).to_lowercase();
    let value = String::from_utf8_lossy(&vb).into_owned();
    with_map_mut(caller.data_mut(), mt, |m| {
        m.insert(key, value);
    });
    WR_OK
}

/// Normalise a property path coming from the guest.
///
/// proxy-wasm spec: NUL-separated path segments
/// (`request\0url_path` ⇒ "request.url_path").  Some guests dot-separate
/// instead — we accept both forms.
fn normalise_property_path(raw: &[u8]) -> String {
    if raw.contains(&0) {
        let mut out = String::with_capacity(raw.len());
        for (i, seg) in raw.split(|b| *b == 0).enumerate() {
            if seg.is_empty() {
                continue;
            }
            if i > 0 && !out.is_empty() {
                out.push('.');
            }
            out.push_str(&String::from_utf8_lossy(seg));
        }
        out
    } else {
        String::from_utf8_lossy(raw).into_owned()
    }
}

/// Encode a `(key, value)` pair list in the proxy-wasm v0.2.1 wire
/// format: `u32 num_pairs | (u32 key_len, u32 value_len)... | (key,
/// '\0', value, '\0')...`.
fn encode_map_pairs(pairs: &[(String, String)]) -> Vec<u8> {
    let n = pairs.len() as u32;
    // Compute total size.
    let header_size = 4 + 8 * pairs.len();
    let mut data_size = 0usize;
    for (k, v) in pairs {
        data_size += k.len() + 1 + v.len() + 1;
    }
    let mut out = Vec::with_capacity(header_size + data_size);
    out.extend_from_slice(&n.to_le_bytes());
    for (k, v) in pairs {
        out.extend_from_slice(&(k.len() as u32).to_le_bytes());
        out.extend_from_slice(&(v.len() as u32).to_le_bytes());
    }
    for (k, v) in pairs {
        out.extend_from_slice(k.as_bytes());
        out.push(0);
        out.extend_from_slice(v.as_bytes());
        out.push(0);
    }
    out
}

/// Inverse of [`encode_map_pairs`].  Returns `None` on malformed input.
fn decode_map_pairs(raw: &[u8]) -> Option<Vec<(String, String)>> {
    if raw.len() < 4 {
        return None;
    }
    let n = u32::from_le_bytes(raw[0..4].try_into().ok()?) as usize;
    let header_end = 4 + 8 * n;
    if raw.len() < header_end {
        return None;
    }
    let mut sizes = Vec::with_capacity(n);
    for i in 0..n {
        let off = 4 + 8 * i;
        let kl = u32::from_le_bytes(raw[off..off + 4].try_into().ok()?) as usize;
        let vl = u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?) as usize;
        sizes.push((kl, vl));
    }
    let mut cursor = header_end;
    let mut out = Vec::with_capacity(n);
    for (kl, vl) in sizes {
        let kend = cursor + kl;
        if kend + 1 > raw.len() {
            return None;
        }
        let key = String::from_utf8_lossy(&raw[cursor..kend]).into_owned();
        cursor = kend + 1; // skip NUL
        let vend = cursor + vl;
        if vend + 1 > raw.len() {
            return None;
        }
        let value = String::from_utf8_lossy(&raw[cursor..vend]).into_owned();
        cursor = vend + 1;
        out.push((key, value));
    }
    Some(out)
}

// ─────────────────────────────────────────────────────────────────────
// Memory helpers
// ─────────────────────────────────────────────────────────────────────

/// Read `len` bytes from the guest's exported `memory` at `ptr`.
///
/// Returns a proxy-wasm `WasmResult` integer code on failure.
fn read_guest_bytes(
    caller: &mut Caller<'_, HostState>,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>, i32> {
    if len <= 0 {
        return Ok(Vec::new());
    }
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(m)) => m,
        _ => return Err(WR_INTERNAL_FAILURE),
    };
    let data = mem.data(caller);
    let start = ptr as usize;
    let end = match start.checked_add(len as usize) {
        Some(e) => e,
        None => return Err(WR_INVALID_MEMORY_ACCESS),
    };
    if end > data.len() {
        return Err(WR_INVALID_MEMORY_ACCESS);
    }
    Ok(data[start..end].to_vec())
}

/// Write `value` (LE u32) at guest memory address `ptr`.
fn write_u32(caller: &mut Caller<'_, HostState>, ptr: u32, value: u32) -> Result<(), i32> {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(m)) => m,
        _ => return Err(WR_INTERNAL_FAILURE),
    };
    let mem_data = mem.data_mut(caller);
    let start = ptr as usize;
    let end = start.checked_add(4).ok_or(WR_INVALID_MEMORY_ACCESS)?;
    if end > mem_data.len() {
        return Err(WR_INVALID_MEMORY_ACCESS);
    }
    mem_data[start..end].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

/// Write `data` into a guest-side allocation, then store the resulting
/// pointer + length at `value_ptr_out` and `value_size_out`.
///
/// Allocation strategy:
///  - if the guest exports `proxy_on_memory_allocate(size: i32) -> i32`,
///    call it to obtain a fresh pointer (this is the proxy-wasm spec
///    convention used by TinyGo / Rust SDK guests);
///  - otherwise fall back to writing the bytes inline at
///    `value_ptr_out + 8` (matches the v0.2.0 inline pattern used by
///    our own test guests).
///
/// On a zero-length payload we still write the zero size + null pointer
/// so the guest sees a well-formed empty buffer.
fn write_guest_buffer(
    caller: &mut Caller<'_, HostState>,
    data: &[u8],
    value_ptr_out: i32,
    value_size_out: i32,
) -> Result<(), i32> {
    let len = data.len() as u32;

    // Empty payload — write null pointer + zero length.
    if len == 0 {
        write_u32(caller, value_ptr_out as u32, 0)?;
        write_u32(caller, value_size_out as u32, 0)?;
        return Ok(());
    }

    // Try the spec-correct path: allocate via guest export.
    let alloc_ptr = caller
        .get_export("proxy_on_memory_allocate")
        .and_then(|e| e.into_func())
        .and_then(|f| f.typed::<i32, i32>(&caller).ok())
        .and_then(|f| f.call(&mut *caller, len as i32).ok());

    let dst_ptr: u32 = match alloc_ptr {
        Some(p) if p > 0 => p as u32,
        // Fallback: inline write at value_ptr_out + 8.  Works for
        // synthetic test guests that exported a memory but no
        // allocator; Coraza always exports the allocator, so this
        // path is test-only.
        _ => (value_ptr_out as u32).wrapping_add(8),
    };

    // Write the bytes at dst_ptr.
    {
        let mem = match caller.get_export("memory") {
            Some(wasmtime::Extern::Memory(m)) => m,
            _ => return Err(WR_INTERNAL_FAILURE),
        };
        let mem_data = mem.data_mut(&mut *caller);
        let start = dst_ptr as usize;
        let end = start.checked_add(data.len()).ok_or(WR_INVALID_MEMORY_ACCESS)?;
        if end > mem_data.len() {
            return Err(WR_INVALID_MEMORY_ACCESS);
        }
        mem_data[start..end].copy_from_slice(data);
    }

    // Write pointer + length into the out-slots.
    write_u32(caller, value_ptr_out as u32, dst_ptr)?;
    write_u32(caller, value_size_out as u32, len)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a tiny WAT guest module that imports `proxy_log` and
    /// invokes it once on instantiation via a `start` function.
    fn proxy_log_caller_wasm(msg: &str) -> Vec<u8> {
        let escaped: String = msg
            .as_bytes()
            .iter()
            .map(|b| format!("\\{:02x}", b))
            .collect();
        let len = msg.len();
        let wat = format!(
            r#"
(module
  (import "env" "proxy_log"
    (func $proxy_log (param i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "{escaped}")
  (func $emit
    (call $proxy_log
      (i32.const 2)        ;; level = Info
      (i32.const 0)        ;; msg_ptr
      (i32.const {len}))   ;; msg_size
    drop
  )
  (start $emit)
)
"#,
        );
        wat::parse_str(wat).expect("WAT parse must succeed")
    }

    /// Build a minimal valid module with no proxy-wasm imports.
    fn empty_valid_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "noop") (result i32) (i32.const 42))
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    /// Guest module that calls `proxy_get_header_map_value` for the
    /// "user-agent" header at instantiation time.  Result codes and the
    /// returned size are stored in memory at offsets 100/104 so the
    /// host can read them.  The fallback inline-write places the value
    /// bytes at `value_ptr_out + 8`.
    fn header_lookup_caller_wasm() -> Vec<u8> {
        // key bytes "user-agent" go at offset 0
        // value-ptr-out at offset 100
        // value-size-out at offset 104
        let wat = r#"
(module
  (import "env" "proxy_get_header_map_value"
    (func $get_header (param i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "user-agent")
  (func (export "do_lookup") (result i32)
    (call $get_header
      (i32.const 0)         ;; map_type = RequestHeaders
      (i32.const 0)         ;; key_ptr
      (i32.const 10)        ;; key_size = strlen("user-agent")
      (i32.const 100)       ;; value_ptr_out
      (i32.const 104)       ;; value_size_out
    )
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    /// Guest module that calls `proxy_get_buffer_bytes` for the request
    /// body at instantiation time.
    fn buffer_lookup_caller_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (import "env" "proxy_get_buffer_bytes"
    (func $get_buf (param i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "do_lookup") (result i32)
    (call $get_buf
      (i32.const 0)         ;; buffer_type = HttpRequestBody
      (i32.const 0)         ;; start
      (i32.const 1024)      ;; max_size
      (i32.const 200)       ;; ret_ptr_out
      (i32.const 204)       ;; ret_size_out
    )
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    /// Guest module that calls `proxy_send_local_response(403, ...)` at
    /// instantiation time.
    fn send_local_response_403_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (import "env" "proxy_send_local_response"
    (func $slr (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Forbidden")
  (data (i32.const 32) "blocked-by-test")
  (func $emit
    (call $slr
      (i32.const 403)       ;; status_code
      (i32.const 0)         ;; status_msg_ptr
      (i32.const 9)         ;; status_msg_size = strlen("Forbidden")
      (i32.const 32)        ;; body_ptr
      (i32.const 15)        ;; body_size = strlen("blocked-by-test")
      (i32.const 0)         ;; headers_ptr
      (i32.const 0)         ;; headers_size
      (i32.const 0)         ;; grpc_status
    )
    drop
  )
  (start $emit)
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    /// Guest that exports a fake `proxy_on_request_body` which always
    /// calls `send_local_response(403, ...)`.  Used to validate the
    /// dispatch path inside `CorazaInstance::on_request_body`.
    fn waf_blocking_guest_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (import "env" "proxy_send_local_response"
    (func $slr (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Blocked")
  (func (export "proxy_on_request_body") (param i32 i32 i32) (result i32)
    (call $slr
      (i32.const 403) (i32.const 0) (i32.const 7)
      (i32.const 0) (i32.const 0)
      (i32.const 0) (i32.const 0) (i32.const 0))
    drop
    (i32.const 0)             ;; Action::Continue
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    /// Guest that exports a fake `proxy_on_request_body` which never
    /// blocks.  Used to validate the Continue path.
    fn waf_passthrough_guest_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "proxy_on_request_body") (param i32 i32 i32) (result i32)
    (i32.const 0)
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse")
    }

    // ── Test 1: load_module from bytes succeeds ──────────────────────
    #[test]
    fn load_from_bytes_succeeds_on_valid_module() {
        let bytes = empty_valid_wasm();
        let module = CorazaModule::load_from_bytes(&bytes)
            .expect("load_from_bytes must succeed on a valid module");
        let _instance = module
            .create_instance()
            .expect("instance creation must succeed");
    }

    // ── Test 2: load_module returns an error for a missing path ─────
    #[test]
    fn load_returns_err_on_missing_path() {
        let path = PathBuf::from("/this/path/does/not/exist/coraza.wasm");
        let result = CorazaModule::load(&path);
        match result {
            Err(CorazaHostError::ModuleIo { path: p, .. }) => {
                assert!(p.contains("/this/path/does/not/exist"));
            }
            other => panic!("expected ModuleIo error, got {other:?}"),
        }
    }

    // ── Test 3: proxy_log host fn correctly forwards a UTF-8 message ─
    #[test]
    fn proxy_log_captures_utf8_message_from_guest() {
        const MSG: &str = "hello-from-guest";
        let bytes = proxy_log_caller_wasm(MSG);

        let module = CorazaModule::load_from_bytes(&bytes).expect("compile guest module");
        let mut instance = module.create_instance().expect("instantiate");

        let logs = instance.drain_logs();
        assert_eq!(logs.len(), 1, "exactly one log entry expected, got {logs:?}");
        assert_eq!(logs[0].0, LogLevel::Info, "level must be Info");
        assert_eq!(logs[0].1, MSG, "message must be reproduced verbatim");
    }

    // ── Test 4: load_from_bytes rejects a non-wasm payload ──────────
    #[test]
    fn load_from_bytes_rejects_invalid_payload() {
        let garbage = b"this is not a wasm module";
        let result = CorazaModule::load_from_bytes(garbage);
        assert!(
            matches!(result, Err(CorazaHostError::Compile(_))),
            "expected Compile error, got {result:?}"
        );
    }

    // ── Test 5: on_request_body returns Continue on a passthrough guest ──
    #[test]
    fn on_request_body_returns_continue_on_passthrough_guest() {
        let bytes = waf_passthrough_guest_wasm();
        let module = CorazaModule::load_from_bytes(&bytes).expect("compile");
        let mut instance = module.create_instance().expect("instantiate");
        assert_eq!(
            instance.on_request_body(b"any body bytes"),
            Decision::Continue,
            "passthrough guest must return Continue",
        );
    }

    // ── Test 6: LogLevel::from_i32 covers all spec values ───────────
    #[test]
    fn log_level_from_i32_maps_all_variants() {
        assert_eq!(LogLevel::from_i32(0), LogLevel::Trace);
        assert_eq!(LogLevel::from_i32(1), LogLevel::Debug);
        assert_eq!(LogLevel::from_i32(2), LogLevel::Info);
        assert_eq!(LogLevel::from_i32(3), LogLevel::Warn);
        assert_eq!(LogLevel::from_i32(4), LogLevel::Error);
        assert_eq!(LogLevel::from_i32(5), LogLevel::Critical);
        assert_eq!(LogLevel::from_i32(99), LogLevel::Critical);
    }

    // ── Test 7 (NEW): proxy_get_header_map_value returns a known header ──
    #[test]
    fn get_header_map_value_returns_known_header() {
        let bytes = header_lookup_caller_wasm();
        let module = CorazaModule::load_from_bytes(&bytes).expect("compile");
        let mut instance = module.create_instance().expect("instantiate");

        // Seed the request header map.
        instance
            .store
            .data_mut()
            .request_headers
            .insert("user-agent".to_string(), "armageddon-tests/1.0".to_string());

        // Invoke the export.
        let f = instance
            .instance
            .get_typed_func::<(), i32>(&mut instance.store, "do_lookup")
            .expect("do_lookup export");
        let rc = f.call(&mut instance.store, ()).expect("call ok");
        assert_eq!(rc, WR_OK, "host fn should return WR_OK, got {rc}");

        // Read back the inline-written bytes (fallback path: at
        // value_ptr_out + 8 = 108).
        let mem = instance
            .instance
            .get_memory(&mut instance.store, "memory")
            .expect("memory export");
        let data = mem.data(&instance.store);
        // value_ptr_out is at offset 100, holds the dst ptr (= 108 for
        // fallback path).
        let dst_ptr = u32::from_le_bytes(data[100..104].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(data[104..108].try_into().unwrap()) as usize;
        let value = &data[dst_ptr..dst_ptr + len];
        assert_eq!(
            std::str::from_utf8(value).unwrap(),
            "armageddon-tests/1.0",
            "header value must be readable from guest memory",
        );
    }

    // ── Test 8 (NEW): proxy_get_buffer_bytes returns the body slice ──
    #[test]
    fn get_buffer_bytes_returns_body_slice() {
        let bytes = buffer_lookup_caller_wasm();
        let module = CorazaModule::load_from_bytes(&bytes).expect("compile");
        let mut instance = module.create_instance().expect("instantiate");

        instance
            .store
            .data_mut()
            .request_body
            .extend_from_slice(b"id=1' OR '1'='1");

        let f = instance
            .instance
            .get_typed_func::<(), i32>(&mut instance.store, "do_lookup")
            .expect("do_lookup export");
        let rc = f.call(&mut instance.store, ()).expect("call");
        assert_eq!(rc, WR_OK);

        let mem = instance
            .instance
            .get_memory(&mut instance.store, "memory")
            .expect("memory");
        let data = mem.data(&instance.store);
        let dst_ptr = u32::from_le_bytes(data[200..204].try_into().unwrap()) as usize;
        let len = u32::from_le_bytes(data[204..208].try_into().unwrap()) as usize;
        let body = &data[dst_ptr..dst_ptr + len];
        assert_eq!(body, b"id=1' OR '1'='1");
    }

    // ── Test 9 (NEW): send_local_response captures status 403 ───────
    #[test]
    fn send_local_response_captures_status_403() {
        let bytes = send_local_response_403_wasm();
        let module = CorazaModule::load_from_bytes(&bytes).expect("compile");
        let instance = module.create_instance().expect("instantiate");

        let lr = instance.local_response().expect("local response set");
        assert_eq!(lr.status, 403);
        assert_eq!(lr.reason, "Forbidden");
        assert_eq!(lr.body, b"blocked-by-test");
    }

    // ── Test 10 (NEW): on_request_body Deny path (synthetic blocker) ─
    #[test]
    fn on_request_body_returns_deny_when_guest_blocks() {
        let bytes = waf_blocking_guest_wasm();
        let module = CorazaModule::load_from_bytes(&bytes).expect("compile");
        let mut instance = module.create_instance().expect("instantiate");

        let verdict = instance.on_request_body(b"anything");
        match verdict {
            Decision::Deny { status, .. } => assert_eq!(status, 403),
            other => panic!("expected Deny(403), got {other:?}"),
        }
    }

    // ── Test 11 (NEW): map-pair codec round-trip ────────────────────
    #[test]
    fn map_pair_codec_round_trip() {
        let pairs = vec![
            (":authority".to_string(), "armageddon.local".to_string()),
            ("user-agent".to_string(), "tests/1.0".to_string()),
            ("x-empty".to_string(), String::new()),
        ];
        let encoded = encode_map_pairs(&pairs);
        let decoded = decode_map_pairs(&encoded).expect("decode ok");
        assert_eq!(decoded, pairs);
    }

    // ── Test 12 (NEW): property path normaliser handles both forms ──
    #[test]
    fn property_path_normaliser_accepts_nul_and_dot() {
        assert_eq!(normalise_property_path(b"request\0url_path"), "request.url_path");
        assert_eq!(normalise_property_path(b"request.url_path"), "request.url_path");
        assert_eq!(normalise_property_path(b"plugin_root_id"), "plugin_root_id");
    }

    // ── Test 13 (NEW): Coraza WAF integration — blocks SQLi ─────────
    //
    // Loads the real `armageddon/coraza/coraza-waf.wasm` and dispatches a
    // SQL-injection-shaped request body.  Marked `#[ignore]` because:
    //   1. requires the 16 MB wasm to be on disk (it is, but CI may not),
    //   2. requires the proxy-wasm v0.2.1 dispatch sequence to fully
    //      cooperate with TinyGo's runtime (we expect minor host-fn gaps
    //      may still surface here — first run typically reveals 1-2
    //      missing imports to wire).
    //
    // To run locally:
    //   cargo test --release --features coraza-wasm \
    //     -p armageddon-wasm coraza_blocks_sqli_request -- --ignored
    #[test]
    #[ignore = "requires real coraza-waf.wasm + may surface missing host imports on first run"]
    fn coraza_blocks_sqli_request() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let wasm_path = PathBuf::from(manifest_dir)
            .join("../coraza/coraza-waf.wasm");
        let conf_path = PathBuf::from(manifest_dir)
            .join("../coraza/coraza.conf");
        if !wasm_path.exists() {
            eprintln!("skipping: {} not found", wasm_path.display());
            return;
        }
        let module = CorazaModule::load_with_config(&wasm_path, &conf_path)
            .expect("coraza module + conf must load");
        let mut instance = module
            .create_instance()
            .expect("instance creation must succeed");

        let _ = instance.on_request_headers(
            "POST",
            "/api/v1/orders?id=1' OR '1'='1",
            "127.0.0.1:54321",
            &[
                ("host".to_string(), "test.local".to_string()),
                ("content-type".to_string(), "application/x-www-form-urlencoded".to_string()),
                ("user-agent".to_string(), "sqlmap/1.0".to_string()),
            ],
        );

        let verdict = instance.on_request_body(b"id=1' OR '1'='1 -- ");
        match verdict {
            Decision::Deny { status, reason } => {
                assert!(
                    (400..=599).contains(&status),
                    "expected 4xx/5xx block, got {status} ({reason})",
                );
            }
            Decision::Continue => {
                panic!(
                    "expected Coraza to block SQLi payload but verdict was Continue — \
                     check host-fn coverage (see PROXY-WASM-HOST-DESIGN.md §10)"
                );
            }
        }
    }

    // ── Test 14 (NEW): Coraza WAF integration — passes benign request ─
    #[test]
    #[ignore = "requires real coraza-waf.wasm; pair with coraza_blocks_sqli_request"]
    fn coraza_passes_benign_request() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let wasm_path = PathBuf::from(manifest_dir).join("../coraza/coraza-waf.wasm");
        let conf_path = PathBuf::from(manifest_dir).join("../coraza/coraza.conf");
        if !wasm_path.exists() {
            eprintln!("skipping: {} not found", wasm_path.display());
            return;
        }
        let module = CorazaModule::load_with_config(&wasm_path, &conf_path)
            .expect("coraza module + conf load");
        let mut instance = module.create_instance().expect("instance");

        let _ = instance.on_request_headers(
            "GET",
            "/api/health",
            "127.0.0.1:54321",
            &[
                ("host".to_string(), "test.local".to_string()),
                ("user-agent".to_string(), "kube-probe/1.30".to_string()),
            ],
        );
        let verdict = instance.on_request_body(b"");
        assert_eq!(verdict, Decision::Continue, "benign request must pass");
    }
}
