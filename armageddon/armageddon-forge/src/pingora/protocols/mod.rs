// SPDX-License-Identifier: AGPL-3.0-or-later
//! Protocol-specific sub-features of the Pingora gateway.
//!
//! **M0 scaffolding** — sub-modules are filled progressively during M4:
//!
//! | Sub-module        | Replaces (hyper path)           | Status |
//! |-------------------|---------------------------------|--------|
//! | `grpc_web`        | `src/grpc_web.rs`               | stub   |
//! | `websocket`       | `src/websocket.rs`              | stub   |
//! | `compression`     | `src/compression.rs`            | M4 ✓   |
//! | `traffic_split`   | `src/traffic_split.rs`          | stub   |

pub mod compression;
pub mod grpc_web;
pub mod traffic_split;
pub mod websocket;

pub use compression::{
    CompressionFilter, CompressionLevel, CompressionStream, Encoding, NegotiationOutcome,
};
