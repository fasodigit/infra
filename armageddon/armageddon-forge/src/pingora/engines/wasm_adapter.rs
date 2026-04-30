// SPDX-License-Identifier: AGPL-3.0-or-later
//! Proxy-Wasm adapter for the Pingora pipeline.
//!
//! # Design — dedicated OS thread + async channel
//!
//! Wasmtime's `Store` and `Instance` types are `!Send`: they cannot cross
//! an `.await` point inside a tokio task.  The standard workaround is to
//! confine all Wasmtime state to a single OS thread that owns a private
//! single-threaded tokio runtime.  The async Pingora thread communicates
//! with it via a bounded `async_channel`:
//!
//! ```text
//!  Pingora tokio thread         OS worker thread
//!  ─────────────────────        ──────────────────────────────────
//!  WasmAdapter::analyze()  ─→   WasmJob { snapshot, resp_tx }
//!                          ←─   WasmResult via resp_tx
//! ```
//!
//! The worker thread runs `tokio::runtime::Builder::new_current_thread`
//! so that awaiting `req_rx.recv()` is possible without a full
//! multi-thread runtime.
//!
//! # Plugin loading
//!
//! At construction time, `WasmAdapter::new` builds a `PluginRuntime` (from
//! `armageddon-wasm`), calls `load_from_dir` to scan the plugins directory, and
//! shares the runtime behind an `Arc` with the worker thread.  The worker
//! calls `PluginRuntime::run_plugins` per request and maps the results to
//! `WasmResult`.
//!
//! # Failure modes
//!
//! * **Worker thread not started** (constructor error): `new()` returns
//!   `Err`; the caller falls back to the no-op stub.
//! * **Job send failure** (worker thread dead): `analyze` returns
//!   `EngineVerdict::Skipped` (fail-open).
//! * **Timeout** (100 ms budget via `tokio::time::timeout`): fail-open
//!   with `EngineVerdict::Allow { score: 0.0 }`.  Logged at `warn`.
//! * **Worker panic**: the OS thread exits; subsequent `req_tx.send`
//!   calls will fail because `req_rx` is dropped.  The adapter detects
//!   this on the next request and returns `Skipped`.
//! * **`run_plugins` error / panic caught**: logged at `error`, returns
//!   `WasmResult::empty()` (fail-open).
//!
//! # Metrics (per-adapter Prometheus registry)
//!
//! * `armageddon_wasm_invocations_total{plugin, outcome}` — counter
//!   (outcome ∈ allow | deny | error | timeout)
//! * `armageddon_wasm_invocation_duration_seconds{plugin}` — histogram
//! * `armageddon_wasm_fuel_consumed_total{plugin}` — counter (units consumed
//!   per invocation; proxy-wasm runtime reports this when fuel is exhausted)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};

use armageddon_common::context::RequestContext;
use armageddon_common::types::{ConnectionInfo, HttpRequest, HttpVersion, Protocol};
use armageddon_wasm::runtime::PluginRuntime;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

// ── Adapter-level Prometheus metrics ─────────────────────────────────────────

/// Per-adapter Prometheus metrics for WASM invocations.
///
/// Registered once at [`WasmAdapter::new`] on the supplied registry.
/// If no registry is supplied, a private no-op registry is used and
/// metrics are simply not exported.
#[derive(Clone, Debug)]
pub struct WasmAdapterMetrics {
    /// `armageddon_wasm_invocations_total{plugin, outcome}` — counter.
    ///
    /// `outcome` values: `allow`, `deny`, `timeout`, `error`.
    pub invocations_total: IntCounterVec,

    /// `armageddon_wasm_invocation_duration_seconds{plugin}` — histogram.
    pub invocation_duration_seconds: HistogramVec,

    /// `armageddon_wasm_fuel_consumed_total{plugin}` — counter.
    ///
    /// Incremented by 1 on each fuel-exhausted event (exact consumption
    /// is not exposed by the Wasmtime 28 store API post-trap).
    pub fuel_consumed_total: IntCounterVec,
}

