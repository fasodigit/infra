// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Debounced batch collector for xDS resource updates.
//!
//! When the xds-controller pushes many resource updates in rapid succession
//! (common during a rolling deploy or a large topology change), applying each
//! update individually causes unnecessary config churn — N ArcSwap stores, N
//! downstream table rebuilds — for work that can be coalesced into one atomic
//! batch.
//!
//! `Debouncer<T>` collects items pushed within a configurable silence `window`
//! and delivers them as a single `Vec<T>` batch to a registered callback once
//! the window elapses with no new items.  Setting `window = Duration::ZERO`
//! disables debouncing: every item is delivered immediately as a singleton
//! batch (useful for tests and low-frequency paths).
//!
//! # Ordering
//!
//! Items within a batch are delivered in push order (FIFO bounded channel).
//! Batch boundaries are determined solely by inter-push timing.
//!
//! # Failure modes
//!
//! * **Shutdown with pending items**: [`Debouncer::shutdown`] sends an explicit
//!   `Shutdown` signal.  The flush task drains the collector before exiting so
//!   no updates are silently dropped on graceful shutdown.
//!
//! * **Backpressure**: the internal channel capacity is 1024.  If the producer
//!   pushes faster than the flush task can drain, [`Debouncer::push`] returns
//!   `Err(DebouncerError::ChannelClosed)` rather than blocking indefinitely.
//!
//! # Metrics
//!
//! * `xds_debounce_batches_total`     — counter, one per flush.
//! * `xds_debounce_items_per_batch`   — histogram, batch length.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};
use tokio::time::Instant;
use tracing::debug;

use crate::metrics;

// ---------------------------------------------------------------------------
// Internal channel message
// ---------------------------------------------------------------------------

enum Msg<T> {
    Item(T),
    Shutdown,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Debounced batch collector — generic over the item type `T`.
///
/// Construct with [`Debouncer::new`], push items with [`push`](Self::push),
/// and call [`shutdown`](Self::shutdown) for a guaranteed final flush.
pub struct Debouncer<T: Send + 'static> {
    /// Silence interval.  Zero → passthrough mode (no batching).
    window: Duration,
    /// Send-side of the bounded internal channel.
    tx: mpsc::Sender<Msg<T>>,
    /// Shared collector so callers can inspect pending-item depth.
    pub collector: Arc<Mutex<Vec<T>>>,
}

impl<T: Send + 'static> Debouncer<T> {
    /// Create a new `Debouncer` with the given silence `window`.
    ///
    /// * `window` — how long to wait after the last push before flushing.
    ///   Pass [`Duration::ZERO`] to disable debouncing (passthrough).
    /// * `on_batch` — synchronous callback invoked with the owned batch;
    ///   runs inside the internal tokio task.
    ///
    /// The internal flush task is spawned immediately on the current tokio
    /// runtime and runs until [`shutdown`](Self::shutdown) is called or the
    /// `Debouncer` is dropped.
    pub fn new<F>(window: Duration, on_batch: Arc<F>) -> Self
    where
        F: Fn(Vec<T>) + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::channel::<Msg<T>>(1024);
        let collector = Arc::new(Mutex::new(Vec::<T>::new()));
        let collector_clone = collector.clone();

        tokio::spawn(flush_task(rx, collector_clone, window, on_batch));

        Self { window, tx, collector }
    }

    /// Push one item into the debounce window.
    ///
    /// # Errors
    ///
    /// Returns [`DebouncerError::ChannelClosed`] if the internal channel is
    /// full or the flush task has exited.
    pub async fn push(&self, item: T) -> Result<(), DebouncerError> {
        self.tx
            .send(Msg::Item(item))
            .await
            .map_err(|_| DebouncerError::ChannelClosed)
    }

    /// Flush any pending items synchronously and stop the internal task.
    ///
    /// After this returns every item pushed before the call has been delivered
    /// to the callback.
    pub async fn shutdown(self) {
        // Send shutdown signal; ignore error if task already gone.
        let _ = self.tx.send(Msg::Shutdown).await;
        // Drop our sender so the task sees the channel closed if it somehow
        // missed the explicit Shutdown message.
        drop(self.tx);
        // Yield to give the spawned task an opportunity to run its final flush.
        tokio::task::yield_now().await;
    }

    /// Returns `true` when debouncing is disabled (`window == Duration::ZERO`).
    #[inline]
    pub fn is_passthrough(&self) -> bool {
        self.window.is_zero()
    }
}

// ---------------------------------------------------------------------------
// Flush task
// ---------------------------------------------------------------------------

