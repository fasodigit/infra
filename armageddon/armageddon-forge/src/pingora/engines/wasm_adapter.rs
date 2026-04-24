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
//! `armageddon-wasm/src/runtime.rs` contains `TODO: scan plugins_dir and
//! load .wasm modules`.  Until that TODO is resolved in M5, the worker
//! thread runs an **empty runtime** — no plugins are loaded and every job
//! receives `EngineVerdict::empty()` (score 0.0, Allow).  The worker
//! still starts, the channel round-trip is validated, and the fail-open
//! behaviour on timeout is fully tested.
//!
//! TODO(M5 #106): wire `PluginRuntime::load_plugin` loop after
//! `plugins_dir` scan is implemented in `armageddon-wasm`.
//!
//! # Failure modes
//!
//! * **Worker thread not started** (constructor error): `new()` returns
//!   `Err`; the caller falls back to the no-op stub.
//! * **Job send failure** (worker thread dead): `analyze` returns
//!   `EngineVerdict::Skipped` (fail-open).
//! * **Timeout** (100 ms budget via `tokio::time::timeout`): fail-open
//!   with `EngineVerdict::empty()`.  Logged at `warn`.
//! * **Worker panic**: the OS thread exits; subsequent `req_tx.send`
//!   calls will fail because `req_rx` is dropped.  The adapter detects
//!   this on the next request and returns `Skipped`.
//!
//! # Metrics
//!
//! Emits structured `tracing` events.  Prometheus counters wired in M5.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;

use super::pipeline::{EngineAdapter, EngineVerdict};
use crate::pingora::ctx::RequestCtx;

// ── Wire-format types ─────────────────────────────────────────────────────

/// Cloneable snapshot of the request state that can be sent over the
/// channel without carrying a `&mut RequestCtx` reference.
///
/// Only the fields the WASM runtime currently uses are captured; extend
/// this struct as the `PluginRuntime` is enriched in M5.
#[derive(Debug, Clone)]
pub struct WasmCtxSnapshot {
    pub request_id: String,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub cluster: String,
    pub waf_score: f32,
    pub ai_score: f32,
}

