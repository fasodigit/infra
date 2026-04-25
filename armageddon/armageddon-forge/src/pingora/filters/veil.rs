// SPDX-License-Identifier: AGPL-3.0-or-later
//! VEIL filter — response-header hygiene.
//!
//! Responsibilities:
//!
//! 1. Strip *fingerprint* headers that leak the upstream stack (`Server`,
//!    `X-Powered-By`, `X-AspNet-Version`, `X-Generator`, …).
//! 2. Inject a strict baseline of *security* headers (HSTS, CSP,
//!    X-Frame-Options, X-Content-Type-Options, Referrer-Policy,
//!    Permissions-Policy) — **without** overwriting any header the
//!    upstream already set for that same name.
//! 3. When `inject_csp_nonce = true`, mint a fresh 16-byte base64 nonce
//!    per request and substitute `{nonce}` placeholders inside the CSP
//!    header value.
//!
//! The TLS-aware rules (`Strict-Transport-Security` must only be emitted
//! over HTTPS to avoid poisoning plain-HTTP clients) are honoured via
//! [`is_request_tls`] which peeks at `X-Forwarded-Proto: https` on the
//! request (the Pingora 0.3 `Session` does not expose a stable `is_tls()`
//! helper).
//!
//! CSP nonce stash: the random nonce is stored in the typed
//! [`RequestCtx::veil_nonce`] field so that downstream HTML-rewriting filters
//! (future) can read it directly without scanning `feature_flags`.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use base64::{engine::general_purpose::STANDARD_NO_PAD as B64NP, Engine as _};
use pingora::http::ResponseHeader;

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Default fingerprint headers stripped unconditionally.
pub const DEFAULT_FINGERPRINT_STRIP: &[&str] = &[
    "server",
    "x-powered-by",
    "x-aspnet-version",
    "x-aspnetmvc-version",
    "x-generator",
];

/// Prefix retained for backward-compat references in external documentation;
/// the actual nonce is now stored in [`RequestCtx::veil_nonce`] (typed slot,
/// M1 consolidation — no longer backed by `feature_flags`).
#[deprecated(note = "use RequestCtx::veil_nonce directly")]
pub const CSP_NONCE_STASH_PREFIX: &str = "veil:nonce:";

/// How VEIL determines whether the incoming request arrived over TLS.
///
/// Controls whether `Strict-Transport-Security` is emitted. The safest
/// default is [`AlwaysOn`](TlsDetection::AlwaysOn) which assumes the
/// gateway **is** the TLS terminator (the documented ARMAGEDDON
/// architecture).  [`ForwardedProto`](TlsDetection::ForwardedProto)
/// is opt-in for deployments behind an external TLS terminator that
/// sets `X-Forwarded-Proto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsDetection {
    /// Trust the `X-Forwarded-Proto` header set by an upstream TLS
    /// terminator.  Only enable when the gateway sits behind a known
    /// trusted reverse proxy.
    ForwardedProto,
    /// Always treat every request as TLS (gateway IS the TLS terminator).
    /// This is the safest default — HSTS is always emitted.
    AlwaysOn,
    /// Never inject HSTS.  Useful for testing or plaintext-only deploys.
    AlwaysOff,
}

/// VEIL configuration.
#[derive(Debug, Clone)]
pub struct VeilConfig {
    /// Header names (case-insensitive) to strip from upstream responses.
    pub fingerprint_strip: Vec<String>,
    /// Security headers to inject if missing.  Values may contain the
    /// `{nonce}` placeholder when [`Self::inject_csp_nonce`] is enabled.
    pub security_headers: HashMap<String, String>,
    /// Generate a per-request CSP nonce and substitute `{nonce}`
    /// placeholders in [`Self::security_headers`] values.
    pub inject_csp_nonce: bool,
    /// How to detect whether the incoming request is TLS-protected.
    /// Default: [`TlsDetection::AlwaysOn`] (safest — HSTS always emitted).
    pub trusted_tls_detection: TlsDetection,
}

