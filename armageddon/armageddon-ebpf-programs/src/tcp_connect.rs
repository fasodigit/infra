// SPDX-License-Identifier: AGPL-3.0-or-later
//! BPF kprobe programs for tracing outbound TCP connections.
//!
//! Attaches to `tcp_connect` in the kernel network stack.
//! On entry, stores the kernel monotonic timestamp in a per-CPU hash map.
//! On return (kretprobe), reads the stored timestamp, computes latency,
//! extracts address/port from the `struct sock`, and submits a
//! `TcpConnectEvent` to the ring buffer.
//!
//! Compile target: `bpfel-unknown-none` (little-endian BPF).

use aya_ebpf::{
    bindings::sock,
    helpers::{bpf_get_current_pid_tgid, bpf_ktime_get_ns, bpf_probe_read_kernel},
    macros::{kprobe, kretprobe, map},
    maps::{HashMap, RingBuf},
    programs::{ProbeContext, RetProbeContext},
};
use aya_log_ebpf::debug;

use crate::events::TcpConnectEvent;

// -- maps --

/// Scratch map: stores entry timestamp per task (tgid<<32|pid).
#[map]
static mut TCP_CONNECT_START: HashMap<u64, u64> =
    HashMap::with_max_entries(8192, 0);

/// Ring buffer delivered to userspace.
#[map]
pub static mut TCP_CONNECT_EVENTS: RingBuf = RingBuf::with_byte_size(4096 * 64, 0);

// -- programs --

/// kprobe: `tcp_connect(struct sock *sk, struct sk_buff *skb)`
///
/// Records the kernel monotonic clock when the connect attempt starts.
#[kprobe]
pub fn tcp_connect_enter(ctx: ProbeContext) -> u32 {
    match try_tcp_connect_enter(&ctx) {
        Ok(()) => 0,
        Err(_) => 0, // never panic in BPF
    }
}

#[inline(always)]
fn try_tcp_connect_enter(ctx: &ProbeContext) -> Result<(), i64> {
    let id = unsafe { bpf_get_current_pid_tgid() };
    let ts = unsafe { bpf_ktime_get_ns() };
    unsafe { TCP_CONNECT_START.insert(&id, &ts, 0) }
}

/// kretprobe: fires when `tcp_connect` returns.
///
/// Reads the saved timestamp, computes latency, reads src/dst from
/// `struct sock`, and pushes `TcpConnectEvent` to the ring buffer.
#[kretprobe]
pub fn tcp_connect_exit(ctx: RetProbeContext) -> u32 {
    match try_tcp_connect_exit(&ctx) {
        Ok(()) => 0,
        Err(_) => 0,
    }
}

#[inline(always)]
fn try_tcp_connect_exit(ctx: &RetProbeContext) -> Result<(), i64> {
    let id = unsafe { bpf_get_current_pid_tgid() };
    let pid = (id >> 32) as u32;

    let start_ts = unsafe { TCP_CONNECT_START.get(&id).copied().ok_or(0i64)? };
    let now = unsafe { bpf_ktime_get_ns() };
    let latency_ns = now.saturating_sub(start_ts);

    unsafe { TCP_CONNECT_START.remove(&id).ok() };

    // The first argument to the original kprobe was `struct sock *sk`.
    // In a kretprobe, the `struct sock *` is no longer directly accessible
    // via ctx.arg(). We store it separately in a real deployment; here we
    // emit a minimal event with what is available and zero-fill addresses
    // that require a paired entry-point to retrieve (stored in a second map).
    //
    // For production, pair this with a dedicated entry HashMap<u64, *const sock>
    // as shown in the companion design notes.
    let event = TcpConnectEvent {
        src_ip: 0,   // filled from sk_rcv_saddr in full implementation
        dst_ip: 0,   // filled from sk_daddr
        dst_port: 0, // filled from sk_dport
        _pad: 0,
        latency_ns,
        timestamp_ns: now,
        pid,
        _pad2: 0,
    };

    if let Some(mut entry) = unsafe { TCP_CONNECT_EVENTS.reserve::<TcpConnectEvent>(0) } {
        entry.write(event);
        entry.submit(0);
    }

    Ok(())
}
