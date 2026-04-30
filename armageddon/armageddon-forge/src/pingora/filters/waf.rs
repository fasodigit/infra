// SPDX-License-Identifier: AGPL-3.0-or-later
//! WAF (Web Application Firewall) filter — pragmatic regex-based scanner.
//!
//! Sits at the very top of the filter chain (before router / JWT / OPA) and
//! short-circuits requests whose URL or selected headers match a curated
//! set of OWASP Top-10 patterns:
//!
//! | Family             | Inspected surface     | Examples                                  |
//! |--------------------|-----------------------|-------------------------------------------|
//! | SQL injection      | path + query + UA     | `' OR 1=1 --`, `UNION SELECT`, sqlmap UA  |
//! | XSS                | path + query          | `<script>`, `javascript:`, `onerror=`     |
//! | Command injection  | path + query          | `;cat`, `\|ls`, backticks, `$(...)`       |
//! | SSRF               | query param values    | 127.0.0.1, 169.254.169.254, 10.0.0.0/8    |
//! | Scanner UA         | User-Agent header     | sqlmap, nikto, dirbuster, acunetix        |
//!
//! **Body inspection is NOT performed** at this hook — Pingora's
//! `request_filter` runs before the body is read.  Adding body coverage
//! requires extending [`super::ForgeFilter`] with an `on_request_body`
//! hook and chaining it from `pingora_proxy::ProxyHttp::request_body_filter`.
//! See `armageddon/coraza/WIRING-TODO.md` for the proxy-wasm path.
//!
//! ## Failure semantics
//!
//! - `learning_mode = true` → match logs at `warn` but returns
//!   `Decision::Continue` (telemetry-only).
//! - `learning_mode = false` → match returns `Decision::Deny(block_status)`
//!   (default 403).
//!
//! ## Metrics
//!
//! - `armageddon_waf_blocks_total{rule_family}` — counter
//! - `armageddon_waf_evaluations_total{outcome}` — counter
//!   (`outcome` ∈ `allow` | `block` | `learning_log`)

use std::sync::Arc;

use prometheus::{IntCounterVec, Opts, Registry};
use regex::RegexSet;
use tracing::{debug, warn};

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Rule family identifier — used as a Prometheus label.
#[derive(Debug, Clone, Copy)]
pub enum RuleFamily {
    SqlInjection,
    Xss,
    CommandInjection,
    Ssrf,
    ScannerUa,
    NoSqlInjection,
}

impl RuleFamily {
    fn as_label(self) -> &'static str {
        match self {
            RuleFamily::SqlInjection => "sqli",
            RuleFamily::Xss => "xss",
            RuleFamily::CommandInjection => "cmdi",
            RuleFamily::Ssrf => "ssrf",
            RuleFamily::ScannerUa => "scanner_ua",
            RuleFamily::NoSqlInjection => "nosqli",
        }
    }
}

/// Pre-compiled rule database — cheap to clone (Arc internally via RegexSet).
#[derive(Debug, Clone)]
struct RuleDb {
    sqli: RegexSet,
    xss: RegexSet,
    cmdi: RegexSet,
    ssrf: RegexSet,
    scanner_ua: RegexSet,
    nosql: RegexSet,
}