impl Default for VeilConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "Strict-Transport-Security".to_string(),
            "max-age=31536000; includeSubDomains; preload".to_string(),
        );
        headers.insert("X-Content-Type-Options".to_string(), "nosniff".to_string());
        headers.insert("X-Frame-Options".to_string(), "DENY".to_string());
        headers.insert(
            "Referrer-Policy".to_string(),
            "strict-origin-when-cross-origin".to_string(),
        );
        headers.insert(
            "Permissions-Policy".to_string(),
            "camera=(), microphone=(), geolocation=()".to_string(),
        );
        headers.insert(
            "Content-Security-Policy".to_string(),
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
             img-src 'self' data:; font-src 'self'; connect-src 'self'; \
             frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
                .to_string(),
        );
        Self {
            fingerprint_strip: DEFAULT_FINGERPRINT_STRIP
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            security_headers: headers,
            inject_csp_nonce: false,
            trusted_tls_detection: TlsDetection::AlwaysOn,
        }
    }
}

/// VEIL filter — response header hygiene with hot-reload.
pub struct VeilFilter {
    config: Arc<ArcSwap<VeilConfig>>,
}

impl std::fmt::Debug for VeilFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VeilFilter")
            .field("strip_count", &self.config.load().fingerprint_strip.len())
            .field(
                "security_header_count",
                &self.config.load().security_headers.len(),
            )
            .finish()
    }
}

impl VeilFilter {
    /// Build a new filter from a config.
    pub fn new(config: VeilConfig) -> Self {
        Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
        }
    }

    /// Hot-reload the config.
    pub fn update(&self, config: VeilConfig) {
        self.config.store(Arc::new(config));
        tracing::info!("pingora veil config hot-reloaded");
    }

    /// Snapshot the active config.
    pub fn snapshot(&self) -> Arc<VeilConfig> {
        self.config.load_full()
    }
}

/// Generate a fresh 16-byte base64 (no-padding) nonce.
fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    // `ring` is a workspace dep and is already pulled in by armageddon-forge.
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
        .expect("ring SecureRandom::fill failed");
    B64NP.encode(bytes)
}

/// Check whether the **incoming** request was received over TLS,
/// according to the configured [`TlsDetection`] strategy.
///
/// - [`TlsDetection::AlwaysOn`] — always returns `true` (safest default;
///   assumes the gateway terminates TLS).
/// - [`TlsDetection::AlwaysOff`] — always returns `false` (testing /
///   plaintext-only deployments).
/// - [`TlsDetection::ForwardedProto`] — inspects `X-Forwarded-Proto`.
///   Only safe when the gateway sits behind a **trusted** TLS terminator
///   that sets this header.
fn is_request_tls(session: &pingora_proxy::Session, mode: TlsDetection) -> bool {
    match mode {
        TlsDetection::AlwaysOn => true,
        TlsDetection::AlwaysOff => false,
        TlsDetection::ForwardedProto => session
            .req_header()
            .headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("https"))
            .unwrap_or(false),
    }
}

/// Case-insensitive check: does `res` already have a header under `name`?
fn has_header(res: &ResponseHeader, name: &str) -> bool {
    res.headers.contains_key(name)
}

/// Stash a CSP nonce into the typed [`RequestCtx::veil_nonce`] slot so
/// downstream filters can read it without scanning `feature_flags`.
fn stash_nonce(ctx: &mut RequestCtx, nonce: &str) {
    ctx.veil_nonce = Some(nonce.to_string());
}

