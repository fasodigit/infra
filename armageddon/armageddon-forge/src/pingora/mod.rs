// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pingora-based proxy backend for ARMAGEDDON-FORGE.
//!
//! This module is the **M0 scaffold** of the ARMAGEDDON → Pingora migration.
//! It sets up the module tree, trait surface and runtime bridge that
//! subsequent gates (M1 – M5) fill with real behaviour.
//!
//! - Tracker: `#108`
//! - Current gate: `#101` (M0)
//! - Design notes: `RUNTIME.md` (next to this file)
//!
//! The hyper 1.x path (`crate::proxy`) remains the default and is
//! **byte-identical** to its pre-M0 form.  Nothing in this module affects
//! the default build.
//!
//! # Feature gate
//!
//! This module is only compiled when the crate feature `pingora` is
//! enabled:
//!
//! ```text
//! cargo build --release --features pingora
//! ```
//!
//! # Module tree
//!
//! ```text
//!   pingora/
//!   ├── ctx                 — RequestCtx (shared per-request state)
//!   ├── gateway             — PingoraGateway + ProxyHttp impl
//!   ├── server              — build_server() bootstrap helper
//!   ├── runtime             — tokio bridge (RUNTIME.md)
//!   ├── filters/            — ForgeFilter trait + stubs (M1)
//!   ├── upstream/           — selector / mtls / lb / retry (M2)
//!   ├── engines/            — SENTINEL / ARBITER / … pipeline (M3)
//!   └── protocols/          — grpc-web / ws / compression (M4)
//! ```

pub mod ctx;
pub mod engines;
pub mod filters;
pub mod gateway;
pub mod protocols;
pub mod runtime;
pub mod server;
pub mod upstream;

// Convenient re-exports so callers can write `pingora::PingoraGateway`
// without digging into `pingora::gateway::…`.
pub use ctx::RequestCtx;
pub use gateway::{PingoraGateway, PingoraGatewayConfig, UpstreamRegistry};
pub use server::build_server;
