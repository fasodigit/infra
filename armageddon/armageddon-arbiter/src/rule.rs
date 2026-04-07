//! WAF rule definitions (OWASP CRS v4 compatible).

use serde::{Deserialize, Serialize};

/// A single WAF rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WafRule {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub paranoia_level: u8,
    pub severity: WafSeverity,
    pub phase: RulePhase,
    pub targets: Vec<RuleTarget>,
    pub operator: RuleOperator,
    pub pattern: String,
    pub action: RuleAction,
    pub tags: Vec<String>,
    pub score: u32,
    pub enabled: bool,
}

/// WAF severity levels aligned to OWASP CRS.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WafSeverity {
    Critical,
    Error,
    Warning,
    Notice,
}

/// Request processing phase.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RulePhase {
    RequestHeaders,
    RequestBody,
    ResponseHeaders,
    ResponseBody,
    Logging,
}

/// What part of the request to inspect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleTarget {
    Uri,
    QueryString,
    RequestHeaders,
    RequestBody,
    ResponseHeaders,
    ResponseBody,
    RemoteAddr,
    Method,
    #[serde(rename = "header")]
    SpecificHeader(String),
    #[serde(rename = "cookie")]
    SpecificCookie(String),
    #[serde(rename = "arg")]
    SpecificArg(String),
}

/// Matching operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleOperator {
    Regex,
    Contains,
    Exact,
    StartsWith,
    EndsWith,
    DetectSqli,
    DetectXss,
    GeoLookup,
    IpMatch,
    ValidateByteRange,
}

/// Action to take when a rule matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RuleAction {
    Block,
    Pass,
    Log,
    Redirect,
    AnomalyScore,
}