#[async_trait::async_trait]
impl ForgeFilter for VeilFilter {
    fn name(&self) -> &'static str {
        "veil"
    }

    async fn on_response(
        &self,
        session: &mut pingora_proxy::Session,
        res: &mut ResponseHeader,
        ctx: &mut RequestCtx,
    ) -> Decision {
        let config = self.config.load();

        // 1. Strip fingerprint headers (case-insensitive).
        for name in &config.fingerprint_strip {
            res.remove_header(name.as_str());
        }

        // 2. Build CSP nonce if required.
        let nonce = if config.inject_csp_nonce {
            let n = generate_nonce();
            stash_nonce(ctx, &n);
            Some(n)
        } else {
            None
        };

        // 3. Inject security headers (only if upstream didn't set them).
        let tls = is_request_tls(session, config.trusted_tls_detection);
        for (name, value) in &config.security_headers {
            // HSTS only over TLS (avoid poisoning plain-HTTP clients).
            if name.eq_ignore_ascii_case("strict-transport-security") && !tls {
                continue;
            }
            if has_header(res, name) {
                continue;
            }
            let final_value = if let Some(n) = &nonce {
                value.replace("{nonce}", n)
            } else {
                value.clone()
            };
            if let Err(e) = res.insert_header(name.clone(), final_value) {
                tracing::warn!(header = %name, error = ?e, "veil: failed to insert header");
            }
        }

        Decision::Continue
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fresh_response() -> ResponseHeader {
        ResponseHeader::build(200u16, Some(16)).expect("build 200")
    }

    // --- config defaults -----------------------------------------------------

    #[test]
    fn default_config_has_expected_fingerprints() {
        let c = VeilConfig::default();
        let set: HashSet<_> = c.fingerprint_strip.iter().map(|s| s.as_str()).collect();
        assert!(set.contains("server"));
        assert!(set.contains("x-powered-by"));
        assert!(set.contains("x-aspnet-version"));
        assert!(set.contains("x-generator"));
    }

    #[test]
    fn default_config_has_expected_security_headers() {
        let c = VeilConfig::default();
        assert!(c.security_headers.contains_key("Strict-Transport-Security"));
        assert!(c.security_headers.contains_key("X-Content-Type-Options"));
        assert!(c.security_headers.contains_key("X-Frame-Options"));
        assert!(c.security_headers.contains_key("Referrer-Policy"));
        assert!(c.security_headers.contains_key("Permissions-Policy"));
        assert!(c.security_headers.contains_key("Content-Security-Policy"));
    }

    // --- strip ---------------------------------------------------------------

    #[test]
    fn strips_default_fingerprint_headers_from_upstream() {
        let mut res = fresh_response();
        res.insert_header("Server", "nginx/1.21.0").unwrap();
        res.insert_header("X-Powered-By", "Express").unwrap();
        res.insert_header("X-Generator", "Drupal 10").unwrap();
        res.insert_header("Content-Type", "text/html").unwrap();

        // Mirror the strip step performed by on_response (no session needed).
        let cfg = VeilConfig::default();
        for name in &cfg.fingerprint_strip {
            res.remove_header(name.as_str());
        }

        assert!(!has_header(&res, "server"));
        assert!(!has_header(&res, "x-powered-by"));
        assert!(!has_header(&res, "x-generator"));
        // Non-fingerprint header survives.
        assert!(has_header(&res, "content-type"));
    }

    #[test]
    fn strip_is_case_insensitive() {
        let mut res = fresh_response();
        res.insert_header("SERVER", "uppercase-ftw").unwrap();

        let cfg = VeilConfig::default();
        for name in &cfg.fingerprint_strip {
            res.remove_header(name.as_str());
        }
        assert!(!has_header(&res, "server"));
        assert!(!has_header(&res, "Server"));
        assert!(!has_header(&res, "SERVER"));
    }

    // --- injection policy ---------------------------------------------------

    #[test]
    fn does_not_overwrite_upstream_set_security_headers() {
        // Simulate what on_response does (minus the Session-dependent TLS
        // check): walk security_headers and only insert if missing.
        let mut res = fresh_response();
        let upstream_csp = "default-src 'self' https://upstream.example";
        res.insert_header("Content-Security-Policy", upstream_csp).unwrap();

        let cfg = VeilConfig::default();
        for (name, value) in &cfg.security_headers {
            if name.eq_ignore_ascii_case("strict-transport-security") {
                continue; // skip TLS-only header in this no-session test
            }
            if has_header(&res, name) {
                continue;
            }
            res.insert_header(name.clone(), value.clone()).unwrap();
        }

        // Upstream CSP must still be there.
        let got = res.headers.get("content-security-policy").unwrap();
        assert_eq!(got.to_str().unwrap(), upstream_csp);
    }

    #[test]
    fn injects_missing_security_headers() {
        let mut res = fresh_response();
        let cfg = VeilConfig::default();
        for (name, value) in &cfg.security_headers {
            if name.eq_ignore_ascii_case("strict-transport-security") {
                continue;
            }
            if has_header(&res, name) {
                continue;
            }
            res.insert_header(name.clone(), value.clone()).unwrap();
        }
        assert!(has_header(&res, "x-frame-options"));
        assert!(has_header(&res, "x-content-type-options"));
        assert!(has_header(&res, "referrer-policy"));
    }

    // --- HSTS on TLS only ---------------------------------------------------

    #[test]
    fn hsts_skipped_on_plain_http() {
        // Simulate: TLS = false path.  We only inject if tls OR header is
        // not the HSTS one.  Walk without a session.
        let mut res = fresh_response();
        let cfg = VeilConfig::default();
        let tls = false;
        for (name, value) in &cfg.security_headers {
            if name.eq_ignore_ascii_case("strict-transport-security") && !tls {
                continue;
            }
            if has_header(&res, name) {
                continue;
            }
            res.insert_header(name.clone(), value.clone()).unwrap();
        }
        assert!(!has_header(&res, "strict-transport-security"));
    }

    #[test]
    fn hsts_emitted_on_tls() {
        let mut res = fresh_response();
        let cfg = VeilConfig::default();
        let tls = true;
        for (name, value) in &cfg.security_headers {
            if name.eq_ignore_ascii_case("strict-transport-security") && !tls {
                continue;
            }
            if has_header(&res, name) {
                continue;
            }
            res.insert_header(name.clone(), value.clone()).unwrap();
        }
        assert!(has_header(&res, "strict-transport-security"));
    }

    // --- CSP nonce ----------------------------------------------------------

    #[test]
    fn nonce_is_unique_across_100_generations() {
        let mut set = HashSet::new();
        for _ in 0..100 {
            let n = generate_nonce();
            // 16 bytes → 22 chars in base64 no-pad.
            assert_eq!(n.len(), 22);
            assert!(set.insert(n));
        }
        assert_eq!(set.len(), 100);
    }

    #[test]
    fn nonce_substitution_in_csp_value() {
        let nonce = "testnonce";
        let tmpl = "default-src 'self'; script-src 'self' 'nonce-{nonce}'";
        let out = tmpl.replace("{nonce}", nonce);
        assert_eq!(
            out,
            "default-src 'self'; script-src 'self' 'nonce-testnonce'"
        );
    }

    #[test]
    fn nonce_stash_round_trip() {
        let mut ctx = RequestCtx::default();
        stash_nonce(&mut ctx, "abc");
        stash_nonce(&mut ctx, "xyz"); // overwrites
        // Uses the typed slot, not feature_flags.
        assert_eq!(ctx.veil_nonce.as_deref(), Some("xyz"));
        // feature_flags must not be polluted.
        assert!(
            ctx.feature_flags.is_empty(),
            "veil_nonce uses a typed slot, not feature_flags"
        );
    }

    // --- filter construction + hot-reload -----------------------------------

    #[test]
    fn filter_construction_and_hot_reload() {
        let filter = VeilFilter::new(VeilConfig::default());
        assert_eq!(filter.name(), "veil");
        assert!(filter
            .snapshot()
            .security_headers
            .contains_key("X-Frame-Options"));

        let mut custom = VeilConfig::default();
        custom.security_headers.clear();
        custom.security_headers.insert("X-Only".to_string(), "1".to_string());
        filter.update(custom);
        assert!(!filter
            .snapshot()
            .security_headers
            .contains_key("X-Frame-Options"));
        assert!(filter.snapshot().security_headers.contains_key("X-Only"));
    }
}
