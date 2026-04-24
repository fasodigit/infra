// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! PII redaction policy for [`ShadowDiffEvent`] payloads.
//!
//! # Overview
//!
//! Before a [`ShadowDiffEvent`] is persisted or forwarded to any backend the
//! dispatcher calls [`RedactionPolicy::apply`].  This replaces PII values
//! in place so no raw personal data ever reaches storage.
//!
//! # Redacted fields
//!
//! | Field | Redaction |
//! |-------|-----------|
//! | `route` | Path segments matching `path_patterns` are replaced with `<redacted>` |
//! | `headers_diff` header values | Headers in `redact_headers` have their values replaced |
//! | Query parameters in `route` | Params in `redact_query_params` get value `<redacted>` |
//! | `tenant_id` / `request_id` | HMAC-SHA256 keyed on `ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY` |
//!
//! # HMAC key policy
//!
//! When `hmac_user_ids = true`:
//!
//! - The HMAC key is read from the env var specified by `hmac_key_env`
//!   (default `ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY`).
//! - If the env var is **absent or empty** the policy is **fail-closed**:
//!   `tenant_id` and `request_id` are replaced with `<hmac-key-missing>`.
//!   This is the safe default — we never leak PII rather than silently
//!   dropping the HMAC requirement.
//! - Same HMAC key + same input → same output, enabling cross-event
//!   correlation without exposing the raw identifier.
//!
//! # Default patterns
//!
//! The `default_strict()` constructor ships with:
//!
//! | Category | Default values |
//! |----------|---------------|
//! | Path patterns | email-like, Burkina Faso phone (+226 XXXXXXXX), UUID v4 |
//! | Redact headers | `authorization`, `cookie`, `x-api-key`, `set-cookie`, `x-forwarded-for` |
//! | Redact query params | `token`, `access_token`, `api_key`, `password` |
//!
//! # Failure modes
//!
//! - **Missing HMAC key** → fail-closed: identifiers replaced with `<hmac-key-missing>`.
//! - **Regex compile error** → only possible at construction time via [`RedactionPolicy::new`].
//!   `default_strict()` uses pre-validated patterns and never panics.

use std::collections::HashSet;

use hmac::{Hmac, Mac};
use regex::Regex;
use sha2::Sha256;
use tracing::warn;

use super::shadow_sink::{HeadersDiff, ShadowDiffEvent};

/// Sentinel value placed in fields when the HMAC key is absent.
pub const HMAC_KEY_MISSING_SENTINEL: &str = "<hmac-key-missing>";

/// Sentinel value placed in path segments / header values that contain PII.
pub const REDACTED_SENTINEL: &str = "<redacted>";

// ---------------------------------------------------------------------------
// RedactionPolicy
// ---------------------------------------------------------------------------

/// Configurable PII redaction policy applied to [`ShadowDiffEvent`]s.
///
/// Construct via [`RedactionPolicy::default_strict()`] or
/// [`RedactionPolicy::new`] for custom patterns.
#[derive(Debug)]
pub struct RedactionPolicy {
    /// Path segments matching any of these regexes are replaced with
    /// [`REDACTED_SENTINEL`].
    pub path_patterns: Vec<Regex>,

    /// Header names (lower-cased) whose values are replaced with
    /// [`REDACTED_SENTINEL`].
    pub redact_headers: HashSet<String>,

    /// Query parameter names whose values are replaced with
    /// [`REDACTED_SENTINEL`].
    pub redact_query_params: HashSet<String>,

    /// If `true`, `tenant_id` and `request_id` are HMAC-SHA256 hashed.
    pub hmac_user_ids: bool,

    /// Name of the env var holding the HMAC key.  Default:
    /// `ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY`.
    pub hmac_key_env: String,
}

impl RedactionPolicy {
    /// Build from explicit compiled patterns.
    pub fn new(
        path_patterns: Vec<Regex>,
        redact_headers: HashSet<String>,
        redact_query_params: HashSet<String>,
        hmac_user_ids: bool,
        hmac_key_env: impl Into<String>,
    ) -> Self {
        Self {
            path_patterns,
            redact_headers,
            redact_query_params,
            hmac_user_ids,
            hmac_key_env: hmac_key_env.into(),
        }
    }

