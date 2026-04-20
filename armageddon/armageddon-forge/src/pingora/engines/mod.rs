// SPDX-License-Identifier: AGPL-3.0-or-later
//! Security-engine integration for the Pingora gateway.
//!
//! Each FASO security engine lives in its own crate (`armageddon-sentinel`,
//! `armageddon-arbiter`, `armageddon-oracle`, `armageddon-aegis`,
//! `armageddon-nexus`, `armageddon-veil`, `armageddon-wasm`,
//! `armageddon-ai`).  Engines are tokio-native, so they execute on the
//! dedicated tokio runtime exposed by [`crate::pingora::runtime`].
//!
//! **M0 scaffolding**: `pipeline.rs` defines the evaluation entry point
//! as a no-op returning `Decision::Continue`.  M3 #104 wires the real
//! fan-out (SENTINEL → ARBITER → ORACLE → AI) and score aggregation into
//! `RequestCtx::{waf_score, ai_score}`.

pub mod pipeline;
