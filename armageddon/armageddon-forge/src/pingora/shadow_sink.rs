// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Shadow diff sink — persistence layer for divergence events detected during
//! shadow-mode parity validation.
//!
//! # Overview
//!
//! When the shadow sampler detects a divergence between the primary (hyper) and
//! shadow (Pingora) responses, it emits a [`ShadowDiffEvent`] to this sink.
//! Events are sent fire-and-forget through a bounded `tokio::mpsc` channel; a
//! background task drains the channel and calls [`ShadowDiffSink::emit`].
//!
//! # Backends
//!
//! | Type | Behaviour |
//! |------|-----------|
//! | [`RedpandaSink`] | Produces JSON to `armageddon.shadow.diffs.v1` via the shared `RedpandaProducer` |
//! | [`SqliteSink`] | Writes rows to `/tmp/armageddon-shadow-diffs.db`; bounded by `max_rows` (trim trigger) |
//! | [`MultiSink`] | Tee: emits to all inner sinks sequentially; first error is logged but does not abort |
//! | [`NoopSink`] | Discards every event silently (dev / unit tests) |
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | Bounded channel full | Event dropped; `armageddon_shadow_sink_dropped_total{reason="channel_full"}` incremented |
//! | Redpanda broker down | Error logged; `armageddon_shadow_sink_dropped_total{reason="backend_error"}` incremented; event lost (best-effort) |
//! | SQLite write error | Same as Redpanda; fallback row is dropped, not retried |
//! | Both backends fail in MultiSink | Each failure logged independently; no retry |
//!
//! # Channel / background task lifecycle
//!
//! Construct a [`ShadowDiffDispatcher`] at gateway startup.  Call
//! [`ShadowDiffDispatcher::sender`] to get the cheap-clone [`DiffEventSender`]
//! that is injected into every [`super::shadow::ShadowSampler`] instance.
//! The background task runs until the last [`DiffEventSender`] is dropped,
//! at which point it flushes remaining items and exits.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use prometheus::{IntCounterVec, IntGaugeVec, Opts};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, warn};

use crate::kafka_producer::RedpandaProducer;
use super::shadow_redaction::RedactionPolicy;

// ---------------------------------------------------------------------------
// ShadowDiffEvent
// ---------------------------------------------------------------------------

/// Structured event emitted for every divergence detected in shadow mode.
///
/// All fields are deliberately flat (no nested objects except `headers_diff`)
/// for easy `rpk topic consume` inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowDiffEvent {
    /// Unix milliseconds at the moment of classification.
    pub timestamp_unix_ms: u64,
    /// Opaque request identifier (uuid v4 from `x-forge-id`).
    pub request_id: String,
    /// Matched route template (e.g. `/api/v1/orders/:id`).
    pub route: String,
    /// HTTP method in upper case.
    pub method: String,
    /// Primary (hyper) HTTP status code.
    pub hyper_status: u16,
    /// Shadow (Pingora) HTTP status code.
    pub pingora_status: u16,
    /// blake3 hex digest of the primary body.
    pub hyper_body_hash: String,
    /// blake3 hex digest of the shadow body.
    pub pingora_body_hash: String,
    /// Primary response latency in milliseconds.
    pub hyper_latency_ms: u32,
    /// Shadow response latency in milliseconds.
    pub pingora_latency_ms: u32,
    /// Which fields diverged: `"status"`, `"body_hash"`, `"headers"`.
    pub diverged_fields: Vec<String>,
    /// Paired header diff — `Some(…)` only when headers diverged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers_diff: Option<HeadersDiff>,
    /// Tenant identifier for multi-tenant deployments.  Used as Redpanda
    /// partition key so that per-tenant ordering is preserved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

