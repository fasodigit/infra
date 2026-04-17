//! KAYA JSON: native JSON document store with JSONPath query support.
//!
//! Provides RESP3-compatible JSON commands (JSON.SET, JSON.GET, JSON.ARRAPPEND, …)
//! backed by an in-memory `DashMap<Vec<u8>, Arc<RwLock<JsonDocument>>>`.

pub mod document;
pub mod error;
pub mod path;
pub mod store;

pub use document::JsonDocument;
pub use error::JsonError;
pub use path::JsonPath;
pub use store::{JsonSetOpts, JsonStore};