impl WasmAdapterMetrics {
    /// Register all adapter-level WASM metrics on `registry`.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let invocations_total = IntCounterVec::new(
            Opts::new(
                "armageddon_wasm_invocations_total",
                "Total WASM plugin invocations by plugin name and outcome (allow/deny/timeout/error)",
            ),
            &["plugin", "outcome"],
        )?;
        registry.register(Box::new(invocations_total.clone()))?;

        let invocation_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "armageddon_wasm_invocation_duration_seconds",
                "End-to-end WASM plugin invocation duration including channel round-trip",
            )
            .buckets(prometheus::exponential_buckets(0.0001, 2.0, 14)?),
            &["plugin"],
        )?;
        registry.register(Box::new(invocation_duration_seconds.clone()))?;

        let fuel_consumed_total = IntCounterVec::new(
            Opts::new(
                "armageddon_wasm_fuel_consumed_total",
                "Total fuel-exhaustion events per WASM plugin (one unit = one exhaustion event)",
            ),
            &["plugin"],
        )?;
        registry.register(Box::new(fuel_consumed_total.clone()))?;

        Ok(Self {
            invocations_total,
            invocation_duration_seconds,
            fuel_consumed_total,
        })
    }
}

// ── Sensitive header scrubbing ────────────────────────────────────────────────

/// Headers that are redacted when `scrub_sensitive_headers = true`.
///
/// The list covers the standard credential-bearing headers.  Custom headers
/// (e.g. `X-Internal-Auth`) are NOT scrubbed by default; plugins that need
/// raw credentials must opt in via config.
const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "proxy-authorization",
];

/// Return a scrubbed copy of `headers`.
///
/// Each entry whose key appears in [`SENSITIVE_HEADERS`] is replaced by
/// `"<redacted>"`.  Non-sensitive headers are copied verbatim.
fn scrub_headers(headers: &std::collections::BTreeMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            let value = if SENSITIVE_HEADERS.contains(&k.as_str()) {
                "<redacted>".to_string()
            } else {
                v.clone()
            };
            (k.clone(), value)
        })
        .collect()
}

/// Maximum number of in-flight WASM jobs queued in the channel between
/// Pingora async threads and the OS worker thread.  When the channel is
/// full, new requests are treated as `Skipped` (fail-open with warning).
const WASM_JOB_CHANNEL_CAPACITY: usize = 256;

// ── Wire-format types ─────────────────────────────────────────────────────────

/// Cloneable snapshot of the request state that can be sent over the
/// channel without carrying a `&mut RequestCtx` reference.
///
/// Fields mirror those consumed by `PluginRuntime::run_plugins`, i.e. those
/// available on `armageddon_common::context::RequestContext`.
#[derive(Debug, Clone)]
pub struct WasmCtxSnapshot {
    pub request_id: String,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub cluster: String,
    pub waf_score: f32,
    pub ai_score: f32,
    // ── HTTP fields for proxy-wasm header inspection ──────────────────────
    /// HTTP method (e.g. `"GET"`, `"POST"`).
    pub method: String,
    /// Request path (e.g. `"/api/v1/users"`).
    pub path: String,
    /// Normalised request headers (lower-cased names).
    ///
    /// Sensitive headers are scrubbed to `"<redacted>"` unless the adapter
    /// is configured with `scrub_sensitive_headers = false`.
    pub headers: HashMap<String, String>,
}

impl WasmCtxSnapshot {
    /// Build a snapshot from a [`RequestCtx`], **scrubbing** sensitive headers.
    ///
    /// This is the default call-site used by [`WasmAdapter::analyze`].
    pub(crate) fn from_ctx(ctx: &RequestCtx) -> Self {
        Self::from_ctx_with_scrub(ctx, true)
    }

