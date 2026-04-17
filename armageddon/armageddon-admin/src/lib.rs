// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Admin API for ARMAGEDDON: hot-reload, stats, cluster inspection.
//!
//! Bound to loopback (127.0.0.1:9901) by default. All mutating routes
//! optionally require an `X-Admin-Token` header validated with
//! constant-time comparison (`subtle`).

pub mod config_reload;
pub mod error;
pub mod routes;
pub mod server;
pub mod state;
pub mod stats;

pub use server::{AdminConfig, AdminServer};
pub use state::AdminState;
