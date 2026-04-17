//! Error types for the KAYA JSON engine.

use thiserror::Error;

/// All errors that can arise when working with JSON documents or the JSON store.
#[derive(Debug, Error)]
pub enum JsonError {
    // -- parse / path errors --------------------------------------------------

    #[error("invalid JSONPath expression '{path}': {reason}")]
    InvalidPath { path: String, reason: String },

    #[error("path not found: {0}")]
    PathNotFound(String),

    #[error("JSON parse error: {0}")]
    ParseError(String),

    // -- type errors ----------------------------------------------------------

    #[error("wrong type at '{path}': expected {expected}, got {got}")]
    WrongType {
        path: String,
        expected: &'static str,
        got: &'static str,
    },

    #[error("not a number at '{0}'")]
    NotANumber(String),

    #[error("not a string at '{0}'")]
    NotAString(String),

    #[error("not an array at '{0}'")]
    NotAnArray(String),

    #[error("not an object at '{0}'")]
    NotAnObject(String),

    #[error("not a boolean at '{0}'")]
    NotABoolean(String),

    // -- key errors -----------------------------------------------------------

    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("key already exists: {0}")]
    KeyExists(String),

    // -- condition errors (NX / XX) -------------------------------------------

    #[error("NX condition failed: key already exists")]
    NxConditionFailed,

    #[error("XX condition failed: key does not exist")]
    XxConditionFailed,

    // -- index / bounds errors -------------------------------------------------

    #[error("array index out of bounds: index {index}, length {length}")]
    IndexOutOfBounds { index: i64, length: usize },

    // -- arithmetic -----------------------------------------------------------

    #[error("arithmetic overflow")]
    Overflow,
}

impl JsonError {
    /// Return the JSON type name string for a `serde_json::Value`.
    pub fn value_type(v: &serde_json::Value) -> &'static str {
        match v {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(n) if n.is_f64() && !n.is_i64() && !n.is_u64() => "number",
            serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }
}