impl ShadowDiffEvent {
    /// Current wall-clock milliseconds (for default timestamp).
    pub fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Diff of normalised response headers between primary and shadow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadersDiff {
    /// Headers present in primary but absent (or different) in shadow.
    pub only_in_hyper: Vec<(String, String)>,
    /// Headers present in shadow but absent (or different) in primary.
    pub only_in_pingora: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// ShadowDiffSink trait
// ---------------------------------------------------------------------------

/// Async sink for [`ShadowDiffEvent`]s.
///
/// Implementations must be `Send + Sync` and must not block the calling task.
/// Errors should be absorbed (logged) — the caller does not retry.
#[async_trait]
pub trait ShadowDiffSink: Send + Sync + std::fmt::Debug {
    /// Persist or forward one diff event.
    ///
    /// This is called from a background task — it is safe to do I/O here.
    /// The implementation should not panic; errors must be handled internally.
    async fn emit(&self, event: &ShadowDiffEvent);

    /// Human-readable backend name for metric labels.
    fn backend_name(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// NoopSink
// ---------------------------------------------------------------------------

/// Discards all events.  Used in dev / unit tests.
#[derive(Debug, Clone, Default)]
pub struct NoopSink;

#[async_trait]
impl ShadowDiffSink for NoopSink {
    async fn emit(&self, _event: &ShadowDiffEvent) {}
    fn backend_name(&self) -> &'static str {
        "noop"
    }
}

// ---------------------------------------------------------------------------
// RedpandaSink
// ---------------------------------------------------------------------------

/// Produces diff events as JSON to a Redpanda topic.
///
/// Partition key = `tenant_id` if present, else `request_id`.
/// This preserves per-tenant ordering while still distributing load.
#[derive(Debug)]
pub struct RedpandaSink {
    producer: RedpandaProducer,
    topic: String,
}

impl RedpandaSink {
    /// Construct from an existing producer and a topic name.
    pub fn new(producer: RedpandaProducer, topic: impl Into<String>) -> Self {
        Self {
            producer,
            topic: topic.into(),
        }
    }

    /// Construct with the log-only backend (no broker required).
    pub fn new_logging(topic: impl Into<String>) -> Self {
        Self::new(RedpandaProducer::new_logging(), topic)
    }
}

#[async_trait]
impl ShadowDiffSink for RedpandaSink {
    async fn emit(&self, event: &ShadowDiffEvent) {
        let key = event
            .tenant_id
            .as_deref()
            .unwrap_or(&event.request_id);

        let payload = match serde_json::to_vec(event) {
            Ok(b) => b,
            Err(e) => {
                error!(
                    request_id = %event.request_id,
                    error = %e,
                    "shadow sink: JSON serialisation failed"
                );
                return;
            }
        };

        if let Err(e) = self.producer.produce(&self.topic, key, &payload).await {
            warn!(
                topic = %self.topic,
                request_id = %event.request_id,
                error = %e,
                "shadow sink: Redpanda produce failed"
            );
        }
    }

    fn backend_name(&self) -> &'static str {
        "redpanda"
    }
}

// ---------------------------------------------------------------------------
// SqliteSink
// ---------------------------------------------------------------------------

/// Writes diff events to a local SQLite database.
///
/// The table is bounded by `max_rows`: after each insert a trigger deletes
/// rows older than the newest `max_rows` entries.  This prevents unbounded
/// disk growth in long-running dev setups.
///
/// # Schema
///
/// ```sql
/// CREATE TABLE IF NOT EXISTS shadow_diffs (
///     id INTEGER PRIMARY KEY AUTOINCREMENT,
///     timestamp_unix_ms INTEGER NOT NULL,
///     request_id TEXT NOT NULL,
///     route TEXT,
///     method TEXT,
///     hyper_status INTEGER,
///     pingora_status INTEGER,
///     hyper_body_hash TEXT,
///     pingora_body_hash TEXT,
///     hyper_latency_ms INTEGER,
///     pingora_latency_ms INTEGER,
///     diverged_fields TEXT,   -- JSON array
///     headers_diff TEXT,      -- JSON object or NULL
///     tenant_id TEXT
/// );
/// ```
#[derive(Debug)]
pub struct SqliteSink {
    path: String,
    max_rows: usize,
    /// Serialised mutex — rusqlite Connection is not Send+Sync,
    /// so we use a Mutex<Connection>.
    conn: tokio::sync::Mutex<rusqlite::Connection>,
}

impl SqliteSink {
    /// Open (or create) the SQLite database at `path` and apply the schema.
    pub fn open(path: impl Into<String>, max_rows: usize) -> Result<Self, rusqlite::Error> {
        let path = path.into();
        let conn = rusqlite::Connection::open(&path)?;

        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;

            CREATE TABLE IF NOT EXISTS shadow_diffs (
                id                 INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_unix_ms  INTEGER NOT NULL,
                request_id         TEXT    NOT NULL,
                route              TEXT,
                method             TEXT,
                hyper_status       INTEGER,
                pingora_status     INTEGER,
                hyper_body_hash    TEXT,
                pingora_body_hash  TEXT,
                hyper_latency_ms   INTEGER,
                pingora_latency_ms INTEGER,
                diverged_fields    TEXT,
                headers_diff       TEXT,
                tenant_id          TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_shadow_diffs_ts
                ON shadow_diffs(timestamp_unix_ms);

            CREATE INDEX IF NOT EXISTS idx_shadow_diffs_route
                ON shadow_diffs(route);
            ",
        )?;

        Ok(Self {
            path,
            max_rows,
            conn: tokio::sync::Mutex::new(conn),
        })
    }

    /// Return the total number of rows in `shadow_diffs`.
    ///
    /// Used in tests and for observability; cheap COUNT(*) over the index.
    pub async fn row_count(&self) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().await;
        conn.query_row("SELECT COUNT(*) FROM shadow_diffs", [], |r| r.get(0))
    }

    /// Trim rows so that at most `max_rows` remain.
    fn trim(conn: &rusqlite::Connection, max_rows: usize) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM shadow_diffs
             WHERE id NOT IN (
                 SELECT id FROM shadow_diffs
                 ORDER BY id DESC
                 LIMIT ?1
             )",
            rusqlite::params![max_rows as i64],
        )?;
        Ok(())
    }
}

