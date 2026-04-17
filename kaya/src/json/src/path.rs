//! JSONPath wrapper with a compiled-path cache (LRU, bounded at 256 entries).
//!
//! Parsing a JSONPath expression has non-trivial cost. This module caches up
//! to 256 recently used compiled expressions so that repeated invocations of
//! the same path string (common in hot loops) pay only a hash-map lookup.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use jsonpath_rust::JsonPath as RawJsonPath;
use serde_json::Value;
use tracing::debug;

use crate::error::JsonError;

// ---------------------------------------------------------------------------
// LRU cache of compiled paths
// ---------------------------------------------------------------------------

const CACHE_CAP: usize = 256;

/// A simple bounded LRU cache: stores (compiled_path, insertion_order) pairs.
/// When full, evicts the entry with the smallest insertion counter.
struct Lru {
    map: HashMap<String, (Arc<RawJsonPath<Value>>, u64)>,
    clock: u64,
}

impl Lru {
    fn new() -> Self {
        Self {
            map: HashMap::with_capacity(CACHE_CAP + 1),
            clock: 0,
        }
    }

    fn get_or_insert(
        &mut self,
        path_str: &str,
    ) -> Result<Arc<RawJsonPath<Value>>, JsonError> {
        if let Some((compiled, ts)) = self.map.get_mut(path_str) {
            self.clock += 1;
            *ts = self.clock;
            return Ok(Arc::clone(compiled));
        }

        // Parse the path expression.
        let compiled = RawJsonPath::try_from(path_str).map_err(|e| JsonError::InvalidPath {
            path: path_str.to_string(),
            reason: e.to_string(),
        })?;
        let compiled = Arc::new(compiled);

        // Evict LRU entry when at capacity.
        if self.map.len() >= CACHE_CAP {
            if let Some(key) = self
                .map
                .iter()
                .min_by_key(|(_, (_, ts))| *ts)
                .map(|(k, _): (&String, _)| k.clone())
            {
                self.map.remove(&key);
                debug!(evicted = %key, "jsonpath lru cache eviction");
            }
        }

        self.clock += 1;
        self.map
            .insert(path_str.to_string(), (Arc::clone(&compiled), self.clock));
        Ok(compiled)
    }
}

// ---------------------------------------------------------------------------
// Public facade
// ---------------------------------------------------------------------------

/// Thread-safe JSONPath cache and query executor.
///
/// Named `JsonPath` at the crate level (re-exported from `kaya_json::JsonPath`).
/// Internally uses `jsonpath_rust::JsonPath` aliased as `RawJsonPath` to avoid
/// the name collision.
pub struct JsonPath(Mutex<Lru>);

impl JsonPath {
    /// Create a new cache.
    pub fn new() -> Self {
        Self(Mutex::new(Lru::new()))
    }

    /// Compile (or fetch from cache) a JSONPath expression.
    pub fn compile(&self, path_str: &str) -> Result<Arc<RawJsonPath<Value>>, JsonError> {
        self.0
            .lock()
            .expect("jsonpath lru mutex poisoned")
            .get_or_insert(path_str)
    }

    /// Execute a JSONPath query and return cloned matches as a `Value::Array`.
    pub fn find(&self, root: &Value, path_str: &str) -> Result<Value, JsonError> {
        let compiled = self.compile(path_str)?;
        Ok(compiled.find(root))
    }

    /// Execute a JSONPath query and return paths of matches as a `Vec<String>`.
    pub fn find_paths(&self, root: &Value, path_str: &str) -> Result<Vec<String>, JsonError> {
        let compiled = self.compile(path_str)?;
        let slices = compiled.find_slice(root);
        let paths = slices
            .into_iter()
            .filter_map(|v| v.to_path())
            .collect();
        Ok(paths)
    }
}

impl Default for JsonPath {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Global singleton (cheap to share via Arc)
// ---------------------------------------------------------------------------

use std::sync::OnceLock;
static GLOBAL_CACHE: OnceLock<Arc<JsonPath>> = OnceLock::new();

/// Return the global, lazily-initialised JSONPath cache.
pub fn global_cache() -> Arc<JsonPath> {
    Arc::clone(GLOBAL_CACHE.get_or_init(|| Arc::new(JsonPath::new())))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn basic_find() {
        let cache = JsonPath::new();
        let root = json!({"a": {"b": 42}});
        let result = cache.find(&root, "$.a.b").unwrap();
        assert_eq!(result, json!([42]));
    }

    #[test]
    fn wildcard_find() {
        let cache = JsonPath::new();
        let root = json!({"items": [{"name": "x"}, {"name": "y"}]});
        let result = cache.find(&root, "$.items[*].name").unwrap();
        assert_eq!(result, json!(["x", "y"]));
    }

    #[test]
    fn invalid_path() {
        let cache = JsonPath::new();
        let root = json!({});
        let err = cache.find(&root, "!!!invalid").unwrap_err();
        assert!(matches!(err, JsonError::InvalidPath { .. }));
    }

    #[test]
    fn cache_hit() {
        let cache = JsonPath::new();
        let root = json!({"n": 1});
        // First call compiles, second should be a cache hit.
        let r1 = cache.find(&root, "$.n").unwrap();
        let r2 = cache.find(&root, "$.n").unwrap();
        assert_eq!(r1, r2);
    }
}
