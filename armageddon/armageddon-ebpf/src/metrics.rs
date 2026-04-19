// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prometheus metrics exposed by the eBPF observability subsystem.
//!
//! Metrics are registered lazily on first `EbpfObservability::attach()` call.
//! They remain zero-valued (but registered) when the eBPF feature is disabled or
//! the graceful fallback is triggered — this avoids gaps in dashboards.

use prometheus::{
    register_counter_vec, register_histogram_vec, CounterVec, HistogramVec,
};
use std::sync::OnceLock;
use tracing::warn;

// -- global metrics singleton --

#[allow(dead_code)] // used only under cfg(all(target_os = "linux", feature = "ebpf"))
pub(crate) static METRICS: OnceLock<Metrics> = OnceLock::new();

/// Container for all eBPF-related Prometheus metrics.
#[derive(Debug)]
pub struct Metrics {
    /// Total outbound TCP connections traced kernel-side.
    ///
    /// Labels: `dst_port`.
    pub tcp_connections_total: CounterVec,

    /// Histogram of `recvfrom` / `sendto` call latency in seconds.
    ///
    /// Labels: `syscall` (recvfrom | sendto).
    pub syscall_latency_seconds: HistogramVec,
}

impl Metrics {
    /// Register metrics with the default Prometheus registry.
    ///
    /// Panics if called twice (prevented by `OnceLock`).
    pub fn new() -> Self {
        let tcp_connections_total = register_counter_vec!(
            "armageddon_ebpf_tcp_connections_total",
            "Total outbound TCP connections observed by the kernel kprobe",
            &["dst_port"]
        )
        .unwrap_or_else(|e| {
            warn!("failed to register tcp_connections_total: {e}");
            // Return a no-op counter vec so the rest of the code compiles.
            prometheus::CounterVec::new(
                prometheus::Opts::new(
                    "armageddon_ebpf_tcp_connections_total_fallback",
                    "fallback",
                ),
                &["dst_port"],
            )
            .expect("fallback counter")
        });

        // Buckets from 1 µs to 1 s in roughly exponential steps.
        let latency_buckets = vec![
            0.000_001, // 1 µs
            0.000_010, // 10 µs
            0.000_100, // 100 µs
            0.001_000, // 1 ms
            0.005_000, // 5 ms
            0.010_000, // 10 ms
            0.050_000, // 50 ms
            0.100_000, // 100 ms
            0.500_000, // 500 ms
            1.000_000, // 1 s
        ];

        let syscall_latency_seconds = register_histogram_vec!(
            "armageddon_ebpf_syscall_latency_seconds",
            "Latency distribution of recvfrom/sendto syscalls measured kernel-side",
            &["syscall"],
            latency_buckets
        )
        .unwrap_or_else(|e| {
            warn!("failed to register syscall_latency_seconds: {e}");
            prometheus::HistogramVec::new(
                prometheus::HistogramOpts::new(
                    "armageddon_ebpf_syscall_latency_seconds_fallback",
                    "fallback",
                ),
                &["syscall"],
            )
            .expect("fallback histogram")
        });

        Self {
            tcp_connections_total,
            syscall_latency_seconds,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

// -- convenience record helpers --

/// Increment the TCP connection counter for the given destination port.
#[allow(dead_code)]
pub(crate) fn record_tcp_connection(dst_port: u16) {
    if let Some(m) = METRICS.get() {
        m.tcp_connections_total
            .with_label_values(&[&dst_port.to_string()])
            .inc();
    }
}

/// Observe a syscall latency sample.
///
/// `syscall` must be `"recvfrom"` or `"sendto"`.
#[allow(dead_code)]
pub(crate) fn record_syscall_latency(syscall: &str, latency_secs: f64) {
    if let Some(m) = METRICS.get() {
        m.syscall_latency_seconds
            .with_label_values(&[syscall])
            .observe(latency_secs);
    }
}