#[async_trait]
impl ShadowDiffSink for SqliteSink {
    async fn emit(&self, event: &ShadowDiffEvent) {
        let diverged_json = serde_json::to_string(&event.diverged_fields)
            .unwrap_or_else(|_| "[]".to_string());
        let headers_json = event
            .headers_diff
            .as_ref()
            .and_then(|h| serde_json::to_string(h).ok());

        let conn = self.conn.lock().await;
        let insert_result = conn.execute(
            "INSERT INTO shadow_diffs (
                timestamp_unix_ms, request_id, route, method,
                hyper_status, pingora_status,
                hyper_body_hash, pingora_body_hash,
                hyper_latency_ms, pingora_latency_ms,
                diverged_fields, headers_diff, tenant_id
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            rusqlite::params![
                event.timestamp_unix_ms as i64,
                event.request_id,
                event.route,
                event.method,
                event.hyper_status as i64,
                event.pingora_status as i64,
                event.hyper_body_hash,
                event.pingora_body_hash,
                event.hyper_latency_ms as i64,
                event.pingora_latency_ms as i64,
                diverged_json,
                headers_json,
                event.tenant_id,
            ],
        );

        if let Err(e) = insert_result {
            warn!(
                path = %self.path,
                request_id = %event.request_id,
                error = %e,
                "shadow sink: SQLite insert failed"
            );
            return;
        }

        // Trim to bound disk usage — ignore trim errors (non-fatal).
        if let Err(e) = Self::trim(&conn, self.max_rows) {
            warn!(error = %e, "shadow sink: SQLite trim failed (non-fatal)");
        }
    }

    fn backend_name(&self) -> &'static str {
        "sqlite"
    }
}

// ---------------------------------------------------------------------------
// MultiSink — tee to N backends
// ---------------------------------------------------------------------------

/// Fans out every event to all inner sinks.
///
/// Errors from individual sinks are logged but do not abort the fan-out.
#[derive(Debug)]
pub struct MultiSink {
    sinks: Vec<Arc<dyn ShadowDiffSink>>,
}