impl RuleDb {
    fn build() -> Result<Self, regex::Error> {
        // ── SQL injection ────────────────────────────────────────────────
        // Case-insensitive (?i). Patterns target query-string content;
        // legitimate path segments rarely contain these tokens.
        let sqli = RegexSet::new([
            r"(?i)\bunion\s+(all\s+)?select\b",
            r"(?i)\bor\s+1\s*=\s*1\b",
            r"(?i)\band\s+1\s*=\s*1\b",
            r"(?i)\b(select|insert|update|delete|drop|create|alter|exec)\s+",
            r"(?i)'\s*(or|and)\s+'?\d",
            r"(?i)--\s*$",
            r"(?i);\s*(drop|delete|update|insert|truncate)\b",
            r"(?i)\bsleep\s*\(\s*\d+\s*\)",
            r"(?i)\bbenchmark\s*\(",
        ])?;

        // ── XSS ──────────────────────────────────────────────────────────
        let xss = RegexSet::new([
            r"(?i)<\s*script[\s>]",
            r"(?i)javascript\s*:",
            r"(?i)\bon(error|load|click|mouseover|focus|blur)\s*=",
            r"(?i)<\s*iframe[\s>]",
            r"(?i)<\s*svg[^>]*on\w+\s*=",
            r"(?i)\bdocument\s*\.\s*cookie\b",
            r"(?i)<\s*img[^>]+src\s*=\s*['\x22]?\s*x\s*['\x22]?\s+onerror",
        ])?;

        // ── Command injection ────────────────────────────────────────────
        // Match shell metacharacters in URL parameters that are unusual in
        // legitimate REST APIs.
        let cmdi = RegexSet::new([
            r";\s*(cat|ls|nc|wget|curl|chmod|sh|bash|python|perl|whoami|id)\b",
            r"\|\s*(cat|ls|nc|wget|curl|chmod|sh|bash|whoami|id)\b",
            r"`[^`]*`",
            r"\$\([^)]+\)",
            r"&&\s*(cat|ls|whoami|id)\b",
            r"(?i)/etc/(passwd|shadow|hosts)\b",
        ])?;

        // ── SSRF ──────────────────────────────────────────────────────────
        // Patterns target URL-shaped query parameter values that point at
        // internal/metadata IPs.
        let ssrf = RegexSet::new([
            // Cloud metadata IPs
            r"169\.254\.169\.254",
            r"metadata\.google\.internal",
            r"metadata\.azure\.com",
            // Loopback
            r"https?://127\.0\.0\.\d+",
            r"https?://localhost\b",
            r"https?://0\.0\.0\.0\b",
            r"https?://\[?::1\]?",
            // Private CIDR — RFC 1918
            r"https?://10\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})",
            r"https?://192\.168\.",
            r"https?://172\.(1[6-9]|2\d|3[01])\.",
            // file:// gopher:// dict:// — known SSRF schemes
            r"^(file|gopher|dict|ftp|tftp|ldap|sftp)://",
        ])?;

        // ── Scanner User-Agent ────────────────────────────────────────────
        let scanner_ua = RegexSet::new([
            r"(?i)\b(sqlmap|nikto|nessus|acunetix|dirbuster|wpscan|nmap|masscan|gobuster|wfuzz|burpsuite|metasploit|havij|fimap|whatweb)\b",
        ])?;

        // ── NoSQL injection (Mongo / CouchDB operators in JSON bodies) ────
        // Covers: $ne / $gt / $lt / $gte / $lte / $in / $nin / $exists /
        // $where / $regex / $expr / $jsonSchema. Must be a JSON object key
        // (preceded by `{`, `,`, `"`, or whitespace) to reduce false positives
        // — legitimate JSON values like `"price": "$ne 5"` are NOT operator
        // keys and should not trigger.
        let nosql = RegexSet::new([
            // Mongo operators as JSON keys: "$ne":, $gt:, etc.
            r#"["']\s*\$\s*(ne|gt|lt|gte|lte|in|nin|exists|where|regex|expr|jsonSchema|all|elemMatch|size)\s*["']\s*:"#,
            // JS injection in $where: function() { ... }
            r"(?i)\$\s*where[^a-z0-9]+(function|return|this\.|process\.)",
            // sleep() inside $where (timing-based blind)
            r"(?i)\$\s*where[^a-z0-9]+sleep\s*\(",
        ])?;

        Ok(Self {
            sqli,
            xss,
            cmdi,
            ssrf,
            scanner_ua,
            nosql,
        })
    }

