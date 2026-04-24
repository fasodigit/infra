// SPDX-License-Identifier: AGPL-3.0-or-later
//! eBPF observability for ARMAGEDDON (Linux, feature-gated).
//!
//! # Overview
//!
//! This crate provides kernel-side L7 observability by attaching eBPF programs
//! to TCP connection hooks and syscall tracepoints. Events flow from the kernel
//! via ring buffers to Tokio tasks that update Prometheus metrics.
//!
//! # Feature gate
//!
//! All eBPF functionality is behind the `ebpf` Cargo feature, which is **off by
//! default**. Without that feature, `cargo check` succeeds on any host. With it,
//! a Linux kernel >= 5.15 and `CAP_SYS_ADMIN` (or `CAP_BPF`) are required at
//! runtime; if either condition is absent the loader falls back gracefully with a
//! warning rather than crashing the process.
//!
//! # Usage
//!
//! ```rust,no_run
//! # #[cfg(all(target_os = "linux", feature = "ebpf"))]
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use armageddon_ebpf::EbpfObservability;
//! let obs = EbpfObservability::attach().await?;
//! // obs stays alive to keep programs attached; drop to detach.
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod metrics;
pub mod programs;

pub use error::EbpfError;

// -- kernel version check (Linux only) --

#[cfg(target_os = "linux")]
#[allow(dead_code)] // used only under cfg(feature = "ebpf")
mod kernel_check {
    use super::EbpfError;
    use std::fs;

    /// Minimum supported kernel: 5.15 (for stable ring-buffer + kprobe API).
    const MIN_MAJOR: u32 = 5;
    const MIN_MINOR: u32 = 15;

