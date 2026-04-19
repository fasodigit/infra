// SPDX-License-Identifier: AGPL-3.0-or-later
//! Error types for the eBPF observability loader.

use thiserror::Error;

/// Errors that can occur while loading, attaching, or reading eBPF programs.
#[derive(Debug, Error)]
pub enum EbpfError {
    /// The running kernel version is below the required minimum (5.15).
    #[error("kernel too old for eBPF observability: {0}")]
    KernelTooOld(String),

    /// Required capability (`CAP_SYS_ADMIN` or `CAP_BPF`) is absent.
    #[error("insufficient capabilities for eBPF: {0}")]
    InsufficientCapabilities(String),

    /// An eBPF program could not be found in the object file.
    #[error("eBPF program not found: {0}")]
    ProgramNotFound(String),

    /// An eBPF map could not be found in the object file.
    #[error("eBPF map not found: {0}")]
    MapNotFound(String),

    /// Failed to load an eBPF program into the kernel verifier.
    #[error("eBPF load error: {0}")]
    Load(String),

    /// Failed to attach an eBPF program to its hook point.
    #[error("eBPF attach error: {0}")]
    Attach(String),

    /// Ring buffer or map access error.
    #[error("eBPF map operation error: {0}")]
    Map(String),

    /// I/O error reading `/proc/version` or similar.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic parse error.
    #[error("parse error: {0}")]
    Parse(String),
}