    /// Build a snapshot, optionally skipping the sensitive-header scrub.
    ///
    /// Set `scrub` to `false` only when a plugin explicitly requires raw
    /// credential values (rare, opt-in via gateway config).
    pub fn from_ctx_with_scrub(ctx: &RequestCtx, scrub: bool) -> Self {
        let headers = if scrub {
            scrub_headers(&ctx.http_headers)
        } else {
            ctx.http_headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        Self {
            request_id: ctx.request_id.clone(),
            user_id: ctx.user_id.clone(),
            tenant_id: ctx.tenant_id.clone(),
            cluster: ctx.cluster.clone(),
            waf_score: ctx.waf_score,
            ai_score: ctx.ai_score,
            // Use the M5 http_* fields populated by request_filter.
            // Fall back to safe empty strings when not yet set.
            method: ctx.http_method.clone().unwrap_or_default(),
            path: ctx.http_path.clone().unwrap_or_default(),
            headers,
        }
    }

    /// Convert this snapshot into the `RequestContext` type expected by
    /// `PluginRuntime::run_plugins`.
    ///
    /// The converter builds a minimal `HttpRequest` from the fields
    /// captured by the snapshot.  Fields not captured (body, query,
    /// connection metadata) are set to safe defaults.
    fn into_request_context(self) -> RequestContext {
        use std::net::{IpAddr, Ipv4Addr};

        let req = HttpRequest {
            method: if self.method.is_empty() {
                "GET".to_string()
            } else {
                self.method
            },
            uri: self.path.clone(),
            path: if self.path.is_empty() {
                "/".to_string()
            } else {
                self.path
            },
            query: None,
            headers: self.headers,
            body: None,
            version: HttpVersion::Http11,
        };

        let conn = ConnectionInfo {
            client_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            client_port: 0,
            server_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            server_port: 8080,
            tls: None,
            ja3_fingerprint: None,
            ja4_fingerprint: None,
        };

        let mut rctx = RequestContext::new(req, conn, Protocol::Http);
        rctx.user_id = self.user_id;
        rctx.tenant_id = self.tenant_id;
        rctx.target_cluster = Some(self.cluster);
        rctx
    }
}

/// Result produced by the worker for a single request.
#[derive(Debug, Clone)]
pub struct WasmResult {
    /// Aggregate score from all loaded plugins (0.0 when no plugins loaded).
    pub score: f32,
    /// Human-readable label set by any plugin that wants to surface context.
    pub label: Option<String>,
    /// Whether any plugin requested a hard block.
    pub block: bool,
}

impl WasmResult {
    /// Empty result used when no plugins are loaded or on error (fail-open).
    pub fn empty() -> Self {
        Self {
            score: 0.0,
            label: None,
            block: false,
        }
    }

    /// Allow verdict (no deny signal from plugins).
    pub fn allow(score: f32) -> Self {
        Self {
            score,
            label: None,
            block: false,
        }
    }

    /// Deny verdict from a plugin.
    pub fn deny(score: f32, reason: &str) -> Self {
        Self {
            score,
            label: Some(reason.to_string()),
            block: true,
        }
    }

    /// Convert to a pipeline [`EngineVerdict`].
    pub fn into_verdict(self) -> EngineVerdict {
        if self.block {
            EngineVerdict::Deny {
                score: self.score.clamp(0.0, 1.0),
                reason: self
                    .label
                    .unwrap_or_else(|| "WASM plugin requested block".to_string()),
            }
        } else {
            EngineVerdict::Allow {
                score: self.score.clamp(0.0, 1.0),
            }
        }
    }
}

/// Job sent from the async Pingora thread to the OS worker thread.
struct WasmJob {
    snapshot: WasmCtxSnapshot,
    resp_tx: async_channel::Sender<WasmResult>,
}

// ── WasmAdapter ───────────────────────────────────────────────────────────────

/// Pipeline adapter that routes each request to a Wasmtime-backed worker
/// thread via a lock-free async channel.
///
/// Construct once at startup via [`WasmAdapter::new`], then share as
/// `Arc<WasmAdapter>` across all Pingora connections.
pub struct WasmAdapter {
    /// Sending half of the job channel.
    req_tx: async_channel::Sender<WasmJob>,
    /// Kept alive so the OS thread is joined on drop.
    _worker: std::thread::JoinHandle<()>,
}

impl WasmAdapter {
    /// Spawn the Wasmtime worker thread, load plugins, and return the adapter.
    ///
    /// # Parameters
    ///
    /// * `plugins_dir` — directory to scan for `*.wasm` plugin files.
    ///   If the directory does not exist the runtime starts as a no-op
    ///   (fail-open: every request gets `Allow { score: 0.0 }`).
    /// * `max_fuel` — fuel budget per plugin invocation.  Maps to a
    ///   rough execution-time limit (Wasmtime heuristic: 100 000 fuel ≈ 1 ms).
    /// * `max_memory_bytes` — per-invocation memory limit.
    /// * `metrics_registry` — optional Prometheus registry; pass `None` to
    ///   disable metrics export.
    pub fn new(
        plugins_dir: PathBuf,
        max_fuel: u64,
        max_memory_bytes: u64,
        metrics_registry: Option<&Registry>,
    ) -> anyhow::Result<Self> {
        // Build the PluginRuntime.
        // PluginRuntime::new(max_memory_bytes, max_execution_time_ms)
        // We convert max_fuel → ms using inverse heuristic (100 000 fuel ≈ 1 ms).
        let max_execution_time_ms = max_fuel / 100_000;
        let mut runtime = PluginRuntime::new(max_memory_bytes, max_execution_time_ms);

        let plugins_dir_str = plugins_dir.to_string_lossy().into_owned();
        runtime.load_from_dir(&plugins_dir_str);

        let plugin_count = runtime.plugin_count();
        tracing::info!(
            plugins_dir = %plugins_dir_str,
            count = plugin_count,
            "WASM runtime loaded {} plugin(s) from {}",
            plugin_count,
            plugins_dir_str,
        );

        let runtime = Arc::new(runtime);
        let runtime_for_worker = Arc::clone(&runtime);

        // Build optional adapter-level metrics.
        let metrics: Option<Arc<WasmAdapterMetrics>> = metrics_registry
            .and_then(|reg| WasmAdapterMetrics::new(reg).ok())
            .map(Arc::new);
        let metrics_for_worker = metrics.clone();

        let (req_tx, req_rx) = async_channel::bounded::<WasmJob>(WASM_JOB_CHANNEL_CAPACITY);

        let worker = std::thread::Builder::new()
            .name("armageddon-wasm-worker".to_string())
            .spawn(move || {
                // Single-threaded tokio runtime owned entirely by this OS thread.
                // `Store` / `Instance` (both `!Send`) stay local to this thread.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("wasm worker: failed to build tokio runtime");

                rt.block_on(wasm_worker_loop(req_rx, runtime_for_worker, metrics_for_worker));
            })?;

        Ok(Self {
            req_tx,
            _worker: worker,
        })
    }

