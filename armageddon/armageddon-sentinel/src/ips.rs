//! IPS engine with attack signatures using Aho-Corasick for multi-pattern matching.
//!
//! Built-in signatures cover: SQL injection, XSS, command injection, path traversal.

use aho_corasick::AhoCorasick;
use serde::{Deserialize, Serialize};

/// An IPS signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub severity: String,
    pub category: SignatureCategory,
    pub enabled: bool,
}

/// Signature categories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignatureCategory {
    SqlInjection,
    Xss,
    CommandInjection,
    PathTraversal,
    FileInclusion,
    Deserialization,
    Ssrf,
    Xxe,
    ProtocolViolation,
    Scanner,
    Bot,
    Custom,
}

/// Result of an IPS scan: which signatures matched.
#[derive(Debug, Clone)]
pub struct IpsMatch {
    pub signature_id: String,
    pub signature_name: String,
    pub category: SignatureCategory,
    pub severity: String,
    pub offset: usize,
}

/// The IPS scanning engine using Aho-Corasick for efficient multi-pattern matching.
pub struct IpsEngine {
    signatures_path: String,
    signatures: Vec<Signature>,
    automaton: Option<AhoCorasick>,
}

impl IpsEngine {
    pub fn new(signatures_path: &str) -> Self {
        Self {
            signatures_path: signatures_path.to_string(),
            signatures: Vec::new(),
            automaton: None,
        }
    }

    /// Load built-in signatures and compile the Aho-Corasick automaton.
    pub fn init(&mut self) {
        self.signatures = Self::builtin_signatures();
        self.compile();
        tracing::info!(
            "IPS engine initialized with {} signatures (path: {})",
            self.signatures.len(),
            self.signatures_path,
        );
    }

    /// Compile signatures into an Aho-Corasick automaton for O(n) scanning.
    fn compile(&mut self) {
        let patterns: Vec<&str> = self
            .signatures
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.pattern.as_str())
            .collect();

        if patterns.is_empty() {
            return;
        }

