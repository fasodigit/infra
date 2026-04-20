// SPDX-License-Identifier: AGPL-3.0-or-later
//! Protocol-specific sub-features of the Pingora gateway.
//!
//! **M0 scaffolding**: every sub-module is an empty stub.  Implementations
//! land in gate M4:
//!
//! | Sub-module        | Replaces (hyper path)           |
//! |-------------------|---------------------------------|
//! | `grpc_web`        | `src/grpc_web.rs`               |
//! | `websocket`       | `src/websocket.rs`              |
//! | `compression`     | `src/compression.rs`            |
//! | `traffic_split`   | `src/traffic_split.rs`          |

pub mod compression;
pub mod grpc_web;
pub mod traffic_split;
pub mod websocket;
