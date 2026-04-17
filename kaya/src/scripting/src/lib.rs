//! KAYA Scripting: Rhai scripting engine, WASM support, Lua compatibility.
//!
//! Replaces DragonflyDB Lua scripts with Rhai for atomic operations like
//! write_behind_dedup, acquire_lock, worm_lock.

pub mod cache;
pub mod engine;
pub mod error;
pub mod functions;
pub mod metrics;
pub mod parser;


use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use cache::ScriptCache;
pub use engine::ScriptEngine;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("script compilation error: {0}")]
    Compilation(String),

    #[error("script execution error: {0}")]
    Execution(String),

    #[error("script not found: {0}")]
    NotFound(String),

    #[error("script timeout after {0}ms")]
    Timeout(u64),

    #[error("script error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub rhai_enabled: bool,
    pub wasm_enabled: bool,
    pub lua_enabled: bool,
    pub cache_size: usize,
    pub max_execution_ms: u64,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            rhai_enabled: true,
            wasm_enabled: false,
            lua_enabled: false,
            cache_size: 256,
            max_execution_ms: 5000,
        }
    }
}

// ---------------------------------------------------------------------------
// Script result
// ---------------------------------------------------------------------------

/// Result of a script execution.
#[derive(Debug, Clone)]
pub enum ScriptResult {
    Nil,
    Integer(i64),
    Str(String),
    Bool(bool),
    Array(Vec<ScriptResult>),
    Error(String),
}

impl ScriptResult {
    /// Convert to a RESP3-compatible representation.
    pub fn to_resp_string(&self) -> String {
        match self {
            Self::Nil => "(nil)".into(),
            Self::Integer(n) => n.to_string(),
            Self::Str(s) => s.clone(),
            Self::Bool(b) => b.to_string(),
            Self::Array(items) => {
                let parts: Vec<String> = items.iter().map(|i| i.to_resp_string()).collect();
                format!("[{}]", parts.join(", "))
            }
            Self::Error(e) => format!("(error) {e}"),
        }
    }
}