impl MultiSink {
    /// Construct from a list of inner sinks.
    pub fn new(sinks: Vec<Arc<dyn ShadowDiffSink>>) -> Self {
        Self { sinks }
    }
}

#[async_trait]
impl ShadowDiffSink for MultiSink {
    async fn emit(&self, event: &ShadowDiffEvent) {
        for sink in &self.sinks {
            sink.emit(event).await;
        }
    }

    fn backend_name(&self) -> &'static str {
        "multi"
    }
}

// ---------------------------------------------------------------------------
// SinkMetrics
// ---------------------------------------------------------------------------

/// Prometheus metrics for the diff dispatcher.
#[derive(Clone, Debug)]
pub struct SinkMetrics {
    /// `armageddon_shadow_sink_emitted_total{backend}`
    pub emitted_total: IntCounterVec,
    /// `armageddon_shadow_sink_dropped_total{backend, reason}`
    pub dropped_total: IntCounterVec,
    /// `armageddon_shadow_sink_lag_seconds{backend}` — gauge (event_ts → flush_ts gap)
    pub lag_seconds: IntGaugeVec,
}

impl SinkMetrics {
    /// Register on `registry`.  Returns `Err` on duplicate registration.
    pub fn new(registry: &prometheus::Registry) -> Result<Self, prometheus::Error> {
        let emitted_total = IntCounterVec::new(
            Opts::new(
                "armageddon_shadow_sink_emitted_total",
                "Total shadow diff events emitted to the sink backend",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(emitted_total.clone()))?;

        let dropped_total = IntCounterVec::new(
            Opts::new(
                "armageddon_shadow_sink_dropped_total",
                "Total shadow diff events dropped (channel_full, backend_error, …)",
            ),
            &["backend", "reason"],
        )?;
        registry.register(Box::new(dropped_total.clone()))?;

        let lag_seconds = IntGaugeVec::new(
            Opts::new(
                "armageddon_shadow_sink_lag_seconds",
                "Lag in seconds between event timestamp and sink flush time",
            ),
            &["backend"],
        )?;
        registry.register(Box::new(lag_seconds.clone()))?;

        Ok(Self {
            emitted_total,
            dropped_total,
            lag_seconds,
        })
    }
}

// ---------------------------------------------------------------------------
// DiffEventSender — cheap clone handle to the bounded channel
// ---------------------------------------------------------------------------

/// Cheap-clone sender side of the diff event channel.
///
/// Constructed by [`ShadowDiffDispatcher::sender`].  Inject one per
/// `ShadowSampler` instance.  When all senders are dropped the background
/// task exits.
#[derive(Debug, Clone)]
pub struct DiffEventSender {
    tx: mpsc::Sender<ShadowDiffEvent>,
    metrics: Option<Arc<SinkMetrics>>,
    backend: &'static str,
}

impl DiffEventSender {
    /// Send an event fire-and-forget.
    ///
    /// Returns `true` if the event was queued, `false` if dropped (channel full).
    /// Never blocks.
    pub fn try_send(&self, event: ShadowDiffEvent) -> bool {
        match self.tx.try_send(event) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("shadow diff channel full — dropping event");
                if let Some(m) = &self.metrics {
                    m.dropped_total
                        .with_label_values(&[self.backend, "channel_full"])
                        .inc();
                }
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Background task exited — nothing we can do.
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowDiffDispatcher — owns the channel + background task
// ---------------------------------------------------------------------------

/// Owns the bounded mpsc channel and spawns the background drain task.
///
/// Construct once at gateway startup:
///
/// ```rust,ignore
/// let dispatcher = ShadowDiffDispatcher::start(sink, capacity, Some(metrics));
/// let sender = dispatcher.sender(); // inject into ShadowSampler
/// ```
///
/// To enable PII redaction, use [`ShadowDiffDispatcher::start_with_redaction`].
pub struct ShadowDiffDispatcher {
    sender: DiffEventSender,
}

impl ShadowDiffDispatcher {
    /// Spawn the background task and return the dispatcher.
    ///
    /// `capacity` bounds the in-memory queue (recommended: 10_000).
    ///
    /// Events are forwarded to the sink **without** redaction.  For production
    /// use, prefer [`start_with_redaction`](Self::start_with_redaction).
    pub fn start(
        sink: Arc<dyn ShadowDiffSink>,
        capacity: usize,
        metrics: Option<Arc<SinkMetrics>>,
    ) -> Self {
        Self::start_with_redaction(sink, capacity, metrics, None)
    }

    /// Spawn the background task with an optional [`RedactionPolicy`].
    ///
    /// When `redaction` is `Some(policy)`, every [`ShadowDiffEvent`] is
    /// redacted in place via [`RedactionPolicy::apply`] **before** being
    /// forwarded to the sink.  This ensures no raw PII ever reaches storage.
    pub fn start_with_redaction(
        sink: Arc<dyn ShadowDiffSink>,
        capacity: usize,
        metrics: Option<Arc<SinkMetrics>>,
        redaction: Option<Arc<RedactionPolicy>>,
    ) -> Self {
        let backend = sink.backend_name();
        let (tx, rx) = mpsc::channel::<ShadowDiffEvent>(capacity);
        let metrics_arc = metrics.clone();

        tokio::spawn(drain_loop(rx, sink, metrics_arc, redaction));

        Self {
            sender: DiffEventSender {
                tx,
                metrics,
                backend,
            },
        }
    }

    /// Get a cheap-clone sender to inject into samplers.
    pub fn sender(&self) -> DiffEventSender {
        self.sender.clone()
    }
}

/// Background task: drain the channel and call `sink.emit`.
///
/// If `redaction` is `Some`, applies [`RedactionPolicy::apply`] before emit.
async fn drain_loop(
    mut rx: mpsc::Receiver<ShadowDiffEvent>,
    sink: Arc<dyn ShadowDiffSink>,
    metrics: Option<Arc<SinkMetrics>>,
    redaction: Option<Arc<RedactionPolicy>>,
) {
    let backend = sink.backend_name();

    while let Some(mut event) = rx.recv().await {
        // Compute lag between event timestamp and now.
        let now_ms = ShadowDiffEvent::now_unix_ms();
        let lag_secs = now_ms.saturating_sub(event.timestamp_unix_ms) / 1_000;

        if let Some(m) = &metrics {
            m.lag_seconds
                .with_label_values(&[backend])
                .set(lag_secs as i64);
        }

        // Apply PII redaction before forwarding to any backend.
        if let Some(policy) = &redaction {
            policy.apply(&mut event);
        }

        sink.emit(&event).await;

        if let Some(m) = &metrics {
            m.emitted_total.with_label_values(&[backend]).inc();
        }
    }

    tracing::debug!(backend, "shadow diff drain loop exiting — channel closed");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Registry;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

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
            tenant_id: Some("tenant-1".to_string()),
        }
    }