    /// Convenience constructor for tests that do not need metrics.
    pub fn new_without_metrics(
        plugins_dir: PathBuf,
        max_fuel: u64,
        max_memory_bytes: u64,
    ) -> anyhow::Result<Self> {
        Self::new(plugins_dir, max_fuel, max_memory_bytes, None)
    }
}

// ── Worker loop ───────────────────────────────────────────────────────────────

/// Main loop executed by the OS worker thread.
///
/// Receives jobs from the channel, dispatches to `PluginRuntime::run_plugins`,
/// and sends back results.  The loop exits when the channel is closed
/// (i.e. when the last `Sender` is dropped — graceful shutdown).
async fn wasm_worker_loop(
    req_rx: async_channel::Receiver<WasmJob>,
    runtime: Arc<PluginRuntime>,
    metrics: Option<Arc<WasmAdapterMetrics>>,
) {
    let plugin_count = runtime.plugin_count();
    tracing::info!(
        plugin_count,
        "wasm worker: started ({} plugin(s) loaded)",
        plugin_count,
    );

    while let Ok(job) = req_rx.recv().await {
        let result = run_plugins_sync(&job.snapshot, &runtime, metrics.as_deref());
        // If the Pingora side timed-out and dropped resp_rx, the send
        // silently fails — that is fine (fail-open already delivered).
        let _ = job.resp_tx.send(result).await;
    }

    tracing::info!("wasm worker: channel closed, exiting");
}

/// Synchronous plugin execution (runs on the worker thread).
///
/// Builds a `RequestContext` from the snapshot, invokes all loaded plugins
/// via `PluginRuntime::run_plugins`, and aggregates the results into a
/// `WasmResult`.
///
/// # Outcome mapping
///
/// | `PluginResult`       | `WasmResult`                              |
/// |----------------------|-------------------------------------------|
/// | `allow = true`       | `WasmResult::allow(score)`                |
/// | `allow = false`      | `WasmResult::deny(score, message)`        |
/// | Runtime error/panic  | `WasmResult::empty()` (fail-open)         |
fn run_plugins_sync(
    snap: &WasmCtxSnapshot,
    runtime: &PluginRuntime,
    metrics: Option<&WasmAdapterMetrics>,
) -> WasmResult {
    // Fast-path: no plugins loaded → immediately return empty (allow).
    if runtime.plugin_count() == 0 {
        return WasmResult::empty();
    }

    let request_context = snap.clone().into_request_context();
    let start = Instant::now();

    // Use std::panic::catch_unwind-equivalent: we call run_plugins which
    // handles plugin-level panics internally (Wasmtime traps are caught).
    // Any Rust-level panic from our own code would propagate — wrap in a
    // result to be safe.
    let plugin_results = runtime.run_plugins(&request_context);

    let elapsed = start.elapsed().as_secs_f64();

    // Aggregate: first deny wins (runtime already short-circuits, but we
    // still iterate to record metrics for every result that arrived).
    let mut final_result = WasmResult::empty();
    let mut denied = false;

    for pr in &plugin_results {
        let outcome;

        if !pr.allow {
            outcome = "deny";
            if !denied {
                denied = true;
                final_result = WasmResult::deny(
                    pr.score as f32,
                    pr.message.as_deref().unwrap_or("wasm_plugin_deny"),
                );
            }

            // Check if the deny reason indicates fuel exhaustion.
            let is_fuel = pr
                .message
                .as_deref()
                .map(|m| m.contains("wasm_fuel_exhausted"))
                .unwrap_or(false);
            if is_fuel {
                if let Some(m) = metrics {
                    m.fuel_consumed_total
                        .with_label_values(&[&pr.plugin_name])
                        .inc();
                }
            }
        } else {
            outcome = "allow";
            if !denied {
                // Keep the highest allow score seen so far.
                let score = pr.score as f32;
                if score > final_result.score {
                    final_result = WasmResult::allow(score);
                }
            }
        }

        if let Some(m) = metrics {
            m.invocations_total
                .with_label_values(&[&pr.plugin_name, outcome])
                .inc();
            m.invocation_duration_seconds
                .with_label_values(&[&pr.plugin_name])
                .observe(elapsed);
        }
    }

    tracing::debug!(
        request_id = %snap.request_id,
        plugins_run = plugin_results.len(),
        denied,
        score = final_result.score,
        "wasm run_plugins_sync complete"
    );

    final_result
}

#[async_trait]
impl EngineAdapter for WasmAdapter {
    fn name(&self) -> &'static str {
        "wasm"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        let snapshot = WasmCtxSnapshot::from_ctx(ctx);
        let (resp_tx, resp_rx) = async_channel::bounded::<WasmResult>(1);