    /// Reads `/proc/version_signature` or `/proc/version` and extracts `(major, minor)`.
    pub fn check_kernel_version() -> Result<(), EbpfError> {
        let content = fs::read_to_string("/proc/version")
            .map_err(EbpfError::Io)?;
        // Format: "Linux version 5.15.0-92-generic (...)"
        let version_str = content
            .split_whitespace()
            .nth(2)
            .ok_or_else(|| EbpfError::Parse("malformed /proc/version".into()))?;
        let mut parts = version_str.splitn(3, '.');
        let major: u32 = parts
            .next()
            .unwrap_or("0")
            .parse()
            .map_err(|_| EbpfError::Parse("bad major version".into()))?;
        let minor: u32 = parts
            .next()
            .unwrap_or("0")
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .map_err(|_| EbpfError::Parse("bad minor version".into()))?;

        if (major, minor) < (MIN_MAJOR, MIN_MINOR) {
            return Err(EbpfError::KernelTooOld(format!(
                "found {major}.{minor}, need >= {MIN_MAJOR}.{MIN_MINOR}"
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public handle — non-Linux stub
// ---------------------------------------------------------------------------

/// Handle that keeps attached eBPF programs alive for the process lifetime.
///
/// Dropping this value detaches all programs and stops the reader tasks.
/// On non-Linux platforms or without the `ebpf` feature this is a zero-size
/// stub with no-op methods.
#[derive(Debug)]
pub struct EbpfObservability {
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    _inner: linux::Inner,
}

impl EbpfObservability {
    /// Attach eBPF programs and start background reader tasks.
    ///
    /// Returns `Ok(handle)` on success. On kernels < 5.15 or with insufficient
    /// capabilities, logs a warning and returns `Ok(stub)` — the process
    /// continues without eBPF metrics rather than failing.
    pub async fn attach() -> Result<Self, EbpfError> {
        #[cfg(all(target_os = "linux", feature = "ebpf"))]
        {
            linux::attach_inner().await
        }
        #[cfg(not(all(target_os = "linux", feature = "ebpf")))]
        {
            tracing::warn!(
                "armageddon-ebpf: eBPF support not compiled in (feature 'ebpf' absent or non-Linux). Skipping."
            );
            Ok(Self {})
        }
    }

    /// Test-only constructor that forces the graceful-fallback (stub) code
    /// path without performing any kernel-version or capability probe.
    ///
    /// Used by the regression test that guards against the previous
    /// `unsafe { std::mem::zeroed() }` UB: dropping the returned handle
    /// must never leak, panic, or call `close(0)` (stdin).
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    #[doc(hidden)]
    pub fn stub_for_test() -> Self {
        Self {
            _inner: linux::stub_inner_for_test(),
        }
    }
}

// ---------------------------------------------------------------------------
// Linux + feature "ebpf" implementation
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "linux", feature = "ebpf"))]
mod linux {
    use std::{net::Ipv4Addr, time::Duration};

    use aya::{maps::RingBuf, Ebpf};
    use tokio::{task::JoinHandle, time::sleep};
    use tracing::{debug, error, info, instrument, warn};

    use armageddon_ebpf_programs::events::{SyscallLatencyEvent, TcpConnectEvent};

    use crate::{
        error::EbpfError,
        metrics::{
            record_syscall_latency, record_tcp_connection, METRICS,
        },
        programs::{syscall_latency, tcp_connect},
        EbpfObservability,
    };

    /// Internal state — keeps Ebpf handle + task handles alive.
    ///
    /// `_ebpf` is `Option<Ebpf>` so the graceful-fallback path (kernel < 5.15
    /// or missing `CAP_BPF`) can construct a well-formed stub without ever
    /// building an `Ebpf` from invalid memory. Only the Active path sets
    /// `Some(_)`; the stub path leaves it `None`.
    #[derive(Debug)]
    pub(crate) struct Inner {
        // The Ebpf handle owns the fd; drop detaches everything.
        _ebpf: Option<Ebpf>,
        _tcp_task: JoinHandle<()>,
        _syscall_task: JoinHandle<()>,
    }

    /// Embedded ELF BPF object produced by `aya-build` in build.rs.
    ///
    /// The file is generated to `OUT_DIR/armageddon-ebpf-programs` and
    /// embedded at compile time. Name matches the package name of the programs crate.
    static BPF_ELF: &[u8] = include_bytes!(concat!(
        env!("OUT_DIR"),
        "/armageddon-ebpf-programs"
    ));

    /// Main attachment entry point.
    #[instrument(err)]
    pub(crate) async fn attach_inner() -> Result<EbpfObservability, EbpfError> {
        // -- graceful fallback: kernel version --
        if let Err(e) = crate::kernel_check::check_kernel_version() {
            warn!("eBPF skipped: {e}");
            return Ok(EbpfObservability { _inner: stub_inner() });
        }

        // -- graceful fallback: capabilities --
        if !has_bpf_capability() {
            warn!("eBPF skipped: process lacks CAP_SYS_ADMIN / CAP_BPF");
            return Ok(EbpfObservability { _inner: stub_inner() });
        }

        // -- load ELF --
        let mut ebpf = Ebpf::load(BPF_ELF)
            .map_err(|e| EbpfError::Load(format!("{e}")))?;

        // -- enable aya-log kernel-side debug messages --
        if let Err(e) = aya_log::EbpfLogger::init(&mut ebpf) {
            warn!("aya-log init failed (non-fatal): {e}");
        }

        // -- attach tcp_connect programs --
        let tcp_ring = tcp_connect::linux::attach(&mut ebpf)?;

        // -- attach syscall latency tracepoints --
        let syscall_ring = syscall_latency::linux::attach(&mut ebpf)?;

        // -- register Prometheus metrics --
        METRICS.get_or_init(crate::metrics::Metrics::new);

        // -- spawn ring buffer reader tasks --
        let tcp_task = tokio::spawn(tcp_ring_reader(tcp_ring));
        let syscall_task = tokio::spawn(syscall_ring_reader(syscall_ring));

        info!("armageddon-ebpf: programs attached, readers running");

        Ok(EbpfObservability {
            _inner: Inner {
                _ebpf: Some(ebpf),
                _tcp_task: tcp_task,
                _syscall_task: syscall_task,
            },
        })
    }

    /// Fallback stub when graceful skip is triggered after capability/version check.
    ///
    /// No `Ebpf` handle is constructed — the previous implementation called
    /// `aya::Ebpf::load(&[])` (which always fails, empty slice has no ELF
    /// header) and then fell back to `unsafe { std::mem::zeroed() }`, which
    /// is immediate UB: `Ebpf` owns `HashMap`s, `OwnedFd`s and `Option<Btf>`
    /// that have library-level validity invariants, and on `Drop` a zeroed
    /// `OwnedFd` would `close(0)` (stdin). We now simply leave `_ebpf: None`.
    fn stub_inner() -> Inner {
        // Spawn no-op tasks so the struct is always well-formed and the
        // `JoinHandle` fields are never uninitialised.
        let t1 = tokio::spawn(async {});
        let t2 = tokio::spawn(async {});
        Inner {
            _ebpf: None,
            _tcp_task: t1,
            _syscall_task: t2,
        }
    }

    /// Exposes `stub_inner` to the crate-public test helper on
    /// [`super::EbpfObservability::stub_for_test`].
    #[doc(hidden)]
    pub(crate) fn stub_inner_for_test() -> Inner {
        stub_inner()
    }

    /// Check for BPF capability using `/proc/self/status`.
    ///
    /// Checks `CapEff` for bit 21 (CAP_SYS_ADMIN) or bit 39 (CAP_BPF, kernel 5.8+).
    fn has_bpf_capability() -> bool {
        // A simple heuristic: try probing a dummy bpf syscall.
        // aya will report an error with EPERM if capabilities are absent.
        // We use libc directly to avoid pulling in extra deps.
        //
        // Read effective capabilities from /proc/self/status.
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return false;
        };
        for line in status.lines() {
            if let Some(hex) = line.strip_prefix("CapEff:\t") {
                let caps = u64::from_str_radix(hex.trim(), 16).unwrap_or(0);
                let cap_sys_admin: u64 = 1 << 21;
                let cap_bpf: u64 = 1 << 39;
                return (caps & cap_sys_admin) != 0 || (caps & cap_bpf) != 0;
            }
        }
        false
    }

    // -- ring buffer readers --

    /// Continuously drains the TCP connect ring buffer and updates metrics.
    async fn tcp_ring_reader(mut ring: RingBuf<&mut aya::maps::MapData>) {
        loop {
            // Poll with a small sleep to avoid busy-waiting.
            // In production, use tokio::io::unix::AsyncFd for edge-triggered wakeup.
            while let Some(item) = ring.next() {
                if item.len() < std::mem::size_of::<TcpConnectEvent>() {
                    debug!("tcp ring: short read {} bytes", item.len());
                    continue;
                }
                let event: TcpConnectEvent =
                    // SAFETY: BPF guarantees alignment and size via repr(C).
                    unsafe { std::ptr::read_unaligned(item.as_ptr() as *const TcpConnectEvent) };

                let src = Ipv4Addr::from(u32::from_be(event.src_ip));
                let dst = Ipv4Addr::from(u32::from_be(event.dst_ip));
                debug!(
                    src = %src, dst = %dst, port = event.dst_port,
                    latency_ns = event.latency_ns, pid = event.pid,
                    "tcp_connect event"
                );
                record_tcp_connection(event.dst_port);
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    /// Continuously drains the syscall latency ring buffer and updates metrics.
    async fn syscall_ring_reader(mut ring: RingBuf<&mut aya::maps::MapData>) {
        loop {
            while let Some(item) = ring.next() {
                if item.len() < std::mem::size_of::<SyscallLatencyEvent>() {
                    debug!("syscall ring: short read {} bytes", item.len());
                    continue;
                }
                let event: SyscallLatencyEvent =
                    // SAFETY: same as above.
                    unsafe {
                        std::ptr::read_unaligned(
                            item.as_ptr() as *const SyscallLatencyEvent,
                        )
                    };

                let syscall_name = if event.syscall_id == 0 { "recvfrom" } else { "sendto" };
                let latency_secs = event.latency_ns as f64 / 1_000_000_000.0;
                debug!(
                    syscall = syscall_name, latency_ns = event.latency_ns,
                    pid = event.pid, sockfd = event.sockfd,
                    "syscall latency event"
                );
                record_syscall_latency(syscall_name, latency_secs);
            }
            sleep(Duration::from_millis(10)).await;
        }
    }
}