        match AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build(&patterns)
        {
            Ok(ac) => {
                self.automaton = Some(ac);
            }
            Err(e) => {
                tracing::error!("failed to compile IPS Aho-Corasick automaton: {}", e);
            }
        }
    }

    /// Scan a payload against all loaded signatures.
    pub fn scan(&self, payload: &[u8]) -> Vec<IpsMatch> {
        let Some(ac) = &self.automaton else {
            return Vec::new();
        };

        let enabled_sigs: Vec<&Signature> =
            self.signatures.iter().filter(|s| s.enabled).collect();

        ac.find_iter(payload)
            .filter_map(|m| {
                let idx = m.pattern().as_usize();
                enabled_sigs.get(idx).map(|sig| IpsMatch {
                    signature_id: sig.id.clone(),
                    signature_name: sig.name.clone(),
                    category: sig.category.clone(),
                    severity: sig.severity.clone(),
                    offset: m.start(),
                })
            })
            .collect()
    }

    /// Scan a request URI, headers, and body. Returns all matches.
    pub fn scan_request(
        &self,
        uri: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Vec<IpsMatch> {
        let mut matches = Vec::new();

        // Scan URI
        matches.extend(self.scan(uri.as_bytes()));

        // Scan header values
        for (_, value) in headers {
            matches.extend(self.scan(value.as_bytes()));
        }

        // Scan body
        if let Some(body) = body {
            matches.extend(self.scan(body));
        }

        matches
    }

    /// Built-in attack signatures covering OWASP Top 10.
    fn builtin_signatures() -> Vec<Signature> {
        vec![
            // --- SQL Injection ---
            Signature {
                id: "SIG-SQLI-001".into(),
                name: "SQL UNION SELECT".into(),
                pattern: "union select".into(),
                severity: "high".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-002".into(),
                name: "SQL OR 1=1".into(),
                pattern: "or 1=1".into(),
                severity: "high".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-003".into(),
                name: "SQL DROP TABLE".into(),
                pattern: "drop table".into(),
                severity: "critical".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-004".into(),
                name: "SQL comment injection".into(),
                pattern: "' --".into(),
                severity: "high".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-005".into(),
                name: "SQL WAITFOR DELAY".into(),
                pattern: "waitfor delay".into(),
                severity: "high".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-006".into(),
                name: "SQL BENCHMARK".into(),
                pattern: "benchmark(".into(),
                severity: "high".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-007".into(),
                name: "SQL hex injection".into(),
                pattern: "0x3127".into(),
                severity: "medium".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-SQLI-008".into(),
                name: "SQL information_schema".into(),
                pattern: "information_schema".into(),
                severity: "high".into(),
                category: SignatureCategory::SqlInjection,
                enabled: true,
            },
            // --- XSS ---
            Signature {
                id: "SIG-XSS-001".into(),
                name: "XSS script tag".into(),
                pattern: "<script".into(),
                severity: "high".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            Signature {
                id: "SIG-XSS-002".into(),
                name: "XSS javascript protocol".into(),
                pattern: "javascript:".into(),
                severity: "high".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            Signature {
                id: "SIG-XSS-003".into(),
                name: "XSS onerror handler".into(),
                pattern: "onerror=".into(),
                severity: "high".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            Signature {
                id: "SIG-XSS-004".into(),
                name: "XSS onload handler".into(),
                pattern: "onload=".into(),
                severity: "medium".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            Signature {
                id: "SIG-XSS-005".into(),
                name: "XSS img src injection".into(),
                pattern: "<img src=".into(),
                severity: "medium".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            Signature {
                id: "SIG-XSS-006".into(),
                name: "XSS eval()".into(),
                pattern: "eval(".into(),
                severity: "high".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            Signature {
                id: "SIG-XSS-007".into(),
                name: "XSS document.cookie".into(),
                pattern: "document.cookie".into(),
                severity: "high".into(),
                category: SignatureCategory::Xss,
                enabled: true,
            },
            // --- Path Traversal ---
            Signature {
                id: "SIG-TRAV-001".into(),
                name: "Path traversal ../".into(),
                pattern: "../".into(),
                severity: "high".into(),
                category: SignatureCategory::PathTraversal,
                enabled: true,
            },
            Signature {
                id: "SIG-TRAV-002".into(),
                name: "Path traversal encoded".into(),
                pattern: "..%2f".into(),
                severity: "high".into(),
                category: SignatureCategory::PathTraversal,
                enabled: true,
            },
            Signature {
                id: "SIG-TRAV-003".into(),
                name: "Path traversal /etc/passwd".into(),
                pattern: "/etc/passwd".into(),
                severity: "critical".into(),
                category: SignatureCategory::PathTraversal,
                enabled: true,
            },
            Signature {
                id: "SIG-TRAV-004".into(),
                name: "Path traversal /etc/shadow".into(),
                pattern: "/etc/shadow".into(),
                severity: "critical".into(),
                category: SignatureCategory::PathTraversal,
                enabled: true,
            },
            // --- Command Injection ---
            Signature {
                id: "SIG-CMD-001".into(),
                name: "Command injection pipe".into(),
                pattern: "| /bin/".into(),
                severity: "critical".into(),
                category: SignatureCategory::CommandInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-CMD-002".into(),
                name: "Command injection backtick".into(),
                pattern: "`id`".into(),
                severity: "critical".into(),
                category: SignatureCategory::CommandInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-CMD-003".into(),
                name: "Command injection $()".into(),
                pattern: "$(whoami)".into(),
                severity: "critical".into(),
                category: SignatureCategory::CommandInjection,
                enabled: true,
            },
            Signature {
                id: "SIG-CMD-004".into(),
                name: "Command injection ; cat".into(),
                pattern: "; cat ".into(),
                severity: "high".into(),
                category: SignatureCategory::CommandInjection,
                enabled: true,
            },
            // --- Scanner / Bot ---
            Signature {
                id: "SIG-SCAN-001".into(),
                name: "Nmap scanner".into(),
                pattern: "nmap".into(),
                severity: "medium".into(),
                category: SignatureCategory::Scanner,
                enabled: true,
            },
            Signature {
                id: "SIG-SCAN-002".into(),
                name: "SQLmap scanner".into(),
                pattern: "sqlmap".into(),
                severity: "high".into(),
                category: SignatureCategory::Scanner,
                enabled: true,
            },
            Signature {
                id: "SIG-SCAN-003".into(),
                name: "Nikto scanner".into(),
                pattern: "nikto".into(),
                severity: "medium".into(),
                category: SignatureCategory::Scanner,
                enabled: true,
            },
            // --- SSRF ---
            Signature {
                id: "SIG-SSRF-001".into(),
                name: "SSRF localhost".into(),
                pattern: "http://127.0.0.1".into(),
                severity: "high".into(),
                category: SignatureCategory::Ssrf,
                enabled: true,
            },
            Signature {
                id: "SIG-SSRF-002".into(),
                name: "SSRF metadata".into(),
                pattern: "169.254.169.254".into(),
                severity: "critical".into(),
                category: SignatureCategory::Ssrf,
                enabled: true,
            },
            // --- XXE ---
            Signature {
                id: "SIG-XXE-001".into(),
                name: "XXE entity declaration".into(),
                pattern: "<!ENTITY".into(),
                severity: "high".into(),
                category: SignatureCategory::Xxe,
                enabled: true,
            },
            Signature {
                id: "SIG-XXE-002".into(),
                name: "XXE SYSTEM".into(),
                pattern: "SYSTEM \"file://".into(),
                severity: "critical".into(),
                category: SignatureCategory::Xxe,
                enabled: true,
            },
        ]
    }

    /// Number of loaded signatures.
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> IpsEngine {
        let mut engine = IpsEngine::new("/dev/null");
        engine.init();
        engine
    }

    #[test]
    fn test_sql_injection_detection() {
        let engine = make_engine();
        let payload = b"SELECT * FROM users WHERE id=1 UNION SELECT password FROM admin";
        let matches = engine.scan(payload);
        assert!(!matches.is_empty());
        assert!(matches
            .iter()
            .any(|m| m.category == SignatureCategory::SqlInjection));
    }

    #[test]
    fn test_xss_detection() {
        let engine = make_engine();
        let payload = b"<script>alert(document.cookie)</script>";
        let matches = engine.scan(payload);
        assert!(!matches.is_empty());
        assert!(matches
            .iter()
            .any(|m| m.category == SignatureCategory::Xss));
    }

    #[test]
    fn test_path_traversal_detection() {
        let engine = make_engine();
        let payload = b"GET /../../etc/passwd HTTP/1.1";
        let matches = engine.scan(payload);
        assert!(!matches.is_empty());
        assert!(matches
            .iter()
            .any(|m| m.category == SignatureCategory::PathTraversal));
    }

    #[test]
    fn test_clean_request_no_match() {
        let engine = make_engine();
        let payload = b"GET /api/users/123 HTTP/1.1\r\nHost: example.com\r\n";
        let matches = engine.scan(payload);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_case_insensitive() {
        let engine = make_engine();
        let payload = b"UNION SELECT * FROM users";
        let matches = engine.scan(payload);
        assert!(!matches.is_empty());
    }
}