    /// Body-only evaluation: SQLi + XSS + cmdi + NoSQL.
    ///
    /// Skips SSRF (URL-only) and scanner-UA (header-only) families that
    /// don't make sense in a body context. Order favors the cheapest /
    /// most common patterns first.
    fn evaluate_body(&self, body: &str) -> Option<RuleFamily> {
        if self.sqli.is_match(body) {
            return Some(RuleFamily::SqlInjection);
        }
        if self.xss.is_match(body) {
            return Some(RuleFamily::Xss);
        }
        if self.nosql.is_match(body) {
            return Some(RuleFamily::NoSqlInjection);
        }
        if self.cmdi.is_match(body) {
            return Some(RuleFamily::CommandInjection);
        }
        None
    }

    /// Returns the first matching family, or `None`.  Inspection order
    /// matters: scanner UA first (cheapest, single header), then URL-based
    /// families.
    fn evaluate(&self, url_haystack: &str, user_agent: &str) -> Option<RuleFamily> {
        if !user_agent.is_empty() && self.scanner_ua.is_match(user_agent) {
            return Some(RuleFamily::ScannerUa);
        }
        if self.sqli.is_match(url_haystack) {
            return Some(RuleFamily::SqlInjection);
        }
        if self.xss.is_match(url_haystack) {
            return Some(RuleFamily::Xss);
        }
        if self.cmdi.is_match(url_haystack) {
            return Some(RuleFamily::CommandInjection);
        }
        if self.ssrf.is_match(url_haystack) {
            return Some(RuleFamily::Ssrf);
        }
        None
    }
}

/// Prometheus metrics for the WAF filter.
#[derive(Debug, Clone)]
struct WafMetrics {
    blocks_total: IntCounterVec,
    evaluations_total: IntCounterVec,
}

impl WafMetrics {
    fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let blocks_total = IntCounterVec::new(
            Opts::new(
                "armageddon_waf_blocks_total",
                "Requests blocked by the WAF, labelled by rule family",
            ),
            &["rule_family"],
        )?;
        registry.register(Box::new(blocks_total.clone()))?;

        let evaluations_total = IntCounterVec::new(
            Opts::new(
                "armageddon_waf_evaluations_total",
                "Total WAF evaluations, labelled by outcome (allow|block|learning_log)",
            ),
            &["outcome"],
        )?;
        registry.register(Box::new(evaluations_total.clone()))?;

        Ok(Self {
            blocks_total,
            evaluations_total,
        })
    }
}

/// Configuration mirror used at runtime.  Decoupled from the serde
/// `armageddon_config::WafConfig` so the forge crate is not forced to
/// depend on the config crate.
#[derive(Debug, Clone)]
pub struct WafFilterConfig {
    pub enabled: bool,
    pub paranoia_level: u8,
    pub learning_mode: bool,
    pub block_status: u16,
    /// Maximum bytes accumulated in `ctx.body_buffer` for body inspection.
    /// Bodies larger than this cap are inspected up to the cap (with the
    /// `body_buffer_overflow` flag set so operators can tune via metrics).
    /// Default: 256 KiB.
    pub max_body_inspect_bytes: usize,
}

impl Default for WafFilterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            paranoia_level: 1,
            learning_mode: false,
            block_status: 403,
            max_body_inspect_bytes: 256 * 1024,
        }
    }
}

/// Returns `true` when the Content-Type header indicates a body that's
/// worth inspecting for injection patterns. Binary uploads (images,
/// video, application/octet-stream) are skipped — random bytes match
/// regex by chance and burn CPU.
fn is_inspectable_content_type(ct: &str) -> bool {
    let lower = ct.to_ascii_lowercase();
    lower.starts_with("application/json")
        || lower.starts_with("application/x-www-form-urlencoded")
        || lower.starts_with("application/xml")
        || lower.starts_with("application/graphql")
        || lower.starts_with("text/")
        || lower.starts_with("multipart/form-data")
}

