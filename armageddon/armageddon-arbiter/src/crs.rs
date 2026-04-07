//! OWASP Core Rule Set v4 loader and built-in rules.
//!
//! Provides built-in WAF rules matching OWASP CRS categories:
//! - 942xxx: SQL Injection
//! - 941xxx: XSS
//! - 932xxx: Remote Code Execution (command injection)
//! - 930xxx: Local File Inclusion (path traversal)

use crate::rule::{RuleAction, RuleOperator, RulePhase, RuleTarget, WafRule, WafSeverity};

/// OWASP CRS rule categories.
#[derive(Debug, Clone, Copy)]
pub enum CrsCategory {
    /// 910xxx - Scanner detection
    ScannerDetection,
    /// 920xxx - Protocol enforcement
    ProtocolEnforcement,
    /// 930xxx - Local File Inclusion
    LocalFileInclusion,
    /// 931xxx - Remote File Inclusion
    RemoteFileInclusion,
    /// 932xxx - Remote Code Execution
    RemoteCodeExecution,
    /// 933xxx - PHP Injection
    PhpInjection,
    /// 934xxx - Node.js Injection
    NodeJsInjection,
    /// 941xxx - XSS
    CrossSiteScripting,
    /// 942xxx - SQL Injection
    SqlInjection,
    /// 943xxx - Session Fixation
    SessionFixation,
    /// 944xxx - Java Injection
    JavaInjection,
}

/// CRS rule set loader.
pub struct CrsLoader {
    crs_path: String,
}

impl CrsLoader {
    pub fn new(crs_path: &str) -> Self {
        Self {
            crs_path: crs_path.to_string(),
        }
    }

    /// Load all built-in CRS rules.
    pub fn load(&self) -> Vec<WafRule> {
        tracing::info!("loading OWASP CRS v4 rules (built-in + {})", self.crs_path);
        let mut rules = Vec::new();
        rules.extend(Self::sqli_rules());
        rules.extend(Self::xss_rules());
        rules.extend(Self::rce_rules());
        rules.extend(Self::lfi_rules());
        rules
    }

    /// Load rules filtered by paranoia level.
    pub fn load_at_paranoia(&self, level: u8) -> Vec<WafRule> {
        self.load()
            .into_iter()
            .filter(|r| r.paranoia_level <= level)
            .collect()
    }

    /// 942xxx: SQL Injection rules.
    fn sqli_rules() -> Vec<WafRule> {
        vec![
            WafRule {
                id: 942100,
                name: "SQL Injection Attack Detected via libinjection".into(),
                description: "Detects SQL injection via common patterns".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectSqli,
                pattern: "union select".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["sqli".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 942110,
                name: "SQL Injection: Common Injection Testing".into(),
                description: "Detects SQL comment/tautology attacks".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectSqli,
                pattern: "or 1=1".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["sqli".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 942120,
                name: "SQL Injection: SQL Operator Detected".into(),
                description: "Detects SQL operators in user input".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectSqli,
                pattern: "' or '".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["sqli".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 942130,
                name: "SQL Injection: DROP/ALTER/CREATE".into(),
                description: "Detects DDL statements".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody, RuleTarget::Uri],
                operator: RuleOperator::DetectSqli,
                pattern: "drop table".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["sqli".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 942140,
                name: "SQL Injection: INFORMATION_SCHEMA access".into(),
                description: "Detects attempts to access INFORMATION_SCHEMA".into(),
                paranoia_level: 2,
                severity: WafSeverity::Warning,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectSqli,
                pattern: "information_schema".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["sqli".into(), "owasp-crs".into()],
                score: 3,
                enabled: true,
            },
            WafRule {
                id: 942150,
                name: "SQL Injection: WAITFOR/BENCHMARK timing".into(),
                description: "Detects timing-based SQL injection".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectSqli,
                pattern: "waitfor delay".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["sqli".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
        ]
    }

    /// 941xxx: XSS rules.
    fn xss_rules() -> Vec<WafRule> {
        vec![
            WafRule {
                id: 941100,
                name: "XSS Attack Detected via libinjection".into(),
                description: "Detects XSS via script tags and event handlers".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody, RuleTarget::Uri],
                operator: RuleOperator::DetectXss,
                pattern: "<script".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["xss".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 941110,
                name: "XSS Filter - Category 1: Script Tag Vector".into(),
                description: "Detects javascript: protocol handler".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody, RuleTarget::Uri],
                operator: RuleOperator::DetectXss,
                pattern: "javascript:".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["xss".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 941120,
                name: "XSS Filter - Category 2: Event Handler Vector".into(),
                description: "Detects onerror/onload event handlers".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectXss,
                pattern: "onerror=".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["xss".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 941130,
                name: "XSS Filter: eval() detection".into(),
                description: "Detects eval() usage".into(),
                paranoia_level: 2,
                severity: WafSeverity::Warning,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectXss,
                pattern: "eval(".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["xss".into(), "owasp-crs".into()],
                score: 3,
                enabled: true,
            },
            WafRule {
                id: 941140,
                name: "XSS Filter: document.cookie".into(),
                description: "Detects cookie theft attempts".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::DetectXss,
                pattern: "document.cookie".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["xss".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
        ]
    }

    /// 932xxx: Remote Code Execution rules.
    fn rce_rules() -> Vec<WafRule> {
        vec![
            WafRule {
                id: 932100,
                name: "Remote Command Execution: Unix Command Injection".into(),
                description: "Detects Unix command injection via pipes and backticks".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::Contains,
                pattern: "| /bin/".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["rce".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 932110,
                name: "Remote Command Execution: Subshell via $()".into(),
                description: "Detects command substitution".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::Contains,
                pattern: "$(whoami)".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["rce".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 932120,
                name: "Remote Command Execution: semicolon chaining".into(),
                description: "Detects command chaining with semicolons".into(),
                paranoia_level: 2,
                severity: WafSeverity::Warning,
                phase: RulePhase::RequestBody,
                targets: vec![RuleTarget::QueryString, RuleTarget::RequestBody],
                operator: RuleOperator::Contains,
                pattern: "; cat ".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["rce".into(), "owasp-crs".into()],
                score: 3,
                enabled: true,
            },
        ]
    }

    /// 930xxx: Local File Inclusion rules.
    fn lfi_rules() -> Vec<WafRule> {
        vec![
            WafRule {
                id: 930100,
                name: "Path Traversal Attack (/../)".into(),
                description: "Detects directory traversal sequences".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestHeaders,
                targets: vec![RuleTarget::Uri, RuleTarget::QueryString],
                operator: RuleOperator::Contains,
                pattern: "../".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["lfi".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
            WafRule {
                id: 930110,
                name: "Path Traversal: /etc/passwd access".into(),
                description: "Detects attempts to read /etc/passwd".into(),
                paranoia_level: 1,
                severity: WafSeverity::Critical,
                phase: RulePhase::RequestHeaders,
                targets: vec![RuleTarget::Uri, RuleTarget::QueryString],
                operator: RuleOperator::Contains,
                pattern: "/etc/passwd".into(),
                action: RuleAction::AnomalyScore,
                tags: vec!["lfi".into(), "owasp-crs".into()],
                score: 5,
                enabled: true,
            },
        ]
    }
}
