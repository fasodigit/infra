// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Chaos tests for the shadow diff sink infrastructure.
//!
//! These tests simulate failure scenarios and verify that:
//! 1. The primary request path is never blocked.
//! 2. Metric counters are updated correctly.
//! 3. No panics occur under any failure condition.
//!
//! # Test matrix
//!
//! | # | Scenario | Verified |
//! |---|----------|---------|
//! | 1 | `pingora_worker_crash` — background task killed mid-sample | hyper path unaffected; counter tracked |
//! | 2 | `sink_outage_multi_sink` — Redpanda stub errors | SQLite still receives all 50 events |
//! | 3 | `channel_full_burst` — capacity=10, send 1000 | >=900 dropped; try_send never blocks |
//! | 4 | `slow_sink_backpressure` — SlowSink 10ms/insert, 1000 events | no panic; drops start at capacity |
//!
//! # Feature gate
//!
//! These tests require the `pingora` feature:
//! ```bash
//! cargo test -p armageddon-forge --features pingora --test shadow_chaos
//! ```

#![cfg(feature = "pingora")]

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use async_trait::async_trait;
use prometheus::Registry;
use tempfile::TempDir;
use tokio::time::sleep;

use armageddon_forge::pingora::shadow_sink::{
    DiffEventSender, MultiSink, ShadowDiffDispatcher, ShadowDiffEvent, ShadowDiffSink,
    SinkMetrics, SqliteSink,
};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn sample_event(req_id: &str) -> ShadowDiffEvent {
    ShadowDiffEvent {
        timestamp_unix_ms: ShadowDiffEvent::now_unix_ms(),
        request_id: req_id.to_string(),
        route: "/api/v1/poulets".to_string(),
        method: "GET".to_string(),
        hyper_status: 200,
        pingora_status: 500,
        hyper_body_hash: "abc123".to_string(),
        pingora_body_hash: "def456".to_string(),
        hyper_latency_ms: 10,
        pingora_latency_ms: 20,
        diverged_fields: vec!["status".to_string()],
        headers_diff: None,
        tenant_id: Some("tenant-bf".to_string()),
    }
}

/// A sink that records every call as an "error" (simulates Redpanda outage).
/// The emit itself does not fail (ShadowDiffSink::emit has no Result return);
/// it simulates a best-effort backend that logs errors but never panics.
#[derive(Debug)]
struct ErrorSink {
    error_count: Arc<AtomicU64>,
}

#[async_trait]
impl ShadowDiffSink for ErrorSink {
    async fn emit(&self, _e: &ShadowDiffEvent) {
        self.error_count.fetch_add(1, Ordering::SeqCst);
        tracing::warn!("ErrorSink: simulated backend failure");
    }
    fn backend_name(&self) -> &'static str {
        "redpanda"
    }
}

/// A sink that sleeps 10 ms per event (simulates slow I/O back-pressure).
#[derive(Debug)]
struct SlowSink {
    count: Arc<AtomicU64>,
}

