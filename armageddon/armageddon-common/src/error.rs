//! Unified error types for ARMAGEDDON.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArmageddonError {
    // --- Gateway / Proxy errors ---
    #[error("upstream connection failed: {0}")]
    UpstreamConnection(String),

    #[error("TLS handshake failed: {0}")]
    TlsHandshake(String),

    #[error("route not found: {method} {path}")]
    RouteNotFound { method: String, path: String },

    #[error("circuit breaker open for cluster: {0}")]
    CircuitBreakerOpen(String),

    #[error("upstream timeout after {0}ms")]
    UpstreamTimeout(u64),

    // --- Security engine errors ---
    #[error("WAF rule triggered: {rule_id} ({message})")]
    WafBlocked { rule_id: String, message: String },

    #[error("IPS signature matched: {signature_id}")]
    IpsBlocked { signature_id: String },

    #[error("rate limit exceeded for key: {0}")]
    RateLimited(String),

    #[error("GeoIP blocked country: {0}")]
    GeoIpBlocked(String),

    #[error("DLP sensitive data detected: {pattern_name}")]
    DlpBlocked { pattern_name: String },

    // --- Auth errors ---
    #[error("JWT validation failed: {0}")]
    JwtInvalid(String),

    #[error("JWT expired")]
    JwtExpired,

    #[error("JWKS fetch failed: {0}")]
    JwksFetchFailed(String),

    #[error("Kratos session invalid: {0}")]
    KratosSessionInvalid(String),

    #[error("Kratos session expired")]
    KratosSessionExpired,

    #[error("Kratos unavailable: {0}")]
    KratosUnavailable(String),

    #[error("OPA ext_authz denied: {0}")]
    ExtAuthzDenied(String),

    #[error("OPA ext_authz unavailable (fail-closed)")]
    ExtAuthzUnavailable,

    // --- Policy errors ---
    #[error("policy denied: {0}")]
    PolicyDenied(String),

    #[error("policy evaluation error: {0}")]
    PolicyEvaluation(String),

    // --- AI / Oracle errors ---
    #[error("anomaly score {score} exceeds threshold {threshold}")]
    AnomalyDetected { score: f64, threshold: f64 },

    #[error("prompt injection detected (confidence: {confidence})")]
    PromptInjection { confidence: f64 },

    #[error("ONNX runtime error: {0}")]
    OnnxRuntime(String),

    // --- WASM errors ---
    #[error("WASM plugin error: {plugin_name}: {message}")]
    WasmPlugin { plugin_name: String, message: String },

    // --- Config errors ---
    #[error("configuration error: {0}")]
    Config(String),

    #[error("xDS stream error: {0}")]
    XdsStream(String),

    // --- Cache / KAYA errors ---
    #[error("KAYA connection error: {0}")]
    KayaConnection(String),

    // --- Generic ---
    #[error("internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Shorthand result type for ARMAGEDDON operations.
pub type Result<T> = std::result::Result<T, ArmageddonError>;