    /// Return a strict default policy suitable for production.
    ///
    /// Patterns cover:
    /// - RFC 5321 email addresses in path segments
    /// - Burkina Faso telephone numbers (`+226` followed by 8 digits)
    /// - UUID v4 path params
    /// - Long numeric sequences (≥ 8 digits — catches BF national ID numbers)
    ///
    /// # Panics
    ///
    /// Never — all patterns are pre-validated literals.
    pub fn default_strict() -> Self {
        // Email in path: word chars, dots, plus, hyphen before @, then domain.
        let email_pat = Regex::new(r"(?i)\b[\w.+%-]+@[\w.-]+\.[a-zA-Z]{2,}\b")
            .expect("email regex is valid");

        // Burkina Faso phone numbers: +226 XXXXXXXX or 00226XXXXXXXX
        let bf_phone_pat =
            Regex::new(r"(?:\+226|00226)\s?\d{2}\s?\d{2}\s?\d{2}\s?\d{2}")
                .expect("bf_phone regex is valid");

        // UUID v4: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        let uuid_pat = Regex::new(
            r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b",
        )
        .expect("uuid regex is valid");

        // Long numeric sequences ≥ 8 digits (national IDs, account numbers).
        let long_numeric_pat = Regex::new(r"\b\d{8,}\b").expect("long_numeric regex is valid");

        let redact_headers: HashSet<String> = [
            "authorization",
            "cookie",
            "x-api-key",
            "set-cookie",
            "x-forwarded-for",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let redact_query_params: HashSet<String> = [
            "token",
            "access_token",
            "api_key",
            "password",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            path_patterns: vec![email_pat, bf_phone_pat, uuid_pat, long_numeric_pat],
            redact_headers,
            redact_query_params,
            hmac_user_ids: true,
            hmac_key_env: "ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY".to_string(),
        }
    }

