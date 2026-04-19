// SPDX-License-Identifier: AGPL-3.0-or-later
//! BPF tracepoint programs for `recvfrom` / `sendto` latency measurement.
//!
//! Attaches to `sys_enter_recvfrom`, `sys_enter_sendto`,
//! `sys_exit_recvfrom`, and `sys_exit_sendto` tracepoints.
//!
//! Entry programs record `(pid_tgid → timestamp)` in a per-CPU hash map.
//! Exit programs compute latency and push `SyscallLatencyEvent` to a ring buffer.
//!
//! Compile target: `bpfel-unknown-none`.

use aya_ebpf::{
    helpers::{bpf_get_current_pid_tgid, bpf_ktime_get_ns},
    macros::{map, tracepoint},
    maps::{HashMap, RingBuf},
    programs::TracePointContext,
};

use crate::events::SyscallLatencyEvent;

// -- constants --

/// Syscall identifier: recvfrom
const SYSCALL_RECVFROM: u8 = 0;
/// Syscall identifier: sendto
const SYSCALL_SENDTO: u8 = 1;

// -- maps --

/// Scratch map: entry timestamp keyed by `(pid_tgid << 1 | syscall_id)`.
/// Using a composite key allows concurrent recvfrom + sendto on the same task.
#[map]
static mut SYSCALL_START: HashMap<u64, u64> =
    HashMap::with_max_entries(16384, 0);

/// Ring buffer pushed to userspace latency reader.
#[map]
pub static mut SYSCALL_LATENCY_EVENTS: RingBuf =
    RingBuf::with_byte_size(4096 * 128, 0);

// -- helpers --

#[inline(always)]
fn syscall_key(pid_tgid: u64, syscall_id: u8) -> u64 {
    (pid_tgid << 1) | (syscall_id as u64)
}

#[inline(always)]
fn record_entry(syscall_id: u8) {
    let id = unsafe { bpf_get_current_pid_tgid() };
    let ts = unsafe { bpf_ktime_get_ns() };
    let key = syscall_key(id, syscall_id);
    let _ = unsafe { SYSCALL_START.insert(&key, &ts, 0) };
}

#[inline(always)]
fn record_exit(ctx: &TracePointContext, syscall_id: u8) {
    let id = unsafe { bpf_get_current_pid_tgid() };
    let pid = (id >> 32) as u32;
    let key = syscall_key(id, syscall_id);

    let start_ts = match unsafe { SYSCALL_START.get(&key).copied() } {
        Some(ts) => ts,
        None => return,
    };
    unsafe { SYSCALL_START.remove(&key).ok() };

    let now = unsafe { bpf_ktime_get_ns() };
    let latency_ns = now.saturating_sub(start_ts);

    // Argument 0 of sys_enter_recvfrom / sys_exit_sendto is the sockfd.
    // TracePointContext::read_at reads from the raw tracepoint args array.
    let sockfd: u32 = unsafe { ctx.read_at::<u32>(16).unwrap_or(0) };

    let event = SyscallLatencyEvent {
        latency_ns,
        timestamp_ns: now,
        pid,
        sockfd,
        syscall_id,
        _pad: [0u8; 7],
    };

    if let Some(mut entry) = unsafe { SYSCALL_LATENCY_EVENTS.reserve::<SyscallLatencyEvent>(0) } {
        entry.write(event);
        entry.submit(0);
    }
}

// -- tracepoint programs --

/// sys_enter_recvfrom: record entry timestamp.
#[tracepoint]
pub fn sys_enter_recvfrom(_ctx: TracePointContext) -> u32 {
    record_entry(SYSCALL_RECVFROM);
    0
}

/// sys_exit_recvfrom: compute and emit latency.
#[tracepoint]
pub fn sys_exit_recvfrom(ctx: TracePointContext) -> u32 {
    record_exit(&ctx, SYSCALL_RECVFROM);
    0
}

/// sys_enter_sendto: record entry timestamp.
#[tracepoint]
pub fn sys_enter_sendto(_ctx: TracePointContext) -> u32 {
    record_entry(SYSCALL_SENDTO);
    0
}

/// sys_exit_sendto: compute and emit latency.
#[tracepoint]
pub fn sys_exit_sendto(ctx: TracePointContext) -> u32 {
    record_exit(&ctx, SYSCALL_SENDTO);
    0
}