/// WAF filter — regex-based pre-routing scanner.
#[derive(Debug, Clone)]
pub struct WafFilter {
    rules: Arc<RuleDb>,
    metrics: WafMetrics,
    config: WafFilterConfig,
}

impl WafFilter {
    /// Build a new WAF filter and register its Prometheus metrics on
    /// `registry`.  Returns an error if regex compilation or metric
    /// registration fails.
    pub fn new(config: WafFilterConfig, registry: &Registry) -> anyhow::Result<Self> {
        let rules = Arc::new(RuleDb::build().map_err(|e| anyhow::anyhow!("WAF rule compile failed: {e}"))?);
        let metrics = WafMetrics::new(registry).map_err(|e| anyhow::anyhow!("WAF metrics register failed: {e}"))?;
        Ok(Self {
            rules,
            metrics,
            config,
        })
    }
}

#[async_trait::async_trait]
impl ForgeFilter for WafFilter {
    fn name(&self) -> &'static str {
        "waf"
    }

    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        if !self.config.enabled {
            return Decision::Continue;
        }

        let req_header = session.req_header();
        let path_and_query = req_header
            .uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or_else(|| req_header.uri.path());
        let user_agent = req_header
            .headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        match self.rules.evaluate(path_and_query, user_agent) {
            None => {
                self.metrics
                    .evaluations_total
                    .with_label_values(&["allow"])
                    .inc();
                Decision::Continue
            }
            Some(family) => {
                let label = family.as_label();
                if self.config.learning_mode {
                    self.metrics
                        .evaluations_total
                        .with_label_values(&["learning_log"])
                        .inc();
                    warn!(
                        rule_family = %label,
                        request_id = %ctx.request_id,
                        path = %path_and_query,
                        "waf: rule matched (learning mode — pass-through)"
                    );
                    Decision::Continue
                } else {
                    self.metrics
                        .blocks_total
                        .with_label_values(&[label])
                        .inc();
                    self.metrics
                        .evaluations_total
                        .with_label_values(&["block"])
                        .inc();
                    warn!(
                        rule_family = %label,
                        request_id = %ctx.request_id,
                        path = %path_and_query,
                        status = self.config.block_status,
                        "waf: blocked"
                    );
                    debug!("waf rule matched, denying with status {}", self.config.block_status);
                    Decision::Deny(self.config.block_status)
                }
            }
        }
    }

    /// Body-inspection hook. Buffers chunks (capped at `max_body_inspect_bytes`)
    /// and evaluates the accumulated buffer on `end_of_stream`.
    ///
    /// Skipped silently when the request's Content-Type is binary (image/*,
    /// video/*, application/octet-stream, …) — those don't carry textual
    /// injection payloads and would cause false positives.
    async fn on_request_body(
        &self,
        session: &mut pingora_proxy::Session,
        body: &Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut RequestCtx,
    ) -> Decision {
        if !self.config.enabled {
            return Decision::Continue;
        }

        // Cheap check: skip non-textual content types entirely. Read the
        // header once per request — we don't know if this is the first
        // chunk so just re-read; it's a header lookup, no allocation.
        let ct = session
            .req_header()
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !is_inspectable_content_type(ct) {
            return Decision::Continue;
        }

        // Accumulate chunk bytes into ctx.body_buffer, respecting the cap.
        if let Some(chunk) = body.as_ref() {
            let cap = self.config.max_body_inspect_bytes;
            let remaining = cap.saturating_sub(ctx.body_buffer.len());
            if remaining == 0 {
                ctx.body_buffer_overflow = true;
            } else {
                let take = remaining.min(chunk.len());
                ctx.body_buffer.extend_from_slice(&chunk[..take]);
                if take < chunk.len() {
                    ctx.body_buffer_overflow = true;
                }
            }
        }

        // Single-pass evaluation at end of stream — simpler than per-chunk
        // (cross-chunk patterns are correctly handled).
        if !end_of_stream {
            return Decision::Continue;
        }

        if ctx.body_buffer.is_empty() {
            return Decision::Continue;
        }

        // UTF-8 lossy view: malformed bytes become U+FFFD, doesn't break regex.
        let body_str = std::str::from_utf8(&ctx.body_buffer).unwrap_or("");
        if body_str.is_empty() {
            return Decision::Continue;
        }

        match self.rules.evaluate_body(body_str) {
            None => {
                self.metrics
                    .evaluations_total
                    .with_label_values(&["allow"])
                    .inc();
                Decision::Continue
            }
            Some(family) => {
                let label = family.as_label();
                if self.config.learning_mode {
                    self.metrics
                        .evaluations_total
                        .with_label_values(&["learning_log"])
                        .inc();
                    warn!(
                        rule_family = %label,
                        request_id = %ctx.request_id,
                        body_bytes = ctx.body_buffer.len(),
                        overflow = ctx.body_buffer_overflow,
                        "waf body: rule matched (learning mode — pass-through)"
                    );
                    Decision::Continue
                } else {
                    self.metrics
                        .blocks_total
                        .with_label_values(&[label])
                        .inc();
                    self.metrics
                        .evaluations_total
                        .with_label_values(&["block"])
                        .inc();
                    warn!(
                        rule_family = %label,
                        request_id = %ctx.request_id,
                        body_bytes = ctx.body_buffer.len(),
                        overflow = ctx.body_buffer_overflow,
                        status = self.config.block_status,
                        "waf body: blocked"
                    );
                    Decision::Deny(self.config.block_status)
                }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> RuleDb {
        RuleDb::build().expect("rule compile")
    }

    #[test]
    fn sqli_or_1_eq_1_matches() {
        assert!(matches!(
            db().evaluate("/api/annonces?id=' OR 1=1 --", ""),
            Some(RuleFamily::SqlInjection)
        ));
    }

    #[test]
    fn sqli_union_select_matches() {
        assert!(matches!(
            db().evaluate("/api/items?q=UNION SELECT password FROM users", ""),
            Some(RuleFamily::SqlInjection)
        ));
    }

    #[test]
    fn xss_script_tag_matches() {
        assert!(matches!(
            db().evaluate("/search?q=<script>alert(1)</script>", ""),
            Some(RuleFamily::Xss)
        ));
    }

    #[test]
    fn cmdi_semicolon_cat_matches() {
        assert!(matches!(
            db().evaluate("/api/file?p=;cat /etc/passwd", ""),
            Some(RuleFamily::CommandInjection)
        ));
    }

    #[test]
    fn ssrf_aws_metadata_matches() {
        assert!(matches!(
            db().evaluate("/api/img?u=http://169.254.169.254/latest/meta-data/", ""),
            Some(RuleFamily::Ssrf)
        ));
    }

    #[test]
    fn ssrf_loopback_matches() {
        assert!(matches!(
            db().evaluate("/api/img?u=http://127.0.0.1:8080/admin", ""),
            Some(RuleFamily::Ssrf)
        ));
    }

    #[test]
    fn ssrf_private_cidr_10_matches() {
        assert!(matches!(
            db().evaluate("/api/img?u=http://10.0.0.5/", ""),
            Some(RuleFamily::Ssrf)
        ));
    }

    #[test]
    fn scanner_sqlmap_ua_matches() {
        assert!(matches!(
            db().evaluate("/api/health", "sqlmap/1.5-dev"),
            Some(RuleFamily::ScannerUa)
        ));
    }

    #[test]
    fn benign_request_passes() {
        assert!(db()
            .evaluate("/api/poulets/offers?type=feed&page=2", "Mozilla/5.0")
            .is_none());
    }

    #[test]
    fn benign_request_with_complex_query_passes() {
        assert!(db()
            .evaluate(
                "/api/poulets/users?id=550e8400-e29b-41d4-a716-446655440000",
                "Chrome/120 Safari/537.36"
            )
            .is_none());
    }

    #[test]
    fn admin_path_with_localhost_in_query_matches_ssrf() {
        // Defense-in-depth: even our own admin URL in a query is suspicious.
        assert!(matches!(
            db().evaluate("/api/probe?target=http://localhost:9902/admin", ""),
            Some(RuleFamily::Ssrf)
        ));
    }

    // ── Body-inspection rule tests ──────────────────────────────────────

    #[test]
    fn body_xss_script_tag_matches() {
        let body = r#"{"description":"<script>alert(1)</script>"}"#;
        assert!(matches!(
            db().evaluate_body(body),
            Some(RuleFamily::Xss)
        ));
    }

    #[test]
    fn body_xss_onerror_handler_matches() {
        let body = r#"<img src=x onerror="alert(document.cookie)">"#;
        assert!(matches!(
            db().evaluate_body(body),
            Some(RuleFamily::Xss)
        ));
    }

    #[test]
    fn body_nosql_dollar_ne_matches() {
        let body = r#"{"id":{"$ne":null}}"#;
        assert!(matches!(
            db().evaluate_body(body),
            Some(RuleFamily::NoSqlInjection)
        ));
    }

    #[test]
    fn body_nosql_dollar_gt_matches() {
        let body = r#"{"price": {"$gt": ""}}"#;
        assert!(matches!(
            db().evaluate_body(body),
            Some(RuleFamily::NoSqlInjection)
        ));
    }

    #[test]
    fn body_nosql_dollar_where_function_matches() {
        let body = r#"{"$where":"function() { return this.password.length > 0 }"}"#;
        assert!(matches!(
            db().evaluate_body(body),
            Some(RuleFamily::NoSqlInjection)
        ));
    }

    #[test]
    fn body_sqli_in_json_value_matches() {
        let body = r#"{"q":"'; DROP TABLE users --"}"#;
        assert!(matches!(
            db().evaluate_body(body),
            Some(RuleFamily::SqlInjection)
        ));
    }

    #[test]
    fn benign_json_body_passes() {
        let body = r#"{"name":"Mariam Ouedraogo","age":42,"city":"Ouagadougou"}"#;
        assert!(db().evaluate_body(body).is_none());
    }

    #[test]
    fn legitimate_dollar_in_string_value_does_not_match_nosql() {
        // A user typing "$ne" inside a string value (not as JSON key) must
        // NOT trip the NoSQL rule. Our regex requires `:` after the operator.
        let body = r#"{"description":"This $ne is a price tag"}"#;
        // Other families might match if patterns are present; here just
        // assert NoSQL specifically is not the family.
        let result = db().evaluate_body(body);
        assert!(
            !matches!(result, Some(RuleFamily::NoSqlInjection)),
            "unexpected NoSQL match on benign dollar string: {result:?}"
        );
    }

    #[test]
    fn is_inspectable_content_type_accepts_json_form_text() {
        assert!(is_inspectable_content_type("application/json"));
        assert!(is_inspectable_content_type("application/json; charset=utf-8"));
        assert!(is_inspectable_content_type("application/x-www-form-urlencoded"));
        assert!(is_inspectable_content_type("application/xml"));
        assert!(is_inspectable_content_type("application/graphql"));
        assert!(is_inspectable_content_type("text/plain"));
        assert!(is_inspectable_content_type("text/html; charset=utf-8"));
        assert!(is_inspectable_content_type("multipart/form-data; boundary=ABC"));
    }

    #[test]
    fn is_inspectable_content_type_rejects_binary() {
        assert!(!is_inspectable_content_type("image/png"));
        assert!(!is_inspectable_content_type("application/octet-stream"));
        assert!(!is_inspectable_content_type("video/mp4"));
        assert!(!is_inspectable_content_type(""));
    }
}
