// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Integration tests for the canary progressive rollout state machine.
//!
//! # Coverage
//!
//! 1. `test_happy_path_advance` — mock Prometheus returns healthy SLO on every
//!    tick; canary auto-advances 1 % → 10 % → 50 % → Promoted(100 %).
//!
//! 2. `test_rollback_on_slo_breach` — mock Prometheus returns error_rate above
//!    threshold for 3 consecutive ticks; canary auto-rolls back.
//!
//! 3. `test_manual_abort` — `AbortCanary` RPC sets weight to 0 % immediately.
//!
//! 4. `test_pause_resume` — `PauseCanary` halts tick advancement; route weights
//!    unchanged; `PromoteCanary` force-promotes from paused state.
//!
//! # Architecture notes
//!
//! The tests use a mock HTTP server (via `tokio::net::TcpListener` + a minimal
//! HTTP responder) to serve static Prometheus JSON responses.  No real Prometheus
//! is required.  The `CanaryOrchestrator` is exercised directly without the
//! tonic gRPC layer so that tests stay fast and do not require a running server.

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use xds_server::canary::{CanaryOrchestrator, SloConfig, Stage};
use xds_store::ConfigStore;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Start a mock Prometheus HTTP server on a random port.
///
/// `error_rate_factory` is called for each `/api/v1/query` request and returns
/// `(error_rate, latency_p99_ms)` for that request.
///
/// The function returns the bound port.
async fn start_mock_prometheus<F>(factory: F) -> u16
where
    F: Fn() -> (f64, f64) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let factory = Arc::new(factory);

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let factory = factory.clone();
            tokio::spawn(async move {
                // Read request (we don't parse it; just drain it).
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;

                let (error_rate, latency_p99_ms) = factory();

                // Check the query param to determine which metric to return.
                let req_str = String::from_utf8_lossy(&buf);
                let value = if req_str.contains("duration_seconds") {
                    latency_p99_ms
                } else {
                    error_rate
                };

                let body = format!(
                    r#"{{"status":"success","data":{{"resultType":"vector","result":[{{"metric":{{}},"value":[1700000000,"{value}"]}}]}}}}"#
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });

    port
}

/// Build a `SloConfig` pointing at the mock Prometheus.
fn slo_for_port(port: u16) -> SloConfig {
    SloConfig {
        error_rate_max: 0.01,        // 1 %
        latency_p99_max_ms: 100.0,   // 100 ms
        prometheus_endpoint: format!("http://127.0.0.1:{port}"),
    }
}

/// Create a fresh `ConfigStore` with `<service>-stable` cluster pre-populated.
fn make_store(service: &str) -> ConfigStore {
    use chrono::Utc;
    use std::collections::HashMap;
    use xds_store::model::{ClusterEntry, DiscoveryType, LbPolicy};

    let store = ConfigStore::new();
    let _ = store.set_cluster(ClusterEntry {
        name: format!("{service}-stable"),
        discovery_type: DiscoveryType::Eds,
        lb_policy: LbPolicy::RoundRobin,
        connect_timeout_ms: 5000,
        health_check: None,
        circuit_breaker: None,
        spiffe_id: None,
        metadata: HashMap::new(),
        updated_at: Utc::now(),
    });
    store
}

// ---------------------------------------------------------------------------
// Test 1: happy path — canary auto-advances through all stages
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_happy_path_advance() {
    // Mock Prometheus always returns healthy SLO.
    let port = start_mock_prometheus(|| (0.001, 20.0)).await; // err=0.1%, p99=20ms
    let store = make_store("poulets-api");
    let orchestrator = CanaryOrchestrator::new(store.clone());

    // Use very short stage duration (0 s) so tests don't need to wait 1 h.
    let canary_id = orchestrator.start(
        "poulets-api",
        "v2.0.0",
        slo_for_port(port),
        Duration::from_secs(0), // advance immediately
    );

    // Initial stage must be Stage1Pct.
    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status.current_stage, Stage::Stage1Pct);
    assert_eq!(status.effective_weight_pct(), 1);

    // Tick → Stage10Pct.
    orchestrator.tick_all().await;
    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status.current_stage, Stage::Stage10Pct, "tick 1 should advance to 10%");
    assert_eq!(status.effective_weight_pct(), 10);

    // Tick → Stage50Pct.
    orchestrator.tick_all().await;
    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status.current_stage, Stage::Stage50Pct, "tick 2 should advance to 50%");
    assert_eq!(status.effective_weight_pct(), 50);

    // Tick → Promoted.
    orchestrator.tick_all().await;
    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status.current_stage, Stage::Promoted, "tick 3 should promote");
    assert_eq!(status.effective_weight_pct(), 100);
    assert!(status.is_terminal());

    // Verify xDS route was mutated — canary route should exist in store.
    let snapshot = store.snapshot();
    assert!(
        snapshot.routes.contains_key("poulets-api-canary-route"),
        "xDS route must be created during canary rollout"
    );
}

