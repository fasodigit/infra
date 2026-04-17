//! Prometheus metrics for the KAYA Functions library subsystem.
//!
//! All metrics are registered against the process-global default registry so
//! the `/metrics` HTTP endpoint exposed by `kaya-observe` surfaces them
//! automatically.

use std::sync::OnceLock;
use std::time::Instant;

use prometheus::{
    register_histogram, register_int_counter, register_int_counter_vec, register_int_gauge,
    Histogram, IntCounter, IntCounterVec, IntGauge,
};

struct Metrics {
    libraries: IntGauge,
    loaded: IntCounter,
    deleted: IntCounter,
    calls: IntCounterVec,
    duration: Histogram,
    timeouts: IntCounter,
    signature_failures: IntCounter,
}

fn metrics() -> &'static Metrics {
    static CELL: OnceLock<Metrics> = OnceLock::new();
    CELL.get_or_init(|| Metrics {
        libraries: register_int_gauge!(
            "kaya_functions_libraries_gauge",
            "Number of currently loaded KAYA Functions libraries"
        )
        .expect("register libraries gauge"),
        loaded: register_int_counter!(
            "kaya_functions_loaded_total",
            "Total number of KAYA Functions library load operations"
        )
        .expect("register loaded counter"),
        deleted: register_int_counter!(
            "kaya_functions_deleted_total",
            "Total number of KAYA Functions library delete operations"
        )
        .expect("register deleted counter"),
        calls: register_int_counter_vec!(
            "kaya_functions_calls_total",
            "Total number of FCALL invocations by library/function",
            &["library", "function"]
        )
        .expect("register calls counter"),
        duration: register_histogram!(
            "kaya_functions_execution_duration_ms",
            "KAYA function execution duration in milliseconds",
            vec![0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 5000.0]
        )
        .expect("register duration histogram"),
        timeouts: register_int_counter!(
            "kaya_functions_timeout_total",
            "Total number of FCALL invocations that exceeded max_execution_ms"
        )
        .expect("register timeouts counter"),
        signature_failures: register_int_counter!(
            "kaya_functions_signature_failures_total",
            "Total number of signature verification failures (load/restore/readonly)"
        )
        .expect("register signature failures counter"),
    })
}

pub fn library_count_set(n: i64) {
    metrics().libraries.set(n);
}

pub fn loaded_inc() {
    metrics().loaded.inc();
}

pub fn deleted_inc() {
    metrics().deleted.inc();
}

pub fn calls_inc(library: &str, function: &str) {
    metrics().calls.with_label_values(&[library, function]).inc();
}

pub fn timeout_inc() {
    metrics().timeouts.inc();
}

pub fn signature_failures_inc() {
    metrics().signature_failures.inc();
}

/// RAII timer that records into `kaya_functions_execution_duration_ms`.
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn observe(self) {
        let elapsed_ms = self.start.elapsed().as_secs_f64() * 1_000.0;
        metrics().duration.observe(elapsed_ms);
    }
}

pub fn start_timer() -> Timer {
    Timer {
        start: Instant::now(),
    }
}
