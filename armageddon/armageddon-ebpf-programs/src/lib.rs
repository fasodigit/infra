// SPDX-License-Identifier: AGPL-3.0-or-later
//! BPF-side programs for ARMAGEDDON eBPF observability.
//!
//! This crate is intended to be compiled for the `bpfel-unknown-none` target
//! via `aya-build` in `armageddon-ebpf`'s `build.rs`. Each sub-module defines
//! one or more eBPF programs as well as the shared event types that are also
//! used by the userspace loader.
//!
//! Compilation guard: the BPF program bodies are gated behind
//! `#[cfg(feature = "bpf")]` so that `cargo check` on the host target
//! still succeeds (the shared types remain visible).

#![cfg_attr(feature = "bpf", no_std)]

pub mod events;

#[cfg(feature = "bpf")]
pub mod tcp_connect;

#[cfg(feature = "bpf")]
pub mod syscall_latency;
