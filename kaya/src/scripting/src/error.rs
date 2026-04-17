//! Structured error types for the KAYA Functions library subsystem.
//!
//! All fallible operations on libraries (`LOAD`, `DELETE`, `DUMP`, `RESTORE`,
//! `FCALL`, ...) return [`FunctionError`]. The variants map cleanly to the
//! RESP3 error replies exposed by the command layer.

use thiserror::Error;

/// Errors produced by the sovereign KAYA Functions library runtime.
#[derive(Debug, Error)]
pub enum FunctionError {
    /// The source does not start with a `#!<engine> ...` shebang line.
    #[error("missing '#!<engine>' shebang on first line")]
    MissingShebang,

    /// The `name=...` metadata is missing, malformed, or contains forbidden
    /// characters.
    #[error("invalid library name: {0}")]
    InvalidLibraryName(String),

    /// A library with the same name is already registered and the caller did
    /// not pass the `REPLACE` option.
    #[error("library already exists: {0}")]
    LibraryAlreadyExists(String),

    /// The requested library is not registered.
    #[error("library not found: {0}")]
    LibraryNotFound(String),

    /// The HMAC-SHA-256 signature of a dump does not match the expected
    /// signature (tampering or wrong server key).
    #[error("signature verification failed (tampered or wrong key)")]
    SignatureMismatch,

    /// The requested function name is not exported by any loaded library.
    #[error("function not found: {0}")]
    FunctionNotFound(String),

    /// The function ran longer than `config.scripting.max_execution_ms`.
    #[error("execution timeout")]
    ExecutionTimeout,

    /// The underlying Rhai engine returned a compile or runtime error.
    #[error("rhai error: {0}")]
    RhaiError(String),

    /// Serde serialization or deserialization failure (dump / restore path).
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// The feature is currently disabled by server configuration.
    #[error("feature disabled: {0}")]
    FeatureDisabled(&'static str),

    /// Attempted to call a `readonly` function with a write-capable command.
    #[error("read-only function cannot perform writes")]
    ReadOnlyViolation,
}

impl FunctionError {
    /// Short stable tag usable as a Prometheus label.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::MissingShebang => "missing_shebang",
            Self::InvalidLibraryName(_) => "invalid_name",
            Self::LibraryAlreadyExists(_) => "already_exists",
            Self::LibraryNotFound(_) => "not_found",
            Self::SignatureMismatch => "signature_mismatch",
            Self::FunctionNotFound(_) => "function_not_found",
            Self::ExecutionTimeout => "timeout",
            Self::RhaiError(_) => "rhai_error",
            Self::SerializationError(_) => "serialization",
            Self::FeatureDisabled(_) => "feature_disabled",
            Self::ReadOnlyViolation => "readonly_violation",
        }
    }
}
