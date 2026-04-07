//! Decision types returned by security engines.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The verdict an engine reaches about a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Request is safe to proceed.
    Allow,
    /// Request should be blocked.
    Deny,
    /// Engine flagged the request but defers final decision to NEXUS.
    Flag,
    /// Engine could not evaluate (e.g. model not loaded); defer to others.
    Abstain,
}

/// Recommended action for a flagged or denied request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Forward request to upstream.
    Forward,
    /// Block and return error to client.
    Block,
    /// Redirect to a challenge page (e.g. CAPTCHA).
    Challenge,
    /// Rate-limit: slow down but do not block.
    Throttle,
    /// Log and forward (shadow mode / learning mode).
    LogOnly,
}

/// Severity classification for flagged threats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Low => write!(f, "LOW"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::High => write!(f, "HIGH"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A single decision produced by one security engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// Which engine produced this decision.
    pub engine: String,
    /// The verdict.
    pub verdict: Verdict,
    /// Recommended action.
    pub action: Action,
    /// Confidence score (0.0 = uncertain, 1.0 = absolute certainty).
    pub confidence: f64,
    /// Severity of the detected threat, if any.
    pub severity: Option<Severity>,
    /// Rule or signature ID that triggered.
    pub rule_id: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// Tags for categorization (e.g. "sqli", "xss", "bot").
    pub tags: Vec<String>,
    /// Processing latency in microseconds.
    pub latency_us: u64,
}

impl Decision {
    /// Create an allow decision.
    pub fn allow(engine: &str, latency_us: u64) -> Self {
        Self {
            engine: engine.to_string(),
            verdict: Verdict::Allow,
            action: Action::Forward,
            confidence: 1.0,
            severity: None,
            rule_id: None,
            description: "No threat detected".to_string(),
            tags: Vec::new(),
            latency_us,
        }
    }

    /// Create a deny decision.
    pub fn deny(
        engine: &str,
        rule_id: &str,
        description: &str,
        severity: Severity,
        latency_us: u64,
    ) -> Self {
        Self {
            engine: engine.to_string(),
            verdict: Verdict::Deny,
            action: Action::Block,
            confidence: 1.0,
            severity: Some(severity),
            rule_id: Some(rule_id.to_string()),
            description: description.to_string(),
            tags: Vec::new(),
            latency_us,
        }
    }

    /// Create a flag decision (defers to NEXUS).
    pub fn flag(
        engine: &str,
        rule_id: &str,
        description: &str,
        severity: Severity,
        confidence: f64,
        latency_us: u64,
    ) -> Self {
        Self {
            engine: engine.to_string(),
            verdict: Verdict::Flag,
            action: Action::LogOnly,
            confidence,
            severity: Some(severity),
            rule_id: Some(rule_id.to_string()),
            description: description.to_string(),
            tags: Vec::new(),
            latency_us,
        }
    }
}