// ---------------------------------------------------------------------------
// Test 2: rollback on SLO breach (3 consecutive ticks)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rollback_on_slo_breach() {
    // Mock Prometheus always returns bad SLO (error_rate=5%, p99=200ms).
    let port = start_mock_prometheus(|| (0.05, 200.0)).await;
    let store = make_store("orders-api");
    let orchestrator = CanaryOrchestrator::new(store.clone());

    let canary_id = orchestrator.start(
        "orders-api",
        "v3.1.0",
        slo_for_port(port),
        Duration::from_secs(0),
    );

    // 3 breach ticks should trigger rollback.
    for tick in 1..=3 {
        orchestrator.tick_all().await;
        let status = orchestrator.status(&canary_id).unwrap();
        if tick < 3 {
            // Should still be in Stage1Pct (breach but not yet 3 consecutive).
            assert_eq!(
                status.current_stage, Stage::Stage1Pct,
                "tick {tick}: should remain at 1% until 3 consecutive breaches"
            );
            assert_eq!(status.consecutive_breaches, tick);
        }
    }

    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(
        status.current_stage, Stage::RolledBack,
        "3 consecutive breaches must trigger rollback"
    );
    assert_eq!(status.effective_weight_pct(), 0);
    assert!(status.rollback_reason.is_some());
    assert!(status.is_terminal());
}

// ---------------------------------------------------------------------------
// Test 3: manual abort (immediate, regardless of SLO)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_manual_abort() {
    let port = start_mock_prometheus(|| (0.001, 10.0)).await;
    let store = make_store("auth-api");
    let orchestrator = CanaryOrchestrator::new(store.clone());

    let canary_id = orchestrator.start(
        "auth-api",
        "v1.5.2",
        slo_for_port(port),
        Duration::from_secs(3600), // long duration — would not advance naturally
    );

    // Abort before any tick.
    let result = orchestrator.abort(&canary_id, "emergency rollback");
    assert!(result.is_ok());

    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status.current_stage, Stage::RolledBack);
    assert_eq!(status.effective_weight_pct(), 0);
    assert_eq!(
        status.rollback_reason.as_deref(),
        Some("emergency rollback")
    );

    // Subsequent tick must not change a terminal canary.
    orchestrator.tick_all().await;
    let status2 = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status2.current_stage, Stage::RolledBack);
}

// ---------------------------------------------------------------------------
// Test 4: pause halts advancement; promote force-promotes from paused state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pause_resume_promote() {
    let port = start_mock_prometheus(|| (0.001, 10.0)).await;
    let store = make_store("payments-api");
    let orchestrator = CanaryOrchestrator::new(store.clone());

    let canary_id = orchestrator.start(
        "payments-api",
        "v4.0.0",
        slo_for_port(port),
        Duration::from_secs(0), // min_stage_duration = 0 → would advance on tick
    );

    // Pause before the first tick.
    let paused = orchestrator.pause(&canary_id).unwrap();
    assert_eq!(paused.current_stage, Stage::Paused);

    // Tick must NOT advance a paused canary.
    orchestrator.tick_all().await;
    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(status.current_stage, Stage::Paused, "paused canary must not advance on tick");

    // Force-promote.
    let promoted = orchestrator.promote(&canary_id).unwrap();
    assert_eq!(promoted.current_stage, Stage::Promoted);
    assert_eq!(promoted.effective_weight_pct(), 100);

    // Another tick must be a no-op (terminal state).
    orchestrator.tick_all().await;
    let final_status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(final_status.current_stage, Stage::Promoted);
}

// ---------------------------------------------------------------------------
// Test 5: breach counter resets after a clean tick
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_breach_counter_reset() {
    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();

    // First 2 ticks: breach; 3rd tick: clean.  Counter should reset.
    let port = start_mock_prometheus(move || {
        let n = call_count_clone.fetch_add(1, Ordering::SeqCst);
        // Each metric query increments counter; each tick calls 2 queries.
        // Calls 0-3 → 2 breached ticks; calls 4-5 → 1 clean tick.
        if n < 4 {
            (0.05, 200.0) // breach
        } else {
            (0.001, 10.0) // ok
        }
    })
    .await;

    let store = make_store("notify-api");
    let orchestrator = CanaryOrchestrator::new(store);

    let canary_id = orchestrator.start(
        "notify-api",
        "v1.0.0",
        slo_for_port(port),
        Duration::from_secs(3600), // will not advance
    );

    // Tick 1 + 2: 2 consecutive breaches.
    orchestrator.tick_all().await;
    orchestrator.tick_all().await;

    {
        let status = orchestrator.status(&canary_id).unwrap();
        assert_eq!(status.consecutive_breaches, 2, "should be 2 after 2 bad ticks");
        assert_eq!(status.current_stage, Stage::Stage1Pct, "still in stage 1");
    }

    // Tick 3: clean → counter must reset to 0.
    orchestrator.tick_all().await;

    let status = orchestrator.status(&canary_id).unwrap();
    assert_eq!(
        status.consecutive_breaches, 0,
        "consecutive_breaches must reset after a clean tick"
    );
    assert_eq!(status.current_stage, Stage::Stage1Pct);
}
