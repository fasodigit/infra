// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared event types transmitted through eBPF ring buffers to userspace.
//!
//! Both the BPF-side programs and the userspace loader include this module.
//! Types must be `#[repr(C)]` so the kernel and userspace agree on layout.
//! No heap allocation — all fields are fixed-size primitives.

/// Event emitted by the `tcp_connect` kprobe program.
///
/// Records each outbound TCP connection attempt with timing information.
/// IPv4 addresses are stored in network byte order (big-endian).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TcpConnectEvent {
    /// Source IPv4 address (network byte order).
    pub src_ip: u32,
    /// Destination IPv4 address (network byte order).
    pub dst_ip: u32,
    /// Destination port (host byte order).
    pub dst_port: u16,
    /// Padding to align to 8 bytes.
    pub _pad: u16,
    /// Connect latency in nanoseconds (time between kprobe entry and return).
    pub latency_ns: u64,
    /// Kernel monotonic timestamp at event creation (bpf_ktime_get_ns).
    pub timestamp_ns: u64,
    /// PID of the process that triggered the connect.
    pub pid: u32,
    /// Padding to align struct to 8 bytes.
    pub _pad2: u32,
}

/// Event emitted by the `syscall_latency` tracepoint programs.
///
/// Covers `recvfrom` and `sendto` latency per socket file descriptor.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SyscallLatencyEvent {
    /// Syscall latency in nanoseconds.
    pub latency_ns: u64,
    /// Kernel monotonic timestamp at event creation.
    pub timestamp_ns: u64,
    /// PID of the calling process.
    pub pid: u32,
    /// Socket file descriptor number.
    pub sockfd: u32,
    /// Syscall identifier: 0 = recvfrom, 1 = sendto.
    pub syscall_id: u8,
    /// Padding.
    pub _pad: [u8; 7],
}