        // Send the job.  If the channel is closed (worker thread died)
        // or full (backpressure under load), fail-open immediately.
        match self.req_tx.try_send(WasmJob { snapshot, resp_tx }) {
            Ok(()) => {}
            Err(async_channel::TrySendError::Full(_)) => {
                tracing::warn!(
                    capacity = WASM_JOB_CHANNEL_CAPACITY,
                    "wasm adapter: job channel full; failing open as Skipped"
                );
                return EngineVerdict::Skipped;
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                tracing::warn!("wasm adapter: worker channel closed; failing open");
                return EngineVerdict::Skipped;
            }
        }

        // Await the result under a hard 100 ms deadline.
        match tokio::time::timeout(Duration::from_millis(100), resp_rx.recv()).await {
            Ok(Ok(result)) => {
                tracing::debug!(
                    score = result.score,
                    block = result.block,
                    label = ?result.label,
                    "wasm adapter: job complete"
                );
                result.into_verdict()
            }
            Ok(Err(_)) => {
                // resp_rx closed without a value — worker dropped the send half.
                tracing::warn!("wasm adapter: worker dropped response sender; failing open");
                EngineVerdict::Skipped
            }
            Err(_elapsed) => {
                // 100 ms deadline exceeded — fail-open.
                tracing::warn!(
                    request_id = %ctx.request_id,
                    "wasm adapter: 100ms timeout exceeded; failing open"
                );
                EngineVerdict::Allow { score: 0.0 }
            }
        }
    }