#[async_trait]
impl ShadowDiffSink for SlowSink {
    async fn emit(&self, _e: &ShadowDiffEvent) {
        sleep(Duration::from_millis(10)).await;
        self.count.fetch_add(1, Ordering::SeqCst);
    }
    fn backend_name(&self) -> &'static str {
        "slow"
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: Pingora worker crash mid-sample
// ---------------------------------------------------------------------------

/// Scenario 1: Boot shadow mode with a background Pingora "worker" task.
/// Send 100 events. Kill the worker task mid-way by dropping its JoinHandle.
/// Assert: the primary (hyper) request path — represented here by the sender
/// path — continues without blocking or panic. The channel either fills and
/// drops are counted, or closes silently on send.
#[tokio::test]
async fn scenario1_pingora_worker_crash() {
    let r = Registry::new();
    let metrics = Arc::new(SinkMetrics::new(&r).unwrap());

    // Use a slow sink so the channel builds up backlog quickly.
    let slow_count = Arc::new(AtomicU64::new(0));
    let slow_sink = Arc::new(SlowSink {
        count: slow_count.clone(),
    });

    // Capacity = 20 — fills quickly at 10 ms/insert drain rate.
    let dispatcher = ShadowDiffDispatcher::start(slow_sink, 20, Some(metrics.clone()));
    let sender: DiffEventSender = dispatcher.sender();

    // Simulate the "pingora worker" as an independent tokio task that dies quickly.
    let worker_handle = tokio::spawn(async move {
        sleep(Duration::from_millis(5)).await;
        // Task exits, simulating worker crash.
    });

    // Send 100 events "from the primary hyper path" — must never block.
    let start = std::time::Instant::now();
    let mut sent = 0u64;
    let mut dropped = 0u64;
    for i in 0..100u64 {
        if sender.try_send(sample_event(&format!("req-{i}"))) {
            sent += 1;
        } else {
            dropped += 1;
        }
    }
    let elapsed = start.elapsed();

    // Drop the worker handle (simulating crash).
    drop(worker_handle);

    // Give the background drain task a moment to process what it can.
    sleep(Duration::from_millis(150)).await;

    // 1. The burst of 100 sends must complete near-instantly (non-blocking).
    assert!(
        elapsed < Duration::from_millis(500),
        "try_send burst must be non-blocking; took {:?}",
        elapsed
    );

    // 2. All send attempts are accounted for.
    assert_eq!(sent + dropped, 100, "all 100 send attempts must be accounted for");

    // 3. If events were dropped, the metric must reflect it.
    if dropped > 0 {
        let families = r.gather();
        let dropped_fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_sink_dropped_total");
        assert!(
            dropped_fam.is_some(),
            "dropped_total metric must exist when events are dropped"
        );
    }

    tracing::info!(
        sent,
        dropped,
        "scenario1: pingora worker crash — primary path unaffected"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: Shadow sink (Redpanda) outage — MultiSink fallback to SQLite
// ---------------------------------------------------------------------------

/// Scenario 2: Configure a MultiSink with an ErrorSink (simulating Redpanda
/// outage) and a real SqliteSink. Send 50 events. Assert:
/// - SqliteSink received all 50 events (MultiSink does not abort on first error).
/// - ErrorSink was called 50 times (all events tried both backends).
#[tokio::test]
async fn scenario2_sink_outage_multi_sink() {
    let tmp_dir = TempDir::new().expect("tmpdir");
    let db_path = tmp_dir.path().join("shadow_chaos2.db");

    let error_count = Arc::new(AtomicU64::new(0));
    let error_sink = Arc::new(ErrorSink {
        error_count: error_count.clone(),
    });
    let sqlite_sink = Arc::new(
        SqliteSink::open(db_path.to_str().unwrap(), 1000).expect("sqlite open"),
    );

    // MultiSink: ErrorSink (Redpanda stub) first, then SqliteSink.
    let multi = Arc::new(MultiSink::new(vec![
        error_sink as Arc<dyn ShadowDiffSink>,
        sqlite_sink.clone() as Arc<dyn ShadowDiffSink>,
    ]));

    let dispatcher = ShadowDiffDispatcher::start(multi, 200, None);
    let sender = dispatcher.sender();

    // Send 50 events.
    for i in 0..50u64 {
        sender.try_send(sample_event(&format!("chaos2-req-{i}")));
    }

    // Give the drain task time to process all 50 events (NoopSink-speed sink).
    sleep(Duration::from_millis(200)).await;

    // ErrorSink must have been called 50 times.
    assert_eq!(
        error_count.load(Ordering::SeqCst),
        50,
        "ErrorSink must be called for all 50 events even when it returns an error"
    );

    // SqliteSink must have received all 50 events.
    let row_count = sqlite_sink.row_count().await.expect("row count query");
    assert_eq!(
        row_count, 50,
        "SqliteSink must receive all 50 events despite Redpanda outage; got {}",
        row_count
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Channel full burst — capacity=10, 1000 events
// ---------------------------------------------------------------------------

/// Scenario 3: Configure dispatcher with capacity=10. Send 1000 events in a
/// burst. Assert:
/// - The sender NEVER blocks (try_send is always immediate).
/// - `dropped_total{reason="channel_full"}` >= 900.
/// - No panic occurs.
#[tokio::test]
async fn scenario3_channel_full_burst() {
    let r = Registry::new();
    let metrics = Arc::new(SinkMetrics::new(&r).unwrap());

    // Use a slow sink so the channel fills quickly.
    let slow_count = Arc::new(AtomicU64::new(0));
    let slow_sink = Arc::new(SlowSink {
        count: slow_count.clone(),
    });

    // Tiny capacity = 10.
    let dispatcher = ShadowDiffDispatcher::start(slow_sink, 10, Some(metrics.clone()));
    let sender = dispatcher.sender();

    // Send 1000 events — must complete instantly (non-blocking).
    let start = std::time::Instant::now();
    let mut queued = 0u64;
    let mut dropped = 0u64;
    for i in 0..1000u64 {
        if sender.try_send(sample_event(&format!("burst-{i}"))) {
            queued += 1;
        } else {
            dropped += 1;
        }
    }
    let elapsed = start.elapsed();

    // The burst of 1000 try_send calls must complete in < 500 ms (non-blocking).
    assert!(
        elapsed < Duration::from_millis(500),
        "try_send burst must be non-blocking; took {:?}",
        elapsed
    );
    assert_eq!(queued + dropped, 1000, "all send attempts accounted for");

    // With capacity=10 and a slow drain, at least 900 must have been dropped.
    assert!(
        dropped >= 900,
        "with capacity=10 and 1000 sends, at least 900 must be dropped; got dropped={}",
        dropped
    );

    // Verify the dropped metric was incremented with reason=channel_full.
    let families = r.gather();
    let dropped_fam = families
        .iter()
        .find(|f| f.get_name() == "armageddon_shadow_sink_dropped_total")
        .expect("dropped_total metric must be registered");

    let total_channel_full: f64 = dropped_fam
        .get_metric()
        .iter()
        .flat_map(|m| {
            let labels = m.get_label();
            if labels.iter().any(|l| l.get_value() == "channel_full") {
                Some(m.get_counter().get_value())
            } else {
                None
            }
        })
        .sum();

    assert!(
        total_channel_full >= 900.0,
        "dropped_total{{reason=channel_full}} must be >= 900; got {}",
        total_channel_full
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: Slow sink back-pressure
// ---------------------------------------------------------------------------

/// Scenario 4: SlowSink with artificial 10 ms per-insert delay.
/// Send 1000 events in a burst. Assert:
/// - No panic occurs (test reaching end == success).
/// - Channel fills and drops accumulate at the capacity boundary.
/// - The drain task continues to make progress (processed > 0 after wait).
#[tokio::test]
async fn scenario4_slow_sink_backpressure() {
    let r = Registry::new();
    let metrics = Arc::new(SinkMetrics::new(&r).unwrap());

    let count = Arc::new(AtomicU64::new(0));
    let slow_sink = Arc::new(SlowSink {
        count: count.clone(),
    });

    // Capacity = 50 — will fill quickly given 10 ms/insert drain speed.
    let dispatcher = ShadowDiffDispatcher::start(slow_sink, 50, Some(metrics.clone()));
    let sender = dispatcher.sender();

    // Send 1000 events fire-and-forget.
    let mut dropped = 0u64;
    for i in 0..1000u64 {
        if !sender.try_send(sample_event(&format!("slow-{i}"))) {
            dropped += 1;
        }
    }

    // Wait 300 ms — at 10 ms/insert the drain task should process ~30 events.
    sleep(Duration::from_millis(300)).await;

    let processed = count.load(Ordering::SeqCst);

    // The drain task must be making progress.
    assert!(
        processed > 0,
        "drain task must have processed at least 1 event; processed={}",
        processed
    );

    // Most events must have been dropped due to slow drain + bounded channel.
    assert!(
        dropped > 500,
        "with slow sink and 1000 burst events, more than 500 must be dropped; got dropped={}",
        dropped
    );

    // Verify dropped metric was incremented.
    let families = r.gather();
    let dropped_fam = families
        .iter()
        .find(|f| f.get_name() == "armageddon_shadow_sink_dropped_total")
        .expect("dropped_total metric must be registered");

    let total_metric_dropped: f64 = dropped_fam
        .get_metric()
        .iter()
        .map(|m| m.get_counter().get_value())
        .sum();

    assert!(
        total_metric_dropped > 0.0,
        "dropped_total must be > 0 after slow-sink burst; got {}",
        total_metric_dropped
    );
}