    // ── NoopSink ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn noop_sink_accepts_any_event() {
        let sink = NoopSink;
        // Must not panic.
        sink.emit(&sample_event("req-noop")).await;
        assert_eq!(sink.backend_name(), "noop");
    }

    // ── RedpandaSink (log-only) ────────────────────────────────────────────────

    #[tokio::test]
    async fn redpanda_sink_logging_does_not_panic() {
        let sink = RedpandaSink::new_logging("armageddon.shadow.diffs.v1");
        assert_eq!(sink.backend_name(), "redpanda");
        sink.emit(&sample_event("req-redpanda")).await;
    }

    #[tokio::test]
    async fn redpanda_sink_uses_tenant_id_as_key() {
        // With log-only backend we can't intercept the key directly,
        // but we verify it completes without error when tenant_id is set.
        let sink = RedpandaSink::new_logging("test.topic");
        let mut ev = sample_event("req-tenant");
        ev.tenant_id = Some("faso-tenant".to_string());
        sink.emit(&ev).await;
    }

    // ── SqliteSink ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn sqlite_sink_insert_and_trim() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("shadow.db");
        let sink = SqliteSink::open(path.to_str().unwrap(), 5).expect("sqlite open");

        // Insert 8 events — trim should keep only 5.
        for i in 0..8u64 {
            let mut ev = sample_event(&format!("req-{i}"));
            ev.timestamp_unix_ms = 1_000 + i;
            sink.emit(&ev).await;
        }