    /// Redact all PII in `event` in place.
    ///
    /// Call this BEFORE passing the event to any sink backend.
    pub fn apply(&self, event: &mut ShadowDiffEvent) {
        // 1. Redact path + query params in `route`.
        event.route = self.redact_route(&event.route);

        // 2. Redact sensitive headers.
        if let Some(hdiff) = event.headers_diff.as_mut() {
            redact_header_pairs(&mut hdiff.only_in_hyper, &self.redact_headers);
            redact_header_pairs(&mut hdiff.only_in_pingora, &self.redact_headers);
        }

        // 3. HMAC user-identifiable IDs.
        if self.hmac_user_ids {
            let key_bytes = self.hmac_key();
            event.request_id = hmac_hex(&event.request_id, &key_bytes);
            if let Some(tid) = event.tenant_id.take() {
                event.tenant_id = Some(hmac_hex(&tid, &key_bytes));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Redact PII from `route` string (path + query).
    fn redact_route(&self, route: &str) -> String {
        // Split path and query.
        let (path_part, query_part) = match route.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (route, None),
        };

        // Redact each path segment independently.
        let redacted_path: String = path_part
            .split('/')
            .map(|seg| {
                let mut s = seg.to_string();
                for pat in &self.path_patterns {
                    if pat.is_match(&s) {
                        s = REDACTED_SENTINEL.to_string();
                        break;
                    }
                }
                s
            })
            .collect::<Vec<_>>()
            .join("/");

        // Redact query params by name.
        let redacted_query = query_part.map(|q| {
            let pairs: Vec<String> = q
                .split('&')
                .map(|pair| {
                    let (name, val) = pair.split_once('=').unwrap_or((pair, ""));
                    if self.redact_query_params.contains(&name.to_lowercase()) {
                        format!("{}={}", name, REDACTED_SENTINEL)
                    } else {
                        if val.is_empty() {
                            name.to_string()
                        } else {
                            format!("{}={}", name, val)
                        }
                    }
                })
                .collect();
            pairs.join("&")
        });

        match redacted_query {
            Some(q) => format!("{}?{}", redacted_path, q),
            None => redacted_path,
        }
    }

    /// Retrieve the HMAC key bytes from the environment variable.
    ///
    /// Returns `None` if the variable is absent or empty → fail-closed.
    fn hmac_key(&self) -> Option<Vec<u8>> {
        match std::env::var(&self.hmac_key_env) {
            Ok(val) if !val.is_empty() => Some(val.into_bytes()),
            Ok(_) | Err(_) => {
                warn!(
                    env_var = %self.hmac_key_env,
                    "HMAC redaction key missing — identifiers replaced with sentinel"
                );
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// Replace values for header pairs whose name (lowercased) is in `redact_set`.
fn redact_header_pairs(pairs: &mut Vec<(String, String)>, redact_set: &HashSet<String>) {
    for (name, val) in pairs.iter_mut() {
        if redact_set.contains(&name.to_lowercase()) {
            *val = REDACTED_SENTINEL.to_string();
        }
    }
}

/// HMAC-SHA256(key, input) → hex string.
///
/// If `key_bytes` is `None` (key missing) returns [`HMAC_KEY_MISSING_SENTINEL`].
fn hmac_hex(input: &str, key_bytes: &Option<Vec<u8>>) -> String {
    match key_bytes {
        None => HMAC_KEY_MISSING_SENTINEL.to_string(),
        Some(key) => {
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(key)
                .expect("HMAC accepts any key length");
            mac.update(input.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pingora::shadow_sink::{HeadersDiff, ShadowDiffEvent};
    use std::env;

    fn base_event() -> ShadowDiffEvent {
        ShadowDiffEvent {
            timestamp_unix_ms: 0,
            request_id: "user@example.com".to_string(),
            route: "/api/users/user@example.com/profile".to_string(),
            method: "GET".to_string(),
            hyper_status: 200,
            pingora_status: 200,
            hyper_body_hash: "abc".to_string(),
            pingora_body_hash: "abc".to_string(),
            hyper_latency_ms: 1,
            pingora_latency_ms: 1,
            diverged_fields: vec![],
            headers_diff: None,
            tenant_id: Some("tenant-faso".to_string()),
        }
    }

    // ── Path redaction ────────────────────────────────────────────────────────

    #[test]
    fn email_in_path_is_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route = "/api/users/admin@faso.gov.bf/profile".to_string();
        policy.apply(&mut ev);
        assert!(
            !ev.route.contains("admin@faso.gov.bf"),
            "email must be redacted from route: {}",
            ev.route
        );
        assert!(ev.route.contains(REDACTED_SENTINEL));
    }

    #[test]
    fn bf_phone_in_path_is_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route = "/api/users/+22670123456/orders".to_string();
        policy.apply(&mut ev);
        assert!(
            !ev.route.contains("+22670123456"),
            "BF phone must be redacted: {}",
            ev.route
        );
        assert!(ev.route.contains(REDACTED_SENTINEL));
    }

    #[test]
    fn uuid_in_path_is_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route = "/api/orders/550e8400-e29b-41d4-a716-446655440000/status".to_string();
        policy.apply(&mut ev);
        assert!(
            !ev.route.contains("550e8400"),
            "UUID must be redacted: {}",
            ev.route
        );
        assert!(ev.route.contains(REDACTED_SENTINEL));
    }

    #[test]
    fn clean_path_not_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route = "/api/v1/poulets".to_string();
        let original = ev.route.clone();
        policy.apply(&mut ev);
        assert_eq!(ev.route, original, "clean path must be unchanged");
    }

    // ── Header redaction ──────────────────────────────────────────────────────

    #[test]
    fn authorization_header_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.headers_diff = Some(HeadersDiff {
            only_in_hyper: vec![
                ("authorization".to_string(), "Bearer eyJhbGc...".to_string()),
                ("content-type".to_string(), "application/json".to_string()),
            ],
            only_in_pingora: vec![("cookie".to_string(), "session=abc123".to_string())],
        });
        policy.apply(&mut ev);
        let hdiff = ev.headers_diff.unwrap();
        assert_eq!(
            hdiff.only_in_hyper[0].1, REDACTED_SENTINEL,
            "Authorization header value must be redacted"
        );
        assert_ne!(
            hdiff.only_in_hyper[1].1, REDACTED_SENTINEL,
            "content-type must not be redacted"
        );
        assert_eq!(
            hdiff.only_in_pingora[0].1, REDACTED_SENTINEL,
            "cookie header value must be redacted"
        );
    }

    // ── Query param redaction ─────────────────────────────────────────────────

    #[test]
    fn token_query_param_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route = "/api/auth/callback?token=super-secret&lang=fr".to_string();
        policy.apply(&mut ev);
        assert!(
            ev.route.contains(&format!("token={}", REDACTED_SENTINEL)),
            "token param must be redacted: {}",
            ev.route
        );
        assert!(
            ev.route.contains("lang=fr"),
            "lang param must be unchanged: {}",
            ev.route
        );
    }

    #[test]
    fn password_query_param_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route = "/api/login?username=alice&password=s3cr3t".to_string();
        policy.apply(&mut ev);
        assert!(
            ev.route.contains(&format!("password={}", REDACTED_SENTINEL)),
            "password param must be redacted: {}",
            ev.route
        );
    }

    // ── HMAC consistency ──────────────────────────────────────────────────────

    #[test]
    fn hmac_same_input_same_hash() {
        env::set_var("ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY_TEST", "test-key-42");
        let policy = RedactionPolicy::new(
            vec![],
            HashSet::new(),
            HashSet::new(),
            true,
            "ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY_TEST",
        );

        let mut ev1 = base_event();
        ev1.request_id = "user-id-abc".to_string();
        ev1.tenant_id = Some("tenant-1".to_string());

        let mut ev2 = ev1.clone();

        policy.apply(&mut ev1);
        policy.apply(&mut ev2);

        assert_eq!(
            ev1.request_id, ev2.request_id,
            "same input → same HMAC hash"
        );
        assert_eq!(ev1.tenant_id, ev2.tenant_id);
        // Must not be the original plaintext.
        assert_ne!(ev1.request_id, "user-id-abc");
    }

    #[test]
    fn hmac_different_inputs_different_hashes() {
        env::set_var("ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY_TEST2", "test-key-42");
        let policy = RedactionPolicy::new(
            vec![],
            HashSet::new(),
            HashSet::new(),
            true,
            "ARMAGEDDON_SHADOW_REDACTION_HMAC_KEY_TEST2",
        );

        let mut ev1 = base_event();
        ev1.request_id = "user-id-A".to_string();

        let mut ev2 = base_event();
        ev2.request_id = "user-id-B".to_string();

        policy.apply(&mut ev1);
        policy.apply(&mut ev2);

        assert_ne!(
            ev1.request_id, ev2.request_id,
            "different inputs → different hashes"
        );
    }

    // ── HMAC key missing → fail-closed ────────────────────────────────────────

    #[test]
    fn missing_hmac_key_fails_closed() {
        // Use a unique env var name that is definitely not set.
        let policy = RedactionPolicy::new(
            vec![],
            HashSet::new(),
            HashSet::new(),
            true,
            "ARMAGEDDON_SHADOW_HMAC_KEY_DEFINITELY_NOT_SET_XYZ",
        );

        let mut ev = base_event();
        ev.request_id = "original-request-id".to_string();
        policy.apply(&mut ev);

        assert_eq!(
            ev.request_id, HMAC_KEY_MISSING_SENTINEL,
            "missing key must use fail-closed sentinel"
        );
        assert_eq!(
            ev.tenant_id.as_deref(),
            Some(HMAC_KEY_MISSING_SENTINEL),
            "tenant_id must also use sentinel when key is missing"
        );
    }

    // ── Default-strict redact headers set ────────────────────────────────────

    #[test]
    fn default_strict_contains_expected_headers() {
        let policy = RedactionPolicy::default_strict();
        assert!(policy.redact_headers.contains("authorization"));
        assert!(policy.redact_headers.contains("cookie"));
        assert!(policy.redact_headers.contains("x-api-key"));
        assert!(policy.redact_headers.contains("set-cookie"));
        assert!(policy.redact_headers.contains("x-forwarded-for"));
    }

    // ── Redaction of route with email AND query params ────────────────────────

    #[test]
    fn route_with_email_and_token_both_redacted() {
        let policy = RedactionPolicy::default_strict();
        let mut ev = base_event();
        ev.route =
            "/api/users/bob@example.com/reset?token=abc123&lang=fr".to_string();
        policy.apply(&mut ev);
        assert!(
            !ev.route.contains("bob@example.com"),
            "email must be redacted: {}",
            ev.route
        );
        assert!(
            ev.route.contains(&format!("token={}", REDACTED_SENTINEL)),
            "token must be redacted: {}",
            ev.route
        );
        assert!(ev.route.contains("lang=fr"), "lang must survive: {}", ev.route);
    }
}
