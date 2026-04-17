//! `JsonDocument`: an in-memory JSON document with JSONPath query/mutation support.
//!
//! The document owns a `serde_json::Value` tree and exposes methods that mirror
//! the KAYA JSON command surface (JSON.SET, JSON.GET, JSON.ARRAPPEND, …).
//! All path resolution is delegated to [`crate::path`] so the compiled-path LRU
//! is shared across the entire process.

use serde_json::{json, Value};
use tracing::instrument;

use crate::error::JsonError;
use crate::path::global_cache;

// ---------------------------------------------------------------------------
// JsonDocument
// ---------------------------------------------------------------------------

/// A mutable JSON document stored as a `serde_json::Value` tree.
#[derive(Debug, Clone)]
pub struct JsonDocument {
    root: Value,
}

impl JsonDocument {
    // -- constructors --------------------------------------------------------

    /// Wrap an existing `serde_json::Value`.
    pub fn new(value: Value) -> Self {
        Self { root: value }
    }

    /// Parse a JSON string into a document.
    pub fn from_str(s: &str) -> Result<Self, JsonError> {
        let v: Value = serde_json::from_str(s)
            .map_err(|e| JsonError::ParseError(e.to_string()))?;
        Ok(Self::new(v))
    }

    // -- accessors -----------------------------------------------------------

    /// Reference to the root value.
    pub fn root(&self) -> &Value {
        &self.root
    }

    /// Consume the document and return the underlying `Value`.
    pub fn into_value(self) -> Value {
        self.root
    }

    /// Serialized byte size of the document.
    pub fn size(&self) -> usize {
        // serde_json compact serialization length (no pretty print).
        self.root.to_string().len()
    }

    // -- query (read-only) ---------------------------------------------------

    /// Execute a JSONPath query and return matching values (cloned).
    ///
    /// Returns an empty `Vec` when the path matches nothing (not an error).
    #[instrument(skip(self), fields(path = path_str))]
    pub fn query(&self, path_str: &str) -> Result<Vec<Value>, JsonError> {
        let cache = global_cache();
        let found = cache.find(&self.root, path_str)?;
        match found {
            Value::Array(arr) => Ok(arr),
            Value::Null => Ok(vec![]),
            other => Ok(vec![other]),
        }
    }

