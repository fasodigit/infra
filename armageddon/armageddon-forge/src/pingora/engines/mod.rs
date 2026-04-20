// SPDX-License-Identifier: AGPL-3.0-or-later
//! Security-engine integration for the Pingora gateway.
//!
//! Each FASO security engine lives in its own crate (`armageddon-sentinel`,
//! `armageddon-arbiter`, `armageddon-oracle`, `armageddon-aegis`,
//! `armageddon-nexus`, `armageddon-veil`, `armageddon-wasm`,
//! `armageddon-ai`).  Engines are tokio-native, so they execute on the
//! dedicated tokio runtime exposed by [`crate::pingora::runtime`].
//!
//! # M3 first-wave scope (issue #104)
//!
//! * [`pipeline`] orchestrates the fan-out, per-engine timeouts and
//!   score aggregation.
//! * [`aegis_adapter`] is the first **real** adapter wired end-to-end
//!   against [`armageddon_aegis::Aegis`].
//! * The six remaining adapters (SENTINEL, ARBITER, ORACLE, NEXUS, AI,
//!   WASM) ship as type-level stubs returning
//!   [`EngineVerdict::Skipped`]; each carries a `TODO(#104)` marker
//!   pointing at the follow-up port.  VEIL is M1's domain and is
//!   intentionally absent.

pub mod aegis_adapter;
pub mod ai_adapter;
pub mod arbiter_adapter;
pub mod nexus_adapter;
pub mod oracle_adapter;
pub mod pipeline;
pub mod sentinel_adapter;
pub mod wasm_adapter;

pub use aegis_adapter::AegisAdapter;
pub use ai_adapter::AiAdapter;
pub use arbiter_adapter::ArbiterAdapter;
pub use nexus_adapter::NexusAdapter;
pub use oracle_adapter::OracleAdapter;
pub use pipeline::{EngineAdapter, EngineVerdict, Pipeline, PipelineVerdict};
pub use sentinel_adapter::SentinelAdapter;
pub use wasm_adapter::WasmAdapter;
