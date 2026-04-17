//! `JsonStore`: in-memory storage for JSON documents.
//!
//! Each entry is a `DashMap<Vec<u8>, Arc<RwLock<JsonDocument>>>` so concurrent
//! readers never block each other; writes take a brief exclusive lock on the
//! individual document, not the whole map.

use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;
use serde_json::{json, Map, Value};
use tracing::{debug, instrument};

use crate::document::JsonDocument;
use crate::error::JsonError;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for [`JsonStore::set`].
#[derive(Debug, Default, Clone)]
pub struct JsonSetOpts {
    /// Only set if the key **does not** exist.
    pub nx: bool,
    /// Only set if the key **already** exists.
    pub xx: bool,
}

// ---------------------------------------------------------------------------
// JsonStore
// ---------------------------------------------------------------------------

/// In-memory store for JSON documents.
///
/// Keys are `Vec<u8>` (binary-safe).  Documents are stored behind
/// `Arc<RwLock<JsonDocument>>` so callers can cheaply clone the `Arc` without
/// copying the underlying JSON tree.
#[derive(Debug, Default)]
pub struct JsonStore {
    docs: DashMap<Vec<u8>, Arc<RwLock<JsonDocument>>>,
}

impl JsonStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            docs: DashMap::new(),
        }
    }

    // -- internal helpers ----------------------------------------------------

    /// Clone the `Arc` for a key, releasing the DashMap shard lock immediately.
    fn get_arc(&self, key: &[u8]) -> Option<Arc<RwLock<JsonDocument>>> {
        self.docs.get(key).map(|e| Arc::clone(e.value()))
    }

    /// Clone the `Arc` or return `KeyNotFound`.
    fn require_arc(&self, key: &[u8]) -> Result<Arc<RwLock<JsonDocument>>, JsonError> {
        self.get_arc(key)
            .ok_or_else(|| JsonError::KeyNotFound(String::from_utf8_lossy(key).into_owned()))
    }

    // -- core write operations -----------------------------------------------

    /// JSON.SET – store `value` at `path` inside the document at `key`.
    ///
    /// `path = "$"` replaces the whole document.
    #[instrument(skip(self, value, opts), fields(path = path))]
    pub fn set(
        &self,
        key: &[u8],
        path: &str,
        value: Value,
        opts: JsonSetOpts,
    ) -> Result<(), JsonError> {
        let key_vec = key.to_vec();
        let exists = self.docs.contains_key(key);

        if opts.nx && exists {
            return Err(JsonError::NxConditionFailed);
        }
        if opts.xx && !exists {
            return Err(JsonError::XxConditionFailed);
        }

        if !exists {
            let doc = if path == "$" {
                JsonDocument::new(value)
            } else {
                let mut d = JsonDocument::new(json!({}));
                d.set(path, value)?;
                d
            };
            self.docs.insert(key_vec, Arc::new(RwLock::new(doc)));
            debug!("json.set: inserted new document");
            return Ok(());
        }

        // Key exists – clone Arc then release DashMap reference before locking.
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        let result = guard.set(path, value);
        result
    }

    /// JSON.GET – retrieve one or more paths from the document at `key`.
    ///
    /// * Single path: returns the matched value(s) as a JSON array.
    /// * Multiple paths: returns a JSON object mapping each path to its matches.
    /// * Returns `Err(JsonError::KeyNotFound)` if the key does not exist.
    #[instrument(skip(self), fields(key_len = key.len()))]
    pub fn get(&self, key: &[u8], paths: &[&str]) -> Result<Value, JsonError> {
        let arc = self.require_arc(key)?;
        let guard = arc.read();

        if paths.is_empty() || (paths.len() == 1 && paths[0] == "$") {
            return Ok(guard.root().clone());
        }

        if paths.len() == 1 {
            let results = guard.query(paths[0])?;
            return Ok(Value::Array(results));
        }

        let mut map = Map::new();
        for &p in paths {
            let results = guard.query(p)?;
            map.insert(p.to_string(), Value::Array(results));
        }
        Ok(Value::Object(map))
    }

    /// JSON.DEL – delete `path` from the document at `key`.
    ///
    /// Returns the count of deleted values.  If `key` does not exist, returns 0.
    pub fn del(&self, key: &[u8], path: &str) -> u64 {
        let key_vec = key.to_vec();
        let arc = match self.get_arc(key) {
            Some(a) => a,
            None => return 0,
        };
        let mut guard = arc.write();
        let n = guard.delete(path).unwrap_or(0);
        if path == "$" {
            drop(guard);
            self.docs.remove(&key_vec);
        }
        n as u64
    }

    /// JSON.TYPE – return the JSON type name at `path`.
    pub fn type_at(&self, key: &[u8], path: &str) -> Option<&'static str> {
        let arc = self.get_arc(key)?;
        let guard = arc.read();
        let result = guard.type_at(path);
        result
    }

    // -- array operations ----------------------------------------------------

    /// JSON.ARRAPPEND – append values to the array at `path`.
    pub fn arr_append(&self, key: &[u8], path: &str, values: Vec<Value>) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.arr_append(path, values)
    }

    /// JSON.ARRLEN – length of the array at `path`.
    pub fn arr_len(&self, key: &[u8], path: &str) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let guard = arc.read();
        guard.arr_len(path)
    }

    /// JSON.ARRPOP – pop the last element of the array at `path`.
    pub fn arr_pop(&self, key: &[u8], path: &str) -> Result<Value, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.arr_pop(path)
    }

    /// JSON.ARRINDEX – return the index of `scalar` in the array at `path`.
    pub fn arr_index(&self, key: &[u8], path: &str, scalar: &Value) -> Result<i64, JsonError> {
        let arc = self.require_arc(key)?;
        let guard = arc.read();
        guard.arr_index(path, scalar)
    }

    /// JSON.ARRINSERT – insert values into the array at `path` before `index`.
    pub fn arr_insert(
        &self,
        key: &[u8],
        path: &str,
        index: i64,
        values: Vec<Value>,
    ) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.arr_insert(path, index, values)
    }

    /// JSON.ARRTRIM – trim the array at `path` to range `[start, stop]`.
    pub fn arr_trim(&self, key: &[u8], path: &str, start: i64, stop: i64) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.arr_trim(path, start, stop)
    }

    // -- object operations ---------------------------------------------------

    /// JSON.OBJKEYS – return the keys of the object at `path`.
    pub fn obj_keys(&self, key: &[u8], path: &str) -> Result<Vec<String>, JsonError> {
        let arc = self.require_arc(key)?;
        let guard = arc.read();
        guard.obj_keys(path)
    }

    /// JSON.OBJLEN – return the number of keys in the object at `path`.
    pub fn obj_len(&self, key: &[u8], path: &str) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let guard = arc.read();
        guard.obj_len(path)
    }

    // -- numeric operations --------------------------------------------------

    /// JSON.NUMINCRBY – add `delta` to the number at `path`.
    pub fn num_incrby(&self, key: &[u8], path: &str, delta: f64) -> Result<Value, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.num_incrby(path, delta)
    }

    /// JSON.NUMMULTBY – multiply the number at `path` by `factor`.
    pub fn num_multby(&self, key: &[u8], path: &str, factor: f64) -> Result<Value, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.num_multby(path, factor)
    }

    // -- string operations ---------------------------------------------------

    /// JSON.STRAPPEND – append suffix to the string at `path`.
    pub fn str_append(&self, key: &[u8], path: &str, suffix: &str) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.str_append(path, suffix)
    }

    /// JSON.STRLEN – return the length of the string at `path`.
    pub fn str_len(&self, key: &[u8], path: &str) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let guard = arc.read();
        guard.str_len(path)
    }

    // -- boolean toggle ------------------------------------------------------

    /// JSON.TOGGLE – flip the boolean at `path`.
    pub fn toggle(&self, key: &[u8], path: &str) -> Result<bool, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();
        guard.bool_toggle(path)
    }

    // -- clear ---------------------------------------------------------------

    /// JSON.CLEAR – reset the value at `path` to `{}` (object) or `[]` (array),
    /// and numeric values to `0`.
    pub fn clear(&self, key: &[u8], path: &str) -> Result<usize, JsonError> {
        let arc = self.require_arc(key)?;
        let mut guard = arc.write();

        if path == "$" {
            let cleared =
                matches!(guard.root(), Value::Object(_) | Value::Array(_) | Value::Number(_));
            guard.set("$", json!({}))?;
            return Ok(if cleared { 1 } else { 0 });
        }

        use crate::document::jsonpath_to_pointer;
        let ptr = jsonpath_to_pointer(path)?;
        let target = guard
            .root_mut()
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path.to_string()))?;

        let was_clearable =
            matches!(target, Value::Object(_) | Value::Array(_) | Value::Number(_));
        if was_clearable {
            *target = match target {
                Value::Object(_) => json!({}),
                Value::Array(_) => json!([]),
                Value::Number(_) => json!(0),
                _ => unreachable!(),
            };
        }
        Ok(if was_clearable { 1 } else { 0 })
    }

    // -- multi-key -----------------------------------------------------------

    /// JSON.MGET – retrieve the value at `path` from multiple keys at once.
    ///
    /// Returns a `Vec` aligned with `keys`; missing/erroring entries yield `None`.
    pub fn mget(&self, keys: &[&[u8]], path: &str) -> Vec<Option<Value>> {
        keys.iter()
            .map(|&k| self.get(k, &[path]).ok())
            .collect()
    }

    // -- debug ---------------------------------------------------------------

    /// JSON.DEBUG MEMORY – return the serialized byte size of a document.
    pub fn resp_debug_memory(&self, key: &[u8]) -> Option<usize> {
        let arc = self.get_arc(key)?;
        let guard = arc.read();
        Some(guard.size())
    }

    /// Return a clone of the document `Arc` (for advanced use).
    pub fn get_raw(&self, key: &[u8]) -> Option<Arc<RwLock<JsonDocument>>> {
        self.get_arc(key)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store_with_doc(key: &str, val: Value) -> JsonStore {
        let s = JsonStore::new();
        s.set(key.as_bytes(), "$", val, JsonSetOpts::default())
            .unwrap();
        s
    }

    // -- JSON.SET / JSON.GET basics -------------------------------------------

    #[test]
    fn set_and_get_root() {
        let store = store_with_doc("k", json!({"a": 1}));
        let v = store.get(b"k", &["$"]).unwrap();
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn set_nested_path() {
        let store = store_with_doc("k", json!({"a": {"b": 1}}));
        store
            .set(b"k", "$.a.b", json!(42), JsonSetOpts::default())
            .unwrap();
        let v = store.get(b"k", &["$.a.b"]).unwrap();
        assert_eq!(v, json!([42]));
    }

    #[test]
    fn get_multi_path() {
        let store = store_with_doc("k", json!({"x": 1, "y": 2}));
        let v = store.get(b"k", &["$.x", "$.y"]).unwrap();
        assert!(v.is_object());
        let obj = v.as_object().unwrap();
        assert_eq!(obj["$.x"], json!([1]));
        assert_eq!(obj["$.y"], json!([2]));
    }

    // -- NX / XX conditions --------------------------------------------------

    #[test]
    fn nx_fails_when_key_exists() {
        let store = store_with_doc("k", json!(1));
        let err = store
            .set(b"k", "$", json!(2), JsonSetOpts { nx: true, xx: false })
            .unwrap_err();
        assert!(matches!(err, JsonError::NxConditionFailed));
    }

    #[test]
    fn xx_fails_when_key_missing() {
        let store = JsonStore::new();
        let err = store
            .set(b"k", "$", json!(1), JsonSetOpts { nx: false, xx: true })
            .unwrap_err();
        assert!(matches!(err, JsonError::XxConditionFailed));
    }

    // -- JSON.DEL ------------------------------------------------------------

    #[test]
    fn del_existing_key() {
        let store = store_with_doc("k", json!({"a": 1}));
        let n = store.del(b"k", "$");
        assert_eq!(n, 1);
        assert!(store.get(b"k", &["$"]).is_err());
    }

    // -- JSON.TYPE -----------------------------------------------------------

    #[test]
    fn type_at_returns_correct_types() {
        let store = store_with_doc("k", json!({"s": "hi", "n": 5, "arr": []}));
        assert_eq!(store.type_at(b"k", "$.s"), Some("string"));
        assert_eq!(store.type_at(b"k", "$.n"), Some("integer"));
        assert_eq!(store.type_at(b"k", "$.arr"), Some("array"));
    }

    // -- Array operations ----------------------------------------------------

    #[test]
    fn arr_append_and_len() {
        let store = store_with_doc("k", json!({"arr": [1]}));
        let len = store
            .arr_append(b"k", "$.arr", vec![json!(2), json!(3)])
            .unwrap();
        assert_eq!(len, 3);
        assert_eq!(store.arr_len(b"k", "$.arr").unwrap(), 3);
    }

    #[test]
    fn arr_pop_returns_last() {
        let store = store_with_doc("k", json!({"arr": [10, 20, 30]}));
        let v = store.arr_pop(b"k", "$.arr").unwrap();
        assert_eq!(v, json!(30));
    }

    #[test]
    fn arr_index_found_and_missing() {
        let store = store_with_doc("k", json!({"arr": ["a", "b", "c"]}));
        assert_eq!(store.arr_index(b"k", "$.arr", &json!("b")).unwrap(), 1);
        assert_eq!(store.arr_index(b"k", "$.arr", &json!("z")).unwrap(), -1);
    }

    // -- Numeric operations --------------------------------------------------

    #[test]
    fn num_incrby_integer_and_float() {
        let store = store_with_doc("k", json!({"n": 10}));
        let v = store.num_incrby(b"k", "$.n", 5.0).unwrap();
        assert_eq!(v, json!(15));
        let v2 = store.num_incrby(b"k", "$.n", 0.5).unwrap();
        assert_eq!(v2, json!(15.5));
    }

    #[test]
    fn num_multby() {
        let store = store_with_doc("k", json!({"n": 3}));
        let v = store.num_multby(b"k", "$.n", 3.0).unwrap();
        assert_eq!(v, json!(9));
    }

    // -- String operations ---------------------------------------------------

    #[test]
    fn str_append_and_len() {
        let store = store_with_doc("k", json!({"s": "hello"}));
        let new_len = store.str_append(b"k", "$.s", " world").unwrap();
        assert_eq!(new_len, 11);
        assert_eq!(store.str_len(b"k", "$.s").unwrap(), 11);
    }

    // -- Object operations ---------------------------------------------------

    #[test]
    fn obj_keys_and_len() {
        let store = store_with_doc("k", json!({"a": 1, "b": 2, "c": 3}));
        let mut keys = store.obj_keys(b"k", "$").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);
        assert_eq!(store.obj_len(b"k", "$").unwrap(), 3);
    }

    // -- Boolean toggle ------------------------------------------------------

    #[test]
    fn toggle_boolean() {
        let store = store_with_doc("k", json!({"f": true}));
        let new_val = store.toggle(b"k", "$.f").unwrap();
        assert!(!new_val);
        let v = store.get(b"k", &["$.f"]).unwrap();
        assert_eq!(v, json!([false]));
    }

    // -- Clear ---------------------------------------------------------------

    #[test]
    fn clear_object() {
        let store = store_with_doc("k", json!({"a": 1, "b": 2}));
        let n = store.clear(b"k", "$").unwrap();
        assert_eq!(n, 1);
        let v = store.get(b"k", &["$"]).unwrap();
        assert_eq!(v, json!({}));
    }

    // -- MGET multi-key ------------------------------------------------------

    #[test]
    fn mget_multi_key() {
        let store = JsonStore::new();
        store
            .set(b"k1", "$", json!({"n": 1}), JsonSetOpts::default())
            .unwrap();
        store
            .set(b"k2", "$", json!({"n": 2}), JsonSetOpts::default())
            .unwrap();
        let results = store.mget(
            &[b"k1".as_ref(), b"k2".as_ref(), b"missing".as_ref()],
            "$.n",
        );
        assert_eq!(results.len(), 3);
        assert!(results[0].is_some());
        assert!(results[1].is_some());
        assert!(results[2].is_none());
    }

    // -- Debug memory --------------------------------------------------------

    #[test]
    fn debug_memory() {
        let store = store_with_doc("k", json!({"hello": "world"}));
        let size = store.resp_debug_memory(b"k").unwrap();
        assert!(size > 0);
    }

    // -- Key not found error --------------------------------------------------

    #[test]
    fn get_missing_key_returns_error() {
        let store = JsonStore::new();
        let err = store.get(b"missing", &["$"]).unwrap_err();
        assert!(matches!(err, JsonError::KeyNotFound(_)));
    }

    // -- Nested modification -------------------------------------------------

    #[test]
    fn nested_modification() {
        let store = store_with_doc("k", json!({"a": {"b": {"c": 1}}}));
        store
            .set(b"k", "$.a.b.c", json!(999), JsonSetOpts::default())
            .unwrap();
        let v = store.get(b"k", &["$.a.b.c"]).unwrap();
        assert_eq!(v, json!([999]));
    }
}
