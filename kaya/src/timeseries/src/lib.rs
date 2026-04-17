//! KAYA TimeSeries: Gorilla-compressed time series storage (RedisTimeSeries parity).
//!
//! ## Architecture
//! - [`chunk`]: Gorilla-compressed storage chunk (up to 256 data points).
//! - [`series`]: Logical time series with multiple chunks, labels, retention, and rules.
//! - [`store`]: Top-level `TimeSeriesStore` backed by `DashMap`.
//! - [`aggregation`]: Aggregation functions for range queries and downsampling.
//! - [`error`]: Error types for the subsystem.

pub mod aggregation;
pub mod chunk;
pub mod error;
pub mod series;
pub mod store;

// Re-exports for ergonomic use by kaya-commands and kaya-server.
pub use aggregation::Aggregator;
pub use chunk::Chunk;
pub use error::{ChunkError, TsError};
pub use series::{CompactionRule, DuplicatePolicy, TimeSeries};
pub use store::{LabelFilter, TimeSeriesStore, TsCreateOpts, TsInfo};