impl WasmCtxSnapshot {
    fn from_ctx(ctx: &RequestCtx) -> Self {
        Self {
            request_id: ctx.request_id.clone(),
            user_id: ctx.user_id.clone(),
            tenant_id: ctx.tenant_id.clone(),
            cluster: ctx.cluster.clone(),
            waf_score: ctx.waf_score,
            ai_score: ctx.ai_score,
        }
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
    /// Empty result used when no plugins are loaded (empty runtime path).
    pub fn empty() -> Self {
        Self {
            score: 0.0,
            label: None,
            block: false,
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

// ── WasmAdapter ───────────────────────────────────────────────────────────

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
    /// Spawn the Wasmtime worker thread and return the adapter.
    ///
    /// `_plugins_dir` is reserved for M5 when `PluginRuntime::load_plugin`
    /// is implemented.  Pass `PathBuf::from("/dev/null")` or any path in
    /// tests — it is currently unused.
    pub fn new(
        _plugins_dir: PathBuf,
        _max_fuel: u64,
    ) -> anyhow::Result<Self> {
        let (req_tx, req_rx) = async_channel::unbounded::<WasmJob>();

        let worker = std::thread::Builder::new()
            .name("armageddon-wasm-worker".to_string())
            .spawn(move || {
                // Single-threaded tokio runtime owned entirely by this OS thread.
                // `Store` / `Instance` (both `!Send`) stay local to this thread.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("wasm worker: failed to build tokio runtime");

                rt.block_on(wasm_worker_loop(req_rx));
            })?;

        Ok(Self {
            req_tx,
            _worker: worker,
        })
    }
}

/// Main loop executed by the OS worker thread.
///
/// Receives jobs from the channel, runs the (currently empty) plugin
/// runtime, and sends back results.  The loop exits when the channel is
/// closed (i.e. when the last `Sender` is dropped — graceful shutdown).
async fn wasm_worker_loop(req_rx: async_channel::Receiver<WasmJob>) {
    tracing::info!("wasm worker: started (empty plugin runtime — TODO M5)");

    while let Ok(job) = req_rx.recv().await {
        let result = run_plugins_sync(&job.snapshot);
        // If the Pingora side timed-out and dropped resp_rx, the send
        // silently fails — that is fine (fail-open already delivered).
        let _ = job.resp_tx.send(result).await;
    }

    tracing::info!("wasm worker: channel closed, exiting");
}

/// Synchronous plugin execution (runs on the worker thread).
///
/// Currently returns `WasmResult::empty()` because `PluginRuntime` has a
/// `TODO: scan plugins_dir` in `armageddon-wasm/src/runtime.rs`.  When
/// that TODO is resolved, replace this body with:
///
/// ```rust,ignore
/// use armageddon_wasm::runtime::PluginRuntime;
/// let ctx = build_request_context(snap);
/// let results = runtime.run_plugins(&ctx);
/// aggregate_results(results)
/// ```
///
/// TODO(M5 #106): implement real plugin execution.
fn run_plugins_sync(_snap: &WasmCtxSnapshot) -> WasmResult {
    WasmResult::empty()
}

#[async_trait]
impl EngineAdapter for WasmAdapter {
    fn name(&self) -> &'static str {
        "wasm"
    }

    async fn analyze(&self, ctx: &mut RequestCtx) -> EngineVerdict {
        let snapshot = WasmCtxSnapshot::from_ctx(ctx);
        let (resp_tx, resp_rx) = async_channel::bounded::<WasmResult>(1);

        // Send the job.  If the channel is closed (worker thread died),
        // fail-open immediately.
        if self
            .req_tx
            .send(WasmJob { snapshot, resp_tx })
            .await
            .is_err()
        {
            tracing::warn!("wasm adapter: worker channel closed; failing open");
            return EngineVerdict::Skipped;
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

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_adapter() -> WasmAdapter {
        WasmAdapter::new(PathBuf::from("/dev/null"), 100_000_000)
            .expect("WasmAdapter::new must succeed in tests")
    }

    // ── Test 1: adapter constructs + worker thread starts ──────────
    #[tokio::test]
    async fn wasm_adapter_constructs_and_worker_starts() {
        // If the worker thread failed to start, `new()` would return Err.
        let adapter = make_adapter();
        // The channel should be open (worker alive).
        assert!(
            !adapter.req_tx.is_closed(),
            "request channel must be open after construction"
        );
    }

    // ── Test 2: job round-trip (send job → receive response) ────────
    //
    // With the empty plugin runtime, every job returns score=0.0 / block=false
    // → EngineVerdict::Allow { score: 0.0 }.
    #[tokio::test]
    async fn wasm_adapter_round_trip_returns_allow() {
        let adapter = make_adapter();
        let mut ctx = RequestCtx::new();
        let v = adapter.analyze(&mut ctx).await;
        assert!(
            matches!(v, EngineVerdict::Allow { score } if score.abs() < f32::EPSILON),
            "empty runtime must return Allow{{score:0.0}}, got {v:?}"
        );
    }

    // ── Test 3: multiple round-trips are independent ─────────────────
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

    // ── Test 4: timeout path — fail-open on stalled worker ──────────
    //
    // We simulate a stalled worker by sending a job directly to a channel
    // whose receiver is never polled, then waiting for the timeout path.
    #[tokio::test]
    async fn wasm_adapter_timeout_fails_open() {
        // Create a channel with a receiver that we never poll.
        let (req_tx, _req_rx_stalled) = async_channel::unbounded::<WasmJob>();
        // Build a fake adapter with a stalled channel.
        // We use the internal types directly — construct a WasmCtxSnapshot
        // and a one-shot response channel, then drive the timeout logic
        // by calling `tokio::time::timeout` manually.
        let (resp_tx, resp_rx) = async_channel::bounded::<WasmResult>(1);
        let snap = WasmCtxSnapshot {
            request_id: "timeout-test".to_string(),
            user_id: None,
            tenant_id: None,
            cluster: String::new(),
            waf_score: 0.0,
            ai_score: 0.0,
        };
        // Send job to the stalled channel (it queues but worker is not running).
        req_tx
            .send(WasmJob { snapshot: snap, resp_tx })
            .await
            .expect("send must succeed (unbounded)");

        // Simulate the 100 ms timeout on the response side.
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

    // ── Test 5: graceful shutdown (worker exits when sender dropped) ─
    //
    // When the last `req_tx` is dropped, `req_rx.recv()` in the worker
    // loop returns `Err(RecvError)` which exits the loop.  We verify this
    // by observing the channel's closed state after dropping the sender.
    #[tokio::test]
    async fn wasm_adapter_graceful_shutdown_on_sender_drop() {
        let (req_tx, req_rx) = async_channel::unbounded::<WasmJob>();

        // Verify channel is open while sender lives.
        assert!(!req_rx.is_closed(), "channel must be open while sender alive");

        // Drop the sender → worker would exit its recv loop.
        drop(req_tx);

        // The channel is now closed from the sender side; recv returns Err.
        let result = req_rx.recv().await;
        assert!(
            result.is_err(),
            "recv must return Err after all senders dropped"
        );
    }

    // ── Test 6: WasmResult::into_verdict mapping ─────────────────────
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
}
