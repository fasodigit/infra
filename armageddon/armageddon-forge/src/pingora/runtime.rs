// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runtime bridge — exposes a dedicated multi-threaded tokio runtime to
//! Pingora-driven code paths.
//!
//! See `RUNTIME.md` in this directory for the full design rationale
//! (Option A: Pingora main process + isolated tokio runtime on a background
//! OS thread).
//!
//! ## Why a bridge exists
//!
//! Pingora ships with its own I/O scheduler (`pingora-runtime` /
//! `monoio`-flavoured) which does **not** drive tokio's reactor.  Every
//! FASO security engine, the xDS control-plane client, KAYA's RESP3
//! client and the SPIFFE certificate fetcher are tokio-native.  Running
//! them inline on a Pingora worker would deadlock (`!Send` ctxs, unknown
//! reactors, or missing `tokio::task_local!`).
//!
//! The bridge therefore spawns a separate multi-threaded tokio runtime
//! on a dedicated OS thread at first call to [`tokio_handle`] and exposes
//! a [`tokio::runtime::Handle`] that can be shipped to any Pingora hook.
//!
//! ## Usage from inside a `ProxyHttp` async method
//!
//! ```ignore
//! // INSIDE e.g. `request_filter`:
//! let handle = crate::pingora::runtime::tokio_handle();
//! let (tx, rx) = tokio::sync::oneshot::channel();
//! handle.spawn(async move {
//!     // use tokio-native code freely here
//!     let score = armageddon_sentinel::score_request(/* … */).await;
//!     let _ = tx.send(score);
//! });
//! // Await the oneshot on the **Pingora** scheduler — DO NOT block_on():
//! let score = rx.await.unwrap_or_default();
//! ```
//!
//! ## Forbidden patterns
//!
//! - **Never** call `tokio_handle().block_on(...)` from inside a Pingora
//!   async hook.  The hook is driven by Pingora's scheduler; a nested
//!   `block_on` from a different runtime can deadlock.
//! - **Never** hold a reference to a tokio-runtime-owned future across a
//!   Pingora `.await` point unless it was spawned via `handle.spawn` and
//!   you are awaiting a `JoinHandle` (`Send`/`'static`).

use std::sync::OnceLock;
use tokio::runtime::{Builder, Handle, Runtime};

/// The global singleton runtime handle.
static BRIDGE: OnceLock<Handle> = OnceLock::new();

/// Number of worker threads the bridge runtime spawns.
///
/// Kept modest (4) because tokio-native security-engine work is
/// short-lived and CPU-cheap; the heavy-lifting is I/O and therefore
/// scales via Pingora's own worker pool.  Can be tuned by the operator
/// through the `ARMAGEDDON_FORGE_BRIDGE_THREADS` env var.
fn bridge_worker_threads() -> usize {
    std::env::var("ARMAGEDDON_FORGE_BRIDGE_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(4)
}

/// Obtain a [`Handle`] to the bridge tokio runtime.
///
/// Lazy-initialised on first call.  The runtime is **not** dropped for the
/// lifetime of the process — a dedicated OS thread parks inside
/// `rt.block_on(std::future::pending())` to keep it alive while allowing
/// `Handle` clones to outlive individual `Runtime` scopes.
pub fn tokio_handle() -> Handle {
    BRIDGE
        .get_or_init(|| {
            let threads = bridge_worker_threads();
            // Build the runtime on the dedicated thread so the worker
            // threads inherit clean TLS (Pingora's TLS has OpenSSL pools
            // we don't want to race with).
            let (tx, rx) = std::sync::mpsc::channel::<Handle>();

            std::thread::Builder::new()
                .name("armageddon-forge-bridge".into())
                .spawn(move || {
                    let rt: Runtime = Builder::new_multi_thread()
                        .worker_threads(threads)
                        .thread_name("forge-bridge-worker")
                        .enable_all()
                        .build()
                        .expect("failed to build forge bridge tokio runtime");
                    let handle = rt.handle().clone();
                    tx.send(handle).expect("bridge handshake failed");
                    // Park: keeps the runtime alive forever.
                    rt.block_on(std::future::pending::<()>());
                })
                .expect("failed to spawn forge bridge thread");

            rx.recv().expect("bridge handshake failed: no handle")
        })
        .clone()
    }

/// Test-only: sanity-check that the bridge can run a future.
#[doc(hidden)]
#[cfg(test)]
pub fn __test_ping() -> u32 {
    let handle = tokio_handle();
    let (tx, rx) = std::sync::mpsc::channel();
    handle.spawn(async move {
        let _ = tx.send(42u32);
    });
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .expect("bridge ping timed out")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_worker_threads_defaults_to_four() {
        // Clean env, ensure default is 4.
        std::env::remove_var("ARMAGEDDON_FORGE_BRIDGE_THREADS");
        assert_eq!(bridge_worker_threads(), 4);
    }

    #[test]
    fn bridge_is_reachable_and_handles_spawned_work() {
        let n = __test_ping();
        assert_eq!(n, 42);
    }

    #[test]
    fn bridge_handle_is_cloneable_across_calls() {
        let h1 = tokio_handle();
        let h2 = tokio_handle();
        // Both handles point at the same runtime.
        let _ = h1.clone();
        let _ = h2.clone();
    }
}
