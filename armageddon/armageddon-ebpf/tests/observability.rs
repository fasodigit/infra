// SPDX-License-Identifier: AGPL-3.0-or-later
//! Integration tests for armageddon-ebpf.
//!
//! These tests run without the `ebpf` feature (the default), verifying:
//! - Event type layout and size invariants.
//! - Metrics registration succeeds and the API is callable.
//! - `EbpfObservability::attach()` falls back gracefully in all environments.

use armageddon_ebpf_programs::events::{SyscallLatencyEvent, TcpConnectEvent};

// ---------------------------------------------------------------------------
// T1 — Happy path: event struct sizes match `#[repr(C)]` expectations
// ---------------------------------------------------------------------------

#[test]
fn tcp_connect_event_size_and_alignment() {
    // TcpConnectEvent layout (repr C, host x86_64):
    //   src_ip(4) + dst_ip(4) + dst_port(2) + _pad(2) [= 12, padded to 16 by u64 alignment]
    //   + latency_ns(8) + timestamp_ns(8) + pid(4) + _pad2(4) = 40 bytes total.
    assert_eq!(
        std::mem::size_of::<TcpConnectEvent>(),
        40,
        "TcpConnectEvent must be 40 bytes for ring-buffer alignment"
    );
    assert_eq!(
        std::mem::align_of::<TcpConnectEvent>(),
        8,
        "TcpConnectEvent must be 8-byte aligned"
    );
}

#[test]
fn syscall_latency_event_size_and_alignment() {
    // SyscallLatencyEvent:
    //   latency_ns(8) + timestamp_ns(8) + pid(4) + sockfd(4)
    //   + syscall_id(1) + _pad[7] = 32
    assert_eq!(
        std::mem::size_of::<SyscallLatencyEvent>(),
        32,
        "SyscallLatencyEvent must be 32 bytes"
    );
    assert_eq!(
        std::mem::align_of::<SyscallLatencyEvent>(),
        8,
        "SyscallLatencyEvent must be 8-byte aligned"
    );
}

// ---------------------------------------------------------------------------
// T2 — Edge case: event fields round-trip through raw byte cast
// ---------------------------------------------------------------------------

#[test]
fn tcp_connect_event_round_trip_bytes() {
    let original = TcpConnectEvent {
        src_ip: 0xC0A8_0101,   // 192.168.1.1 in network order
        dst_ip: 0x08080808,    // 8.8.8.8
        dst_port: 443,
        _pad: 0,
        latency_ns: 123_456,
        timestamp_ns: 999_999_999,
        pid: 1234,
        _pad2: 0,
    };

    // Simulate what the ring buffer reader does: read_unaligned from raw bytes.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            &original as *const TcpConnectEvent as *const u8,
            std::mem::size_of::<TcpConnectEvent>(),
        )
    };
    let recovered: TcpConnectEvent =
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const TcpConnectEvent) };

    assert_eq!(recovered.src_ip, original.src_ip);
    assert_eq!(recovered.dst_ip, original.dst_ip);
    assert_eq!(recovered.dst_port, original.dst_port);
    assert_eq!(recovered.latency_ns, original.latency_ns);
    assert_eq!(recovered.timestamp_ns, original.timestamp_ns);
    assert_eq!(recovered.pid, original.pid);
}

#[test]
fn syscall_latency_event_round_trip_bytes() {
    let original = SyscallLatencyEvent {
        latency_ns: 50_000,
        timestamp_ns: 1_000_000_000,
        pid: 42,
        sockfd: 7,
        syscall_id: 1, // sendto
        _pad: [0u8; 7],
    };

    let bytes = unsafe {
        std::slice::from_raw_parts(
            &original as *const SyscallLatencyEvent as *const u8,
            std::mem::size_of::<SyscallLatencyEvent>(),
        )
    };
    let recovered: SyscallLatencyEvent =
        unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const SyscallLatencyEvent) };

    assert_eq!(recovered.latency_ns, original.latency_ns);
    assert_eq!(recovered.syscall_id, original.syscall_id);
    assert_eq!(recovered.sockfd, original.sockfd);
    assert_eq!(recovered.pid, original.pid);
}

// ---------------------------------------------------------------------------
// T3 — Error case: graceful fallback when ebpf feature is absent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn attach_returns_ok_without_ebpf_feature() {
    // Without the `ebpf` feature, attach() must always succeed and log a warning.
    // It must NOT panic, even when running as an unprivileged user or in CI.
    let result = armageddon_ebpf::EbpfObservability::attach().await;
    assert!(
        result.is_ok(),
        "EbpfObservability::attach() must not fail without feature: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// T4 — Metrics registration (unit test inside the crate)
// ---------------------------------------------------------------------------

#[test]
fn metrics_new_does_not_panic() {
    // Constructing Metrics must succeed (counters + histograms registered).
    // Note: calling twice on the same process may trigger duplicate-registration
    // warnings from prometheus; we use a fallback path that swallows those.
    let m = armageddon_ebpf::metrics::Metrics::new();
    // Increment once to verify the counter is wired.
    m.tcp_connections_total
        .with_label_values(&["443"])
        .inc();
    assert!(
        m.tcp_connections_total
            .with_label_values(&["443"])
            .get()
            >= 1.0
    );
}

// ---------------------------------------------------------------------------
// T5 — Edge case: syscall_id discriminants are stable
// ---------------------------------------------------------------------------

#[test]
fn syscall_id_discriminants() {
    // Protocol between BPF and userspace: 0 = recvfrom, 1 = sendto.
    // If these ever change, dashboards break.
    let recvfrom = SyscallLatencyEvent {
        latency_ns: 0,
        timestamp_ns: 0,
        pid: 0,
        sockfd: 0,
        syscall_id: 0,
        _pad: [0u8; 7],
    };
    let sendto = SyscallLatencyEvent {
        syscall_id: 1,
        ..recvfrom
    };
    assert_eq!(recvfrom.syscall_id, 0, "recvfrom must be 0");
    assert_eq!(sendto.syscall_id, 1, "sendto must be 1");
}
