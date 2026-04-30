// SPDX-License-Identifier: AGPL-3.0-or-later
//! WebSocket sub-system for terroir-mobile-bff.
//!
//! - `registry`  : `WsRegistry` indexed by tenant slug + user id.
//! - `handler`   : Axum WebSocket handler `/ws/sync/{producerId}`.

pub mod handler;
pub mod registry;

pub use handler::ws_sync_handler;
pub use registry::WsRegistry;