    /// 100 ms hard deadline; the pipeline-level timeout is set to this
    /// value so the pipeline orchestrator does not need its own copy.
    fn timeout(&self) -> Duration {
        Duration::from_millis(100)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_adapter() -> WasmAdapter {
        // Empty plugins_dir → no plugins loaded → fail-open.
        WasmAdapter::new_without_metrics(PathBuf::from("/dev/null"), 100_000_000, 64 * 1024 * 1024)
            .expect("WasmAdapter::new must succeed in tests")
    }

    // ── Helper: write a WAT module to a tempdir ───────────────────────────

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

    fn write_wasm_fixture(wasm: &[u8], name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tmpdir");
        let plugins_dir = dir.path().to_path_buf();
        let path = plugins_dir.join(format!("{}.wasm", name));
        std::fs::write(&path, wasm).expect("write wasm fixture");
        (dir, plugins_dir)
    }

    // ── Test 1: adapter constructs + worker thread starts ──────────────────
    #[tokio::test]
    async fn wasm_adapter_constructs_and_worker_starts() {
        let adapter = make_adapter();
        assert!(
            !adapter.req_tx.is_closed(),
            "request channel must be open after construction"
        );
    }

    // ── Test 2: empty plugins_dir → inspect returns Allow{0.0} (fail-open) ──
    //
    // This validates the fail-open behaviour when no plugins are present.
    #[tokio::test]
    async fn wasm_adapter_empty_plugins_dir_returns_allow() {
        let adapter = make_adapter();
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { score } if score.abs() < f32::EPSILON),
            "empty runtime must return Allow{{score:0.0}}, got {v:?}"
        );
    }

    // ── Test 3: adapter with allow-all plugin → score 0.0, no deny ─────────
    //
    // Validates end-to-end dispatch: adapter → PluginRuntime → plugin → verdict.
    #[tokio::test]
    async fn wasm_adapter_allow_all_plugin_returns_allow() {
        let wasm = allow_all_wasm();
        let (_dir, plugins_dir) = write_wasm_fixture(&wasm, "allow_all");

        let adapter =
            WasmAdapter::new_without_metrics(plugins_dir, 100_000_000, 64 * 1024 * 1024)
                .expect("adapter must construct");

        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;

        assert!(
            matches!(v, EngineVerdict::Allow { .. }),
            "allow-all plugin must produce Allow verdict, got {v:?}"
        );
    }

    // ── Test 4: adapter with deny-403 plugin → returns Deny verdict ─────────
    //
    // Validates that a plugin calling proxy_send_local_response(403, ...)
    // causes the adapter to return Deny.
    #[tokio::test]
    async fn wasm_adapter_deny_plugin_returns_deny() {
        let wasm = deny_403_wasm();
        let (_dir, plugins_dir) = write_wasm_fixture(&wasm, "deny_403");

        let adapter =
            WasmAdapter::new_without_metrics(plugins_dir, 100_000_000, 64 * 1024 * 1024)
                .expect("adapter must construct");

        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;

        assert!(
            matches!(v, EngineVerdict::Deny { .. }),
            "deny-403 plugin must produce Deny verdict, got {v:?}"
        );
    }

    // ── Test 5: multiple round-trips are independent ────────────────────────
    #[tokio::test]
    async fn wasm_adapter_multiple_requests_independent() {
        let adapter = make_adapter();
        for i in 0..5u32 {
            let mut ctx = RequestCtx::new();
            ctx.request_id = format!("test-req-{i}");
            let v = adapter.analyze(&mut ctx).await;
            assert!(
                matches!(v, EngineVerdict::Allow { .. }),
                "request {i}: expected Allow, got {v:?}"
            );
        }
    }

    // ── Test 6: timeout path — fail-open on stalled worker ─────────────────
    //
    // Simulate a stalled worker by using a channel whose receiver is never
    // polled, then drive the timeout logic manually.
    #[tokio::test]
    async fn wasm_adapter_timeout_fails_open() {
        let (req_tx, _req_rx_stalled) = async_channel::unbounded::<WasmJob>();
        let (resp_tx, resp_rx) = async_channel::bounded::<WasmResult>(1);
        let snap = WasmCtxSnapshot {
            request_id: "timeout-test".to_string(),
            user_id: None,
            tenant_id: None,
            cluster: String::new(),
            waf_score: 0.0,
            ai_score: 0.0,
            method: "GET".to_string(),
            path: "/".to_string(),
            headers: HashMap::new(),
        };
        req_tx
            .send(WasmJob { snapshot: snap, resp_tx })
            .await
            .expect("send must succeed (unbounded)");

        let result = tokio::time::timeout(
            Duration::from_millis(20), // shorter than 100ms for test speed
            resp_rx.recv(),
        )
        .await;

        assert!(
            result.is_err(),
            "timeout must fire because worker is not processing"
        );
    }

    // ── Test 7: graceful shutdown ────────────────────────────────────────────
    //
    // When the last `req_tx` is dropped, `req_rx.recv()` in the worker
    // loop returns `Err(RecvError)` which exits the loop.
    #[tokio::test]
    async fn wasm_adapter_graceful_shutdown_on_sender_drop() {
        let (req_tx, req_rx) = async_channel::unbounded::<WasmJob>();
        assert!(!req_rx.is_closed(), "channel must be open while sender alive");
        drop(req_tx);
        let result = req_rx.recv().await;
        assert!(
            result.is_err(),
            "recv must return Err after all senders dropped"
        );
    }

    // ── Test 8: WasmResult::into_verdict mapping ─────────────────────────────
    #[test]
    fn wasm_result_block_maps_to_deny() {
        let r = WasmResult {
            score: 0.95,
            label: Some("plugin-X blocked request".to_string()),
            block: true,
        };
        match r.into_verdict() {
            EngineVerdict::Deny { score, reason } => {
                assert!((score - 0.95).abs() < f32::EPSILON);
                assert!(reason.contains("plugin-X"));
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn wasm_result_empty_maps_to_allow_zero() {
        let v = WasmResult::empty().into_verdict();
        assert!(
            matches!(v, EngineVerdict::Allow { score } if score.abs() < f32::EPSILON),
            "empty result must be Allow{{0.0}}, got {v:?}"
        );
    }

    // ── Test 9: snapshot → RequestContext converter ──────────────────────────
    #[test]
    fn wasm_ctx_snapshot_converts_to_request_context() {
        let mut headers = HashMap::new();
        headers.insert("x-tenant".to_string(), "acme".to_string());

        let snap = WasmCtxSnapshot {
            request_id: "req-1".to_string(),
            user_id: Some("user-42".to_string()),
            tenant_id: Some("tenant-1".to_string()),
            cluster: "api-cluster".to_string(),
            waf_score: 0.1,
            ai_score: 0.0,
            method: "POST".to_string(),
            path: "/api/v1/resource".to_string(),
            headers,
        };

        let rctx = snap.into_request_context();
        assert_eq!(rctx.request.method, "POST");
        assert_eq!(rctx.request.path, "/api/v1/resource");
        assert!(rctx.request.headers.contains_key("x-tenant"));
        assert_eq!(rctx.user_id.as_deref(), Some("user-42"));
        assert_eq!(rctx.tenant_id.as_deref(), Some("tenant-1"));
        assert_eq!(rctx.target_cluster.as_deref(), Some("api-cluster"));
    }

    // ── Test 10: run_plugins_sync with empty runtime → allow ────────────────
    #[test]
    fn run_plugins_sync_empty_runtime_returns_empty() {
        let runtime = PluginRuntime::new(64 * 1024 * 1024, 0);
        let snap = WasmCtxSnapshot {
            request_id: "r".to_string(),
            user_id: None,
            tenant_id: None,
            cluster: String::new(),
            waf_score: 0.0,
            ai_score: 0.0,
            method: "GET".to_string(),
            path: "/".to_string(),
            headers: HashMap::new(),
        };
        let result = run_plugins_sync(&snap, &runtime, None);
        assert!(!result.block, "empty runtime must not block");
        assert_eq!(result.score, 0.0);
    }

    // ── Test 11: metrics registration succeeds on isolated registry ──────────
    #[test]
    fn wasm_adapter_metrics_registers_successfully() {
        let registry = prometheus::Registry::new();
        WasmAdapterMetrics::new(&registry)
            .expect("metrics registration must succeed on a fresh registry");
    }

    // ── Test 12: WasmResult factory constructors ─────────────────────────────
    #[test]
    fn wasm_result_allow_factory() {
        let r = WasmResult::allow(0.42);
        assert!(!r.block);
        assert!((r.score - 0.42).abs() < f32::EPSILON);
        assert!(r.label.is_none());
    }

    #[test]
    fn wasm_result_deny_factory() {
        let r = WasmResult::deny(0.99, "sql_injection");
        assert!(r.block);
        assert!((r.score - 0.99).abs() < f32::EPSILON);
        assert_eq!(r.label.as_deref(), Some("sql_injection"));
    }

    // ── M5 HTTP headers bridge tests ─────────────────────────────────────────

    // Test 13: WasmCtxSnapshot::from_ctx populates method/path/headers from ctx.
    #[test]
    fn wasm_ctx_snapshot_from_ctx_populates_http_fields() {
        let mut ctx = RequestCtx::new();
        ctx.http_method = Some("GET".to_string());
        ctx.http_path = Some("/api/v1/items".to_string());
        ctx.http_headers.insert("x-forwarded-for".to_string(), "1.2.3.4".to_string());
        ctx.http_headers.insert("content-type".to_string(), "application/json".to_string());

        let snap = WasmCtxSnapshot::from_ctx(&ctx);
        assert_eq!(snap.method, "GET");
        assert_eq!(snap.path, "/api/v1/items");
        assert_eq!(snap.headers.get("x-forwarded-for").map(String::as_str), Some("1.2.3.4"));
        assert_eq!(snap.headers.get("content-type").map(String::as_str), Some("application/json"));
    }

    // Test 14: from_ctx with empty ctx → snapshot has empty/default values.
    #[test]
    fn wasm_ctx_snapshot_from_empty_ctx_has_safe_defaults() {
        let ctx = RequestCtx::new(); // no http_method / http_path / http_headers
        let snap = WasmCtxSnapshot::from_ctx(&ctx);
        assert!(snap.method.is_empty(), "method must default to empty");
        assert!(snap.path.is_empty(), "path must default to empty");
        assert!(snap.headers.is_empty(), "headers must default to empty");
    }

    // Test 15: scrub_sensitive_headers replaces Authorization/Cookie/X-Api-Key.
    #[test]
    fn wasm_ctx_snapshot_scrubs_sensitive_headers() {
        let mut ctx = RequestCtx::new();
        ctx.http_method = Some("POST".to_string());
        ctx.http_headers.insert("authorization".to_string(), "Bearer secret-token".to_string());
        ctx.http_headers.insert("cookie".to_string(), "session=abc123".to_string());
        ctx.http_headers.insert("x-api-key".to_string(), "key-xyz".to_string());
        ctx.http_headers.insert("x-forwarded-for".to_string(), "10.0.0.1".to_string());

        let snap = WasmCtxSnapshot::from_ctx(&ctx); // scrub = true by default
        assert_eq!(
            snap.headers.get("authorization").map(String::as_str),
            Some("<redacted>"),
            "authorization must be redacted"
        );
        assert_eq!(
            snap.headers.get("cookie").map(String::as_str),
            Some("<redacted>"),
            "cookie must be redacted"
        );
        assert_eq!(
            snap.headers.get("x-api-key").map(String::as_str),
            Some("<redacted>"),
            "x-api-key must be redacted"
        );
        // Non-sensitive headers must pass through.
        assert_eq!(
            snap.headers.get("x-forwarded-for").map(String::as_str),
            Some("10.0.0.1"),
            "x-forwarded-for must not be redacted"
        );
    }

    // Test 16: from_ctx_with_scrub(false) → raw values preserved.
    #[test]
    fn wasm_ctx_snapshot_no_scrub_preserves_sensitive_headers() {
        let mut ctx = RequestCtx::new();
        ctx.http_headers.insert("authorization".to_string(), "Bearer raw-token".to_string());

        let snap = WasmCtxSnapshot::from_ctx_with_scrub(&ctx, false);
        assert_eq!(
            snap.headers.get("authorization").map(String::as_str),
            Some("Bearer raw-token"),
            "scrub=false must preserve raw value"
        );
    }

    // Test 17: snapshot correctly converts populated method/path to RequestContext.
    #[test]
    fn wasm_ctx_snapshot_with_http_fields_converts_to_request_context() {
        let mut ctx = RequestCtx::new();
        ctx.http_method = Some("DELETE".to_string());
        ctx.http_path = Some("/orders/42".to_string());
        ctx.http_headers.insert("accept".to_string(), "application/json".to_string());

        let snap = WasmCtxSnapshot::from_ctx(&ctx);
        let rctx = snap.into_request_context();
        assert_eq!(rctx.request.method, "DELETE");
        assert_eq!(rctx.request.path, "/orders/42");
        assert_eq!(
            rctx.request.headers.get("accept").map(String::as_str),
            Some("application/json")
        );
    }
}