    /// Return the JSON type name at the given path (one match, first result).
    /// Returns `None` if the path matches nothing.
    pub fn type_at(&self, path_str: &str) -> Option<&'static str> {
        let cache = global_cache();
        let found = cache.find(&self.root, path_str).ok()?;
        let first = match &found {
            Value::Array(arr) if !arr.is_empty() => &arr[0],
            Value::Null => return None,
            other => other,
        };
        Some(Self::value_type(first))
    }

    // -- mutation (write) ----------------------------------------------------

    /// Set the value at `path_str`.
    ///
    /// When `path_str` is `"$"` the entire root is replaced.
    /// For nested paths, uses JSON pointer semantics after resolving the path.
    #[instrument(skip(self, value), fields(path = path_str))]
    pub fn set(&mut self, path_str: &str, value: Value) -> Result<(), JsonError> {
        if path_str == "$" {
            self.root = value;
            return Ok(());
        }

        let ptr = jsonpath_to_pointer(path_str)?;
        if let Some(target) = self.root.pointer_mut(&ptr) {
            *target = value;
            Ok(())
        } else {
            // Attempt to create intermediate structure for simple dotted paths.
            self.set_creating(path_str, value)
        }
    }

    /// Delete values matching `path_str`. Returns the count of deleted nodes.
    #[instrument(skip(self), fields(path = path_str))]
    pub fn delete(&mut self, path_str: &str) -> Result<usize, JsonError> {
        if path_str == "$" {
            self.root = Value::Null;
            return Ok(1);
        }

        // Resolve paths first, then delete by pointer.
        let cache = global_cache();
        let paths = cache.find_paths(&self.root, path_str)?;

        let mut count = 0usize;
        // Delete in reverse order to preserve indices for array operations.
        for path in paths.iter().rev() {
            if self.delete_by_path(path) {
                count += 1;
            }
        }
        Ok(count)
    }

    // -- array helpers -------------------------------------------------------

    /// Append one or more values to the array at `path_str`.
    /// Returns the new length of the array.
    pub fn arr_append(&mut self, path_str: &str, values: Vec<Value>) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;

        match target {
            Value::Array(arr) => {
                arr.extend(values);
                Ok(arr.len())
            }
            other => Err(JsonError::NotAnArray(format!(
                "expected array at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Length of the array at `path_str`.
    pub fn arr_len(&self, path_str: &str) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Array(arr) => Ok(arr.len()),
            other => Err(JsonError::NotAnArray(format!(
                "expected array at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Pop the last element from the array at `path_str`. Returns the popped element.
    pub fn arr_pop(&mut self, path_str: &str) -> Result<Value, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Array(arr) => arr.pop().ok_or(JsonError::NotAnArray(
                format!("array at '{}' is empty", path_str),
            )),
            other => Err(JsonError::NotAnArray(format!(
                "expected array at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Return the first index of `scalar` in the array at `path_str`, or -1 if not found.
    pub fn arr_index(&self, path_str: &str, scalar: &Value) -> Result<i64, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Array(arr) => {
                for (i, el) in arr.iter().enumerate() {
                    if el == scalar {
                        return Ok(i as i64);
                    }
                }
                Ok(-1)
            }
            other => Err(JsonError::NotAnArray(format!(
                "expected array at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Insert `values` into the array at `path_str` before `index`.
    /// Negative indices count from the end.
    pub fn arr_insert(
        &mut self,
        path_str: &str,
        index: i64,
        values: Vec<Value>,
    ) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Array(arr) => {
                let len = arr.len() as i64;
                let idx = if index < 0 {
                    (len + index).max(0) as usize
                } else {
                    index.min(len) as usize
                };
                for (offset, v) in values.into_iter().enumerate() {
                    arr.insert(idx + offset, v);
                }
                Ok(arr.len())
            }
            other => Err(JsonError::NotAnArray(format!(
                "expected array at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Trim the array at `path_str` to the range `[start, stop]` (inclusive).
    /// Returns the new length.
    pub fn arr_trim(&mut self, path_str: &str, start: i64, stop: i64) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Array(arr) => {
                let len = arr.len() as i64;
                let real_start = if start < 0 {
                    (len + start).max(0) as usize
                } else {
                    start.min(len) as usize
                };
                let real_stop = if stop < 0 {
                    (len + stop + 1).max(0) as usize
                } else {
                    (stop + 1).min(len) as usize
                };
                let trimmed: Vec<Value> = arr
                    .drain(..)
                    .skip(real_start)
                    .take(if real_stop > real_start {
                        real_stop - real_start
                    } else {
                        0
                    })
                    .collect();
                *arr = trimmed;
                Ok(arr.len())
            }
            other => Err(JsonError::NotAnArray(format!(
                "expected array at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    // -- string helpers ------------------------------------------------------

    /// Append a string suffix to the string value at `path_str`.
    /// Returns the new string length.
    pub fn str_append(&mut self, path_str: &str, suffix: &str) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::String(s) => {
                s.push_str(suffix);
                Ok(s.len())
            }
            other => Err(JsonError::NotAString(format!(
                "expected string at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Length of the string at `path_str`.
    pub fn str_len(&self, path_str: &str) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::String(s) => Ok(s.len()),
            other => Err(JsonError::NotAString(format!(
                "expected string at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    // -- numeric helpers -----------------------------------------------------

    /// Add `delta` to the number at `path_str`. Returns the new value.
    pub fn num_incrby(&mut self, path_str: &str, delta: f64) -> Result<Value, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Number(n) => {
                let current = n.as_f64().ok_or(JsonError::Overflow)?;
                let next = current + delta;
                // Prefer integer representation when both current and delta are integers.
                let new_val = if next.fract() == 0.0 && next >= i64::MIN as f64 && next <= i64::MAX as f64 {
                    json!(next as i64)
                } else {
                    json!(next)
                };
                *target = new_val.clone();
                Ok(new_val)
            }
            other => Err(JsonError::NotANumber(format!(
                "expected number at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Multiply the number at `path_str` by `factor`. Returns the new value.
    pub fn num_multby(&mut self, path_str: &str, factor: f64) -> Result<Value, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Number(n) => {
                let current = n.as_f64().ok_or(JsonError::Overflow)?;
                let next = current * factor;
                let new_val = if next.fract() == 0.0 && next >= i64::MIN as f64 && next <= i64::MAX as f64 {
                    json!(next as i64)
                } else {
                    json!(next)
                };
                *target = new_val.clone();
                Ok(new_val)
            }
            other => Err(JsonError::NotANumber(format!(
                "expected number at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    // -- object helpers ------------------------------------------------------

    /// Return the keys of the object at `path_str`.
    pub fn obj_keys(&self, path_str: &str) -> Result<Vec<String>, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Object(map) => Ok(map.keys().cloned().collect()),
            other => Err(JsonError::NotAnObject(format!(
                "expected object at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    /// Number of keys in the object at `path_str`.
    pub fn obj_len(&self, path_str: &str) -> Result<usize, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Object(map) => Ok(map.len()),
            other => Err(JsonError::NotAnObject(format!(
                "expected object at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    // -- boolean helper ------------------------------------------------------

    /// Toggle the boolean at `path_str`. Returns the new value.
    pub fn bool_toggle(&mut self, path_str: &str) -> Result<bool, JsonError> {
        let ptr = jsonpath_to_pointer(path_str)?;
        let target = self
            .root
            .pointer_mut(&ptr)
            .ok_or_else(|| JsonError::PathNotFound(path_str.to_string()))?;
        match target {
            Value::Bool(b) => {
                *b = !*b;
                Ok(*b)
            }
            other => Err(JsonError::NotABoolean(format!(
                "expected boolean at '{}', got {}",
                path_str,
                JsonError::value_type(other)
            ))),
        }
    }

    // -- crate-internal helpers ----------------------------------------------

    /// Mutable access to the root value (used by `JsonStore::clear`).
    pub(crate) fn root_mut(&mut self) -> &mut Value {
        &mut self.root
    }

    // -- private helpers -----------------------------------------------------

    fn value_type(v: &Value) -> &'static str {
        JsonError::value_type(v)
    }

    /// Attempt to set a value by creating intermediate objects if needed.
    /// Only handles simple dot-separated paths like `$.a.b.c`.
    fn set_creating(&mut self, path_str: &str, value: Value) -> Result<(), JsonError> {
        // Convert JSONPath `$.a.b` → JSON pointer `/a/b`.
        let ptr = jsonpath_to_pointer(path_str)?;
        let parts: Vec<&str> = ptr.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            self.root = value;
            return Ok(());
        }

        // Walk down the tree creating missing objects.
        let mut cursor = &mut self.root;
        let last_idx = parts.len() - 1;
        for (i, part) in parts.iter().enumerate() {
            if i == last_idx {
                match cursor {
                    Value::Object(map) => {
                        map.insert((*part).to_string(), value);
                        return Ok(());
                    }
                    _ => {
                        return Err(JsonError::WrongType {
                            path: path_str.to_string(),
                            expected: "object",
                            got: Self::value_type(cursor),
                        })
                    }
                }
            } else {
                // Ensure the intermediate node is an object.
                match cursor {
                    Value::Object(map) => {
                        cursor = map
                            .entry((*part).to_string())
                            .or_insert_with(|| json!({}));
                    }
                    _ => {
                        return Err(JsonError::WrongType {
                            path: path_str.to_string(),
                            expected: "object",
                            got: Self::value_type(cursor),
                        })
                    }
                }
            }
        }
        Ok(())
    }

    /// Delete one node identified by an absolute jsonpath-rust path string
    /// (e.g. `"$.['store'].['book'][0]"`).
    fn delete_by_path(&mut self, abs_path: &str) -> bool {
        // Convert to JSON pointer.
        let ptr = match jsonpath_abs_to_pointer(abs_path) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let parts: Vec<&str> = ptr.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            self.root = Value::Null;
            return true;
        }

        let parent_ptr = if parts.len() == 1 {
            "/".to_string()
        } else {
            "/".to_string() + &parts[..parts.len() - 1].join("/")
        };
        let last_key = parts[parts.len() - 1];

        // Navigate to the parent.
        let parent = if parent_ptr == "/" {
            Some(&mut self.root)
        } else {
            self.root.pointer_mut(&("/".to_string() + &parts[..parts.len() - 1].join("/")))
        };

        match parent {
            Some(Value::Object(map)) => {
                map.remove(last_key).is_some()
            }
            Some(Value::Array(arr)) => {
                if let Ok(idx) = last_key.parse::<usize>() {
                    if idx < arr.len() {
                        arr.remove(idx);
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Path conversion helpers
// ---------------------------------------------------------------------------

/// Convert a simple JSONPath string like `$.a.b[0]` into a JSON Pointer like `/a/b/0`.
///
/// This intentionally supports only the subset of JSONPath that maps 1:1 to
/// JSON Pointers (root, field access, numeric index). Complex predicates like
/// `[?(@.active)]` will return `Err(JsonError::InvalidPath)`.
pub(crate) fn jsonpath_to_pointer(path: &str) -> Result<String, JsonError> {
    if path == "$" {
        return Ok(String::new());
    }

    // Strip leading `$` or `$.`
    let stripped = path.strip_prefix("$.").unwrap_or_else(|| path.strip_prefix('$').unwrap_or(path));

    let mut ptr = String::new();
    // Tokenize on `.` and `[...]`.
    let mut remaining = stripped;
    while !remaining.is_empty() {
        if remaining.starts_with('[') {
            let end = remaining.find(']').ok_or_else(|| JsonError::InvalidPath {
                path: path.to_string(),
                reason: "unmatched '['".to_string(),
            })?;
            let key = &remaining[1..end];
            // Strip quotes for string keys.
            let key = key.trim_matches('\'').trim_matches('"');
            ptr.push('/');
            ptr.push_str(&key.replace('~', "~0").replace('/', "~1"));
            remaining = &remaining[end + 1..];
            // Skip optional leading dot after closing bracket.
            if remaining.starts_with('.') {
                remaining = &remaining[1..];
            }
        } else {
            let dot_pos = remaining.find('.').unwrap_or(remaining.len());
            let bracket_pos = remaining.find('[').unwrap_or(remaining.len());
            let end = dot_pos.min(bracket_pos);
            let key = &remaining[..end];
            if !key.is_empty() {
                ptr.push('/');
                ptr.push_str(&key.replace('~', "~0").replace('/', "~1"));
            }
            remaining = &remaining[end..];
            if remaining.starts_with('.') {
                remaining = &remaining[1..];
            }
        }
    }
    Ok(ptr)
}

/// Convert an absolute path returned by jsonpath-rust (e.g. `"$.['store'].['book'][0]"`)
/// to a JSON Pointer.
fn jsonpath_abs_to_pointer(abs: &str) -> Result<String, JsonError> {
    // jsonpath-rust uses `$.['key']` for object fields and `$[idx]` for indices.
    // We handle that here.
    let stripped = abs.strip_prefix("$.").unwrap_or(abs.strip_prefix('$').unwrap_or(abs));
    jsonpath_to_pointer(&format!("$.{stripped}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- basic set / get / delete --------------------------------------------

    #[test]
    fn test_set_root() {
        let mut doc = JsonDocument::new(json!({"a": 1}));
        doc.set("$", json!({"b": 2})).unwrap();
        assert_eq!(doc.root(), &json!({"b": 2}));
    }

    #[test]
    fn test_set_nested() {
        let mut doc = JsonDocument::new(json!({"a": {"b": 1}}));
        doc.set("$.a.b", json!(99)).unwrap();
        assert_eq!(doc.root(), &json!({"a": {"b": 99}}));
    }

    #[test]
    fn test_query_wildcard() {
        let doc = JsonDocument::new(json!({"names": [{"n": "alice"}, {"n": "bob"}]}));
        let result = doc.query("$.names[*].n").unwrap();
        assert_eq!(result, vec![json!("alice"), json!("bob")]);
    }

    #[test]
    fn test_type_at_all_types() {
        let doc = JsonDocument::new(json!({
            "s": "hello",
            "i": 42,
            "f": 3.14,
            "b": true,
            "n": null,
            "arr": [1, 2],
            "obj": {}
        }));
        assert_eq!(doc.type_at("$.s"), Some("string"));
        assert_eq!(doc.type_at("$.i"), Some("integer"));
        assert_eq!(doc.type_at("$.b"), Some("boolean"));
        assert_eq!(doc.type_at("$.n"), Some("null"));
        assert_eq!(doc.type_at("$.arr"), Some("array"));
        assert_eq!(doc.type_at("$.obj"), Some("object"));
    }

    // -- array operations ----------------------------------------------------

    #[test]
    fn test_arr_append_and_len() {
        let mut doc = JsonDocument::new(json!({"arr": [1, 2, 3]}));
        let len = doc.arr_append("$.arr", vec![json!(4), json!(5)]).unwrap();
        assert_eq!(len, 5);
        assert_eq!(doc.arr_len("$.arr").unwrap(), 5);
    }

    #[test]
    fn test_arr_pop() {
        let mut doc = JsonDocument::new(json!({"arr": [1, 2, 3]}));
        let popped = doc.arr_pop("$.arr").unwrap();
        assert_eq!(popped, json!(3));
        assert_eq!(doc.arr_len("$.arr").unwrap(), 2);
    }

    #[test]
    fn test_arr_index() {
        let doc = JsonDocument::new(json!({"arr": ["a", "b", "c"]}));
        assert_eq!(doc.arr_index("$.arr", &json!("b")).unwrap(), 1);
        assert_eq!(doc.arr_index("$.arr", &json!("z")).unwrap(), -1);
    }

    #[test]
    fn test_arr_insert() {
        let mut doc = JsonDocument::new(json!({"arr": [1, 2, 3]}));
        doc.arr_insert("$.arr", 1, vec![json!(99)]).unwrap();
        assert_eq!(doc.root(), &json!({"arr": [1, 99, 2, 3]}));
    }

    #[test]
    fn test_arr_trim() {
        let mut doc = JsonDocument::new(json!({"arr": [0, 1, 2, 3, 4]}));
        doc.arr_trim("$.arr", 1, 3).unwrap();
        assert_eq!(doc.root(), &json!({"arr": [1, 2, 3]}));
    }

    // -- string operations ---------------------------------------------------

    #[test]
    fn test_str_append() {
        let mut doc = JsonDocument::new(json!({"s": "hello"}));
        let new_len = doc.str_append("$.s", " world").unwrap();
        assert_eq!(new_len, 11);
        assert_eq!(doc.root(), &json!({"s": "hello world"}));
    }

    #[test]
    fn test_str_len() {
        let doc = JsonDocument::new(json!({"s": "kaya"}));
        assert_eq!(doc.str_len("$.s").unwrap(), 4);
    }

    // -- numeric operations --------------------------------------------------

    #[test]
    fn test_num_incrby_integer() {
        let mut doc = JsonDocument::new(json!({"n": 10}));
        let result = doc.num_incrby("$.n", 5.0).unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn test_num_incrby_float() {
        let mut doc = JsonDocument::new(json!({"n": 1.5}));
        let result = doc.num_incrby("$.n", 0.5).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn test_num_multby() {
        let mut doc = JsonDocument::new(json!({"n": 3}));
        let result = doc.num_multby("$.n", 4.0).unwrap();
        assert_eq!(result, json!(12));
    }

    // -- object operations ---------------------------------------------------

    #[test]
    fn test_obj_keys() {
        let doc = JsonDocument::new(json!({"a": 1, "b": 2}));
        let mut keys = doc.obj_keys("$").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn test_obj_len() {
        let doc = JsonDocument::new(json!({"x": 1, "y": 2, "z": 3}));
        assert_eq!(doc.obj_len("$").unwrap(), 3);
    }

    // -- boolean toggle ------------------------------------------------------

    #[test]
    fn test_bool_toggle() {
        let mut doc = JsonDocument::new(json!({"flag": true}));
        let new_val = doc.bool_toggle("$.flag").unwrap();
        assert!(!new_val);
        assert_eq!(doc.root(), &json!({"flag": false}));
    }

    // -- delete --------------------------------------------------------------

    #[test]
    fn test_delete_root() {
        let mut doc = JsonDocument::new(json!({"a": 1}));
        let count = doc.delete("$").unwrap();
        assert_eq!(count, 1);
    }

    // -- size ----------------------------------------------------------------

    #[test]
    fn test_size() {
        let doc = JsonDocument::new(json!({"a": 1}));
        assert!(doc.size() > 0);
    }

    // -- path not found error ------------------------------------------------

    #[test]
    fn test_path_not_found_error() {
        let mut doc = JsonDocument::new(json!({"a": 1}));
        let err = doc.arr_len("$.missing").unwrap_err();
        assert!(matches!(err, JsonError::PathNotFound(_)));
    }

    // -- wrong type error ----------------------------------------------------

    #[test]
    fn test_wrong_type_for_arr_on_string() {
        let mut doc = JsonDocument::new(json!({"s": "hello"}));
        let err = doc.arr_append("$.s", vec![json!(1)]).unwrap_err();
        assert!(matches!(err, JsonError::NotAnArray(_)));
    }
}