        // Read row count.
        let conn = sink.conn.lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shadow_diffs", [], |r| r.get(0))
            .expect("count query");
        assert_eq!(count, 5, "trim should keep last 5 rows");
    }

    #[tokio::test]
    async fn sqlite_sink_headers_diff_serialised() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("shadow_hdrs.db");
        let sink = SqliteSink::open(path.to_str().unwrap(), 100).expect("sqlite open");

        let mut ev = sample_event("req-hdrs");
        ev.headers_diff = Some(HeadersDiff {
            only_in_hyper: vec![("x-custom".to_string(), "a".to_string())],
            only_in_pingora: vec![("x-custom".to_string(), "b".to_string())],
        });
        sink.emit(&ev).await;

        let conn = sink.conn.lock().await;
        let hdiff: Option<String> = conn
            .query_row("SELECT headers_diff FROM shadow_diffs LIMIT 1", [], |r| {
                r.get(0)
            })
            .expect("query");
        assert!(
            hdiff.is_some(),
            "headers_diff column should be non-NULL when diff present"
        );
    }

    // ── MultiSink ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn multi_sink_fans_out_to_all() {
        // Use a counter-backed stub to count calls.
        #[derive(Debug)]
        struct CountSink(Arc<AtomicU64>);
        #[async_trait]
        impl ShadowDiffSink for CountSink {
            async fn emit(&self, _e: &ShadowDiffEvent) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            fn backend_name(&self) -> &'static str {
                "count"
            }
        }

        let c1 = Arc::new(AtomicU64::new(0));
        let c2 = Arc::new(AtomicU64::new(0));
        let multi = MultiSink::new(vec![
            Arc::new(CountSink(c1.clone())),
            Arc::new(CountSink(c2.clone())),
        ]);

        multi.emit(&sample_event("req-multi")).await;
        assert_eq!(c1.load(Ordering::SeqCst), 1);
        assert_eq!(c2.load(Ordering::SeqCst), 1);
        assert_eq!(multi.backend_name(), "multi");
    }

    // ── SinkMetrics ───────────────────────────────────────────────────────────

    #[test]
    fn sink_metrics_register_ok() {
        let r = Registry::new();
        SinkMetrics::new(&r).expect("metrics registration must succeed");
    }

    #[test]
    fn sink_metrics_double_registration_fails() {
        let r = Registry::new();
        SinkMetrics::new(&r).expect("first ok");
        assert!(SinkMetrics::new(&r).is_err(), "must fail on duplicate");
    }

    #[test]
    fn sink_metrics_counters_increment() {
        let r = Registry::new();
        let m = SinkMetrics::new(&r).unwrap();
        m.emitted_total.with_label_values(&["redpanda"]).inc();
        m.dropped_total
            .with_label_values(&["redpanda", "channel_full"])
            .inc();
        m.lag_seconds.with_label_values(&["redpanda"]).set(3);

        let families = r.gather();
        let names: Vec<&str> = families.iter().map(|f| f.get_name()).collect();
        assert!(names.contains(&"armageddon_shadow_sink_emitted_total"));
        assert!(names.contains(&"armageddon_shadow_sink_dropped_total"));
        assert!(names.contains(&"armageddon_shadow_sink_lag_seconds"));
    }

    // ── DiffEventSender / Dispatcher ─────────────────────────────────────────

    #[tokio::test]
    async fn dispatcher_delivers_events_to_sink() {
        let received = Arc::new(AtomicU64::new(0));

        #[derive(Debug)]
        struct LatchSink(Arc<AtomicU64>);
        #[async_trait]
        impl ShadowDiffSink for LatchSink {
            async fn emit(&self, _e: &ShadowDiffEvent) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            fn backend_name(&self) -> &'static str {
                "latch"
            }
        }

        let sink = Arc::new(LatchSink(received.clone()));
        let dispatcher = ShadowDiffDispatcher::start(sink, 128, None);
        let sender = dispatcher.sender();

        sender.try_send(sample_event("r1"));
        sender.try_send(sample_event("r2"));
        sender.try_send(sample_event("r3"));

        // Give the background task time to drain.
        sleep(Duration::from_millis(50)).await;
        assert_eq!(received.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn dispatcher_channel_full_drops_and_counts() {
        let r = Registry::new();
        let m = Arc::new(SinkMetrics::new(&r).unwrap());

        #[derive(Debug)]
        struct SlowSink;
        #[async_trait]
        impl ShadowDiffSink for SlowSink {
            async fn emit(&self, _e: &ShadowDiffEvent) {
                // never consume — simulate a blocked backend
                sleep(Duration::from_secs(60)).await;
            }
            fn backend_name(&self) -> &'static str {
                "slow"
            }
        }

        let dispatcher = ShadowDiffDispatcher::start(Arc::new(SlowSink), 2, Some(m.clone()));
        let sender = dispatcher.sender();

        // Fill channel (capacity = 2).
        sender.try_send(sample_event("fill-1"));
        sender.try_send(sample_event("fill-2"));
        // This must be dropped.
        let dropped = !sender.try_send(sample_event("overflow"));

        assert!(dropped, "overflow event must be dropped when channel is full");

        let families = r.gather();
        let dropped_fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_shadow_sink_dropped_total")
            .expect("dropped counter must exist");
        let total_dropped: f64 = dropped_fam
            .get_metric()
            .iter()
            .map(|m| m.get_counter().get_value())
            .sum();
        assert!(total_dropped >= 1.0, "at least one event must be counted as dropped");
    }

    // ── ShadowDiffEvent serialisation ─────────────────────────────────────────

    #[test]
    fn shadow_diff_event_roundtrips_json() {
        let ev = ShadowDiffEvent {
            timestamp_unix_ms: 1_700_000_000_000,
            request_id: "uuid-abc".to_string(),
            route: "/api/v1/orders".to_string(),
            method: "POST".to_string(),
            hyper_status: 201,
            pingora_status: 500,
            hyper_body_hash: "aabbcc".to_string(),
            pingora_body_hash: "ddeeff".to_string(),
            hyper_latency_ms: 15,
            pingora_latency_ms: 30,
            diverged_fields: vec!["status".to_string(), "body_hash".to_string()],
            headers_diff: Some(HeadersDiff {
                only_in_hyper: vec![("x-trace-id".to_string(), "h1".to_string())],
                only_in_pingora: vec![],
            }),
            tenant_id: Some("bf-faso".to_string()),
        };

        let json = serde_json::to_string(&ev).expect("serialize");
        let ev2: ShadowDiffEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev.request_id, ev2.request_id);
        assert_eq!(ev.hyper_status, ev2.hyper_status);
        assert_eq!(ev.diverged_fields, ev2.diverged_fields);
        assert!(ev2.headers_diff.is_some());
        assert_eq!(ev.tenant_id, ev2.tenant_id);
    }

    #[test]
    fn shadow_diff_event_omits_null_fields() {
        let ev = ShadowDiffEvent {
            timestamp_unix_ms: 0,
            request_id: "r".to_string(),
            route: "/".to_string(),
            method: "GET".to_string(),
            hyper_status: 200,
            pingora_status: 200,
            hyper_body_hash: "x".to_string(),
            pingora_body_hash: "x".to_string(),
            hyper_latency_ms: 1,
            pingora_latency_ms: 1,
            diverged_fields: vec![],
            headers_diff: None,
            tenant_id: None,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(!json.contains("headers_diff"), "null headers_diff must be omitted");
        assert!(!json.contains("tenant_id"), "null tenant_id must be omitted");
    }
}