/// Background task driving the debounce state machine.
///
/// State transitions:
///
/// ```text
/// IDLE ──push──► COLLECTING ──deadline──► FLUSH ──► IDLE
///                     │                               ▲
///                     └──push (reset deadline)────────┘
///                     │
///                     └──Shutdown ──► FLUSH ──► EXIT
/// ```
async fn flush_task<T, F>(
    mut rx: mpsc::Receiver<Msg<T>>,
    collector: Arc<Mutex<Vec<T>>>,
    window: Duration,
    on_batch: Arc<F>,
) where
    T: Send + 'static,
    F: Fn(Vec<T>) + Send + Sync + 'static,
{
    let passthrough = window.is_zero();

    // Sentinel: "no active deadline" — set to a far-future instant.
    // We recompute it rather than using an Option to avoid extra branches.
    let sentinel = || Instant::now() + Duration::from_secs(3_600 * 24 * 365);
    let mut deadline = sentinel();
    let mut has_pending = false;

    loop {
        if passthrough || !has_pending {
            // No active window: block on next message.
            match rx.recv().await {
                None => {
                    // Sender dropped unexpectedly — flush remainder and exit.
                    drain_and_call(&collector, &*on_batch).await;
                    return;
                }
                Some(Msg::Shutdown) => {
                    drain_and_call(&collector, &*on_batch).await;
                    return;
                }
                Some(Msg::Item(item)) => {
                    if passthrough {
                        emit_batch(vec![item], &*on_batch);
                    } else {
                        collector.lock().await.push(item);
                        deadline = Instant::now() + window;
                        has_pending = true;
                        debug!(
                            window_ms = window.as_millis(),
                            "debouncer: first item in window, deadline set"
                        );
                    }
                }
            }
        } else {
            // Active window: race message against deadline.
            tokio::select! {
                biased;

                msg = rx.recv() => {
                    match msg {
                        None => {
                            // Sender dropped — flush and exit.
                            drain_and_call(&collector, &*on_batch).await;
                            return;
                        }
                        Some(Msg::Shutdown) => {
                            drain_and_call(&collector, &*on_batch).await;
                            return;
                        }
                        Some(Msg::Item(item)) => {
                            // Append and reset the silence window.
                            collector.lock().await.push(item);
                            deadline = Instant::now() + window;
                            debug!(
                                window_ms = window.as_millis(),
                                "debouncer: item added, deadline reset"
                            );
                        }
                    }
                }

                _ = tokio::time::sleep_until(deadline) => {
                    // Silence window expired — flush.
                    drain_and_call(&collector, &*on_batch).await;
                    deadline = sentinel();
                    has_pending = false;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Drain the collector and invoke the callback with the owned batch.
async fn drain_and_call<T, F>(collector: &Mutex<Vec<T>>, on_batch: &F)
where
    T: Send + 'static,
    F: Fn(Vec<T>),
{
    let batch: Vec<T> = {
        let mut guard = collector.lock().await;
        if guard.is_empty() {
            return;
        }
        std::mem::take(&mut *guard)
    };
    emit_batch(batch, on_batch);
}

/// Record metrics and call the callback with an owned batch.
#[inline]
fn emit_batch<T, F>(batch: Vec<T>, on_batch: &F)
where
    F: Fn(Vec<T>),
{
    let n = batch.len();
    metrics::inc_batches();
    metrics::observe_batch_size(n);
    debug!(batch_size = n, "debouncer: flushing batch");
    on_batch(batch);
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from [`Debouncer::push`].
#[derive(Debug, thiserror::Error)]
pub enum DebouncerError {
    #[error("debouncer channel closed — shutdown was called or task panicked")]
    ChannelClosed,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};

    // ------------------------------------------------------------------
    // Test 1 — 50 updates within 20 ms → 1 batch of 50 items
    // ------------------------------------------------------------------
    /// Push 50 items in rapid succession (total ≪ 20 ms).  The 100 ms window
    /// has not expired yet, so all items must be coalesced into a single batch
    /// of exactly 50 items, delivered in push order.
    #[tokio::test]
    async fn test_50_updates_in_20ms_yields_one_batch() {
        // Use std::sync::Mutex so the closure is synchronous — avoids the
        // "Cannot start a runtime from within a runtime" panic that would occur
        // with tokio::sync::Mutex + block_on inside an async context.
        let batches: Arc<StdMutex<Vec<Vec<u32>>>> = Arc::new(StdMutex::new(vec![]));
        let batches_cb = batches.clone();

        let debouncer: Debouncer<u32> = Debouncer::new(
            Duration::from_millis(100),
            Arc::new(move |batch: Vec<u32>| {
                batches_cb.lock().unwrap().push(batch);
            }),
        );

        for i in 0u32..50 {
            debouncer.push(i).await.expect("push must succeed");
        }

        // Wait for window (100 ms) + generous margin.
        tokio::time::sleep(Duration::from_millis(300)).await;

        let collected = batches.lock().unwrap();
        assert_eq!(
            collected.len(),
            1,
            "expected exactly 1 batch, got {}",
            collected.len()
        );
        assert_eq!(
            collected[0].len(),
            50,
            "batch must contain all 50 items, got {}",
            collected[0].len()
        );
        // Order within batch must be preserved.
        for (idx, &val) in collected[0].iter().enumerate() {
            assert_eq!(
                val, idx as u32,
                "order violated at position {idx}: expected {idx}, got {val}"
            );
        }
    }

    // ------------------------------------------------------------------
    // Test 2 — 10 updates spaced 200 ms apart → 10 individual callbacks
    // ------------------------------------------------------------------
    /// Each push is separated by 200 ms — twice the 100 ms window.  Every item
    /// causes its own batch flush before the next arrives → 10 callbacks.
    #[tokio::test]
    async fn test_10_updates_spaced_200ms_yields_10_callbacks() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let item_total = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let it = item_total.clone();

        let debouncer: Debouncer<u32> = Debouncer::new(
            Duration::from_millis(100),
            Arc::new(move |batch: Vec<u32>| {
                cc.fetch_add(1, Ordering::SeqCst);
                it.fetch_add(batch.len(), Ordering::SeqCst);
            }),
        );

        for i in 0u32..10 {
            debouncer.push(i).await.expect("push ok");
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // Allow the final window to expire.
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            10,
            "expected 10 individual callbacks"
        );
        assert_eq!(
            item_total.load(Ordering::SeqCst),
            10,
            "total items across all callbacks must equal 10"
        );
    }

    // ------------------------------------------------------------------
    // Test 3 — shutdown flushes pending items before window expires
    // ------------------------------------------------------------------
    /// Push 5 items, then immediately call `shutdown` without waiting for the
    /// 100 ms window.  All 5 items must be delivered exactly once.
    #[tokio::test]
    async fn test_shutdown_flushes_pending_items() {
        let flushed: Arc<StdMutex<Vec<u32>>> = Arc::new(StdMutex::new(vec![]));
        let flushed_cb = flushed.clone();

        let debouncer: Debouncer<u32> = Debouncer::new(
            Duration::from_millis(100),
            Arc::new(move |batch: Vec<u32>| {
                flushed_cb.lock().unwrap().extend(batch);
            }),
        );

        for i in 0u32..5 {
            debouncer.push(i).await.expect("push ok");
        }

        // Shutdown immediately — before the 100 ms window expires.
        debouncer.shutdown().await;

        // Allow yield to propagate the final flush.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let items = flushed.lock().unwrap();
        assert_eq!(
            items.len(),
            5,
            "shutdown must flush all pending items; got {}",
            items.len()
        );
    }

    // ------------------------------------------------------------------
    // Test 4 — zero-window passthrough
    // ------------------------------------------------------------------
    /// `window = Duration::ZERO` bypasses batching: each push triggers an
    /// immediate singleton callback.
    #[tokio::test]
    async fn test_zero_window_passthrough() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let debouncer: Debouncer<u32> = Debouncer::new(
            Duration::ZERO,
            Arc::new(move |batch: Vec<u32>| {
                assert_eq!(
                    batch.len(),
                    1,
                    "passthrough must deliver single-item batches"
                );
                cc.fetch_add(1, Ordering::SeqCst);
            }),
        );

        assert!(debouncer.is_passthrough(), "window=0 must be passthrough");

        for i in 0u32..8 {
            debouncer.push(i).await.expect("push ok");
        }

        tokio::time::sleep(Duration::from_millis(80)).await;

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            8,
            "expected 8 singleton callbacks in passthrough mode"
        );
    }

    // ------------------------------------------------------------------
    // Test 5 — concurrent pushes are thread-safe
    // ------------------------------------------------------------------
    /// 10 tasks each push 5 items concurrently (50 total).  All items must
    /// be delivered with correct total count.
    #[tokio::test]
    async fn test_concurrent_pushes_total_count() {
        let total = Arc::new(AtomicUsize::new(0));
        let total_cb = total.clone();

        let debouncer = Arc::new(Debouncer::<u32>::new(
            Duration::from_millis(100),
            Arc::new(move |batch: Vec<u32>| {
                total_cb.fetch_add(batch.len(), Ordering::SeqCst);
            }),
        ));

        let mut handles = Vec::new();
        for task in 0u32..10 {
            let d = debouncer.clone();
            handles.push(tokio::spawn(async move {
                for item in (task * 5)..(task * 5 + 5) {
                    d.push(item).await.expect("push ok");
                }
            }));
        }

        for h in handles {
            h.await.expect("task must not panic");
        }

        // Wait for window to expire.
        tokio::time::sleep(Duration::from_millis(350)).await;

        assert_eq!(
            total.load(Ordering::SeqCst),
            50,
            "all 50 items from concurrent pushes must be delivered"
        );
    }
}
