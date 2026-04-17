//! KAYA Tiered Storage
//!
//! Hot RAM / cold NVMe tiered key-value engine.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                         TieredStore                           │
//! │                                                                │
//! │  ┌─────────────────────┐       ┌────────────────────────────┐ │
//! │  │   Hot tier (RAM)    │       │  Cold tier (NVMe / fjall)  │ │
//! │  │  kaya_store::Store  │ ←──── │  FjallBackend / MemBackend │ │
//! │  │  DashMap shards     │ ────► │  LSM-tree, persistent      │ │
//! │  └─────────────────────┘       └────────────────────────────┘ │
//! │           ▲                              ▲                     │
//! │           │  promote (cold → hot)        │ demote (hot → cold) │
//! │           └──────────────────────────────┘                     │
//! │                    TieredMigrator                              │
//! │              (background Tokio task, 1s tick)                  │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Migration policies
//!
//! - **LfuCold**: demote least-frequently-used keys when memory pressure
//!   exceeds `max_hot_mem_bytes` or keys exceed `min_idle_secs`.
//! - **TtlCold**: demote any key not accessed within `hot_ttl_secs`.
//! - **Manual**: no automatic migration; operator-driven via
//!   `KAYA.TIERED.DEMOTE` / `KAYA.TIERED.PROMOTE`.
//!
//! # Key invariants
//!
//! - A key lives in exactly one tier at a time. The location table
//!   (`DashMap<Vec<u8>, AccessRecord>`) is the single source of truth.
//! - Writes always land in the hot tier first (no write-behind to cold).
//! - Corrupt hot reads never fall back silently; errors propagate as
//!   `TieredError`.

pub mod backend;
pub mod error;
pub mod migrator;
pub mod policy;
pub mod store;

pub use backend::{ColdBackend, FjallBackend, MemBackend};
pub use error::TieredError;
pub use migrator::{AccessRecord, Location, TieredMigrator};
pub use policy::MigrationPolicy;
pub use store::{TieredStats, TieredStore};

// Re-export async_trait so consumers can implement ColdBackend without
// a direct dep on the proc-macro crate.
pub use async_trait::async_trait;
