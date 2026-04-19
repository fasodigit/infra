//! Security engine configuration.

use serde::{Deserialize, Serialize};

/// Configuration for all Pentagon security engines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub sentinel: SentinelConfig,
    pub arbiter: ArbiterConfig,
    pub oracle: OracleConfig,
    pub aegis: AegisConfig,
    pub nexus: NexusConfig,
    pub veil: VeilConfig,
    pub wasm: WasmConfig,
    pub ai: AiConfig,
    /// GraphQL depth/complexity limiter applied before the Pentagon pipeline.
    #[serde(default)]
    pub graphql_limits: GraphQLLimiterConfig,
}

/// GraphQL depth, complexity, alias, and introspection limiter config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLLimiterConfig {
    /// Enable the limiter.  When `false` all GraphQL queries pass unchecked.
    #[serde(default = "default_gql_enabled")]
    pub enabled: bool,
    /// Maximum query nesting depth (0 = unlimited).
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Maximum query complexity score (0 = unlimited).
    #[serde(default = "default_max_complexity")]
    pub max_complexity: u32,
    /// Maximum number of alias definitions in a document (0 = unlimited).
    #[serde(default = "default_max_aliases")]
    pub max_aliases: u32,
    /// Maximum number of directive usages in a document (0 = unlimited).
    #[serde(default = "default_max_directives")]
    pub max_directives: u32,
    /// Whether `__schema` / `__type` introspection queries are permitted.
    #[serde(default)]
    pub introspection_enabled: bool,
}

fn default_gql_enabled() -> bool { true }
fn default_max_depth() -> u32 { 8 }
fn default_max_complexity() -> u32 { 1000 }
fn default_max_aliases() -> u32 { 30 }
fn default_max_directives() -> u32 { 20 }

impl Default for GraphQLLimiterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_depth: default_max_depth(),
            max_complexity: default_max_complexity(),
            max_aliases: default_max_aliases(),
            max_directives: default_max_directives(),
            introspection_enabled: false,
        }
    }
}

/// SENTINEL (IPS) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelConfig {
    pub enabled: bool,
    pub signature_path: String,
    pub geoip_db_path: String,
    pub blocked_countries: Vec<String>,
    pub ja3_blacklist_path: Option<String>,
    #[serde(default)]
    pub ja4_blacklist_path: Option<String>,
    pub rate_limit: RateLimitConfig,
    pub dlp: DlpConfig,
}

/// Rate limiting configuration (sliding window).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub window_secs: u64,
    pub max_requests: u64,
    pub key_type: RateLimitKeyType,
}

/// What to key rate limits on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitKeyType {
    #[serde(rename = "ip")]
    Ip,
    #[serde(rename = "jwt_sub")]
    JwtSub,
    #[serde(rename = "api_key")]
    ApiKey,
    #[serde(rename = "composite")]
    Composite,
}

/// DLP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpConfig {
    pub enabled: bool,
    pub patterns_path: String,
    pub scan_response: bool,
}

/// ARBITER (WAF) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbiterConfig {
    pub enabled: bool,
    pub paranoia_level: u8,
    pub crs_path: String,
    pub custom_rules_path: Option<String>,
    pub anomaly_threshold: u32,
    pub learning_mode: bool,
}

/// ORACLE (AI anomaly detection) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleConfig {
    pub enabled: bool,
    pub model_path: String,
    pub feature_count: usize,
    pub anomaly_threshold: f64,
    pub prompt_injection_threshold: f64,
}

/// AEGIS (policy engine) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AegisConfig {
    pub enabled: bool,
    pub policy_dir: String,
    pub default_decision: AegisDefault,
}

/// AEGIS default decision (deny-by-default).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AegisDefault {
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "allow")]
    Allow,
}

/// NEXUS (brain) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusConfig {
    pub block_threshold: f64,
    pub challenge_threshold: f64,
    pub correlation_window_ms: u64,
}

/// VEIL (header masking) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeilConfig {
    pub enabled: bool,
    pub remove_headers: Vec<String>,
    pub inject_headers: Vec<HeaderInjection>,
}

/// A header to inject into responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderInjection {
    pub name: String,
    pub value: String,
}

/// WASM plugin runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    pub enabled: bool,
    pub plugins_dir: String,
    pub max_memory_bytes: u64,
    pub max_execution_time_ms: u64,
}

/// AI (threat intel / prompt injection) configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub enabled: bool,
    pub threat_intel_feeds: Vec<String>,
    pub prompt_injection_model_path: Option<String>,
    pub refresh_interval_secs: u64,
}
