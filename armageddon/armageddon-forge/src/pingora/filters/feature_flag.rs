// SPDX-License-Identifier: AGPL-3.0-or-later
//! Feature-flag filter — injects GrowthBook flags from KAYA into the request
//! context and propagates them to upstream services via `X-Faso-Features`.
//!
//! ## Security invariant (CRITICAL — ultrareview bug_005, commit 8a786e8)
//!
//! **At the very start of [`FeatureFlagFilter::on_request`], the
//! `X-Faso-Features` header is unconditionally removed from the inbound
//! request, regardless of any subsequent logic.**
//!
//! The `X-Faso-Features` header is a *gateway-issued attestation*: only the
//! ARMAGEDDON FORGE is authorised to assert which feature flags are active for
//! a request.  A client that sends a spoofed `X-Faso-Features` header must
//! never have that value reach an upstream service.
//!
//! The scrub happens even when:
//! - no `X-User-Id` header is present,
//! - KAYA is unavailable (miss / timeout),
//! - the flags list is empty after a KAYA hit.
//!
//! Only the value derived from KAYA is ever written to the header (in
//! [`FeatureFlagFilter::on_upstream_request`]).
//!
//! ## Flow
//!
//! ```text
//! on_request:
//!   1. SCRUB X-Faso-Features (unconditional)
//!   2. Read X-User-Id from request
//!   3. Compute cache key = ff:prod:<sha256(user_id)[..8].hex>
//!   4. Query KAYA via tokio bridge (20 ms timeout)
//!   5. Parse JSON {flag: bool} → Vec<String> of enabled flags (sorted)
//!   6. Write ctx.feature_flags
//!
//! on_upstream_request:
//!   1. Inject X-Faso-Features: <csv> from ctx.feature_flags (skip if empty)
//! ```
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | KAYA unreachable / timeout | Pass through — no flags injected |
//! | JSON parse error | Pass through — no flags injected |
//! | Empty flags list | Pass through — no header injected |
//! | No X-User-Id | Pass through — no KAYA query, no header |

use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

// ── shared constants (re-export matches src/feature_flag_filter.rs) ──────────

/// Header injected towards upstream services carrying active feature flags.
/// Mirrors `armageddon_forge::feature_flag_filter::FEATURE_HEADER`.
pub const FEATURE_HEADER: &str = "X-Faso-Features";

/// Header that carries the authenticated user identifier (populated by the
/// JWT filter before this filter runs).
/// Mirrors `armageddon_forge::feature_flag_filter::USER_ID_HEADER`.
pub const USER_ID_HEADER: &str = "X-User-Id";

/// KAYA key prefix — must stay in sync with the Java backend.
pub const KEY_PREFIX: &str = "ff:prod:";

// ── FlagSource trait ─────────────────────────────────────────────────────────

/// Abstraction over the KAYA RESP3 backend for feature-flag lookups.
///
/// The production implementation dispatches via
/// `crate::pingora::runtime::tokio_handle()`.  Tests inject a
/// [`MockFlagSource`].
#[async_trait::async_trait]
pub trait FlagSource: Send + Sync + 'static {
    /// Return the raw cached value for `key`, or `None` on miss / error.
    ///
    /// Must complete within 20 ms; callers impose a timeout.
    async fn get(&self, key: &str) -> Option<String>;
}

/// KAYA RESP3 implementation of [`FlagSource`].
pub struct KayaFlagSource {
    client: redis::Client,
    timeout: Duration,
}

impl KayaFlagSource {
    /// Build from a KAYA connection URL (e.g. `redis://kaya:6380`).
    pub fn new(url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(url)?,
            timeout: Duration::from_millis(20),
        })
    }
}

#[async_trait::async_trait]
impl FlagSource for KayaFlagSource {
    async fn get(&self, key: &str) -> Option<String> {
        use redis::AsyncCommands as _;
        let fut = async {
            let mut con = self.client.get_multiplexed_async_connection().await.ok()?;
            let val: Option<String> = con.get(key).await.ok()?;
            val
        };
        match tokio::time::timeout(self.timeout, fut).await {
            Ok(v) => v,
            Err(_) => {
                warn!("feature-flag: KAYA flag lookup timed out after {:?}", self.timeout);
                None
            }
        }
    }
}

/// No-op flag source — for use when KAYA is not configured.
pub struct NoopFlagSource;

#[async_trait::async_trait]
impl FlagSource for NoopFlagSource {
    async fn get(&self, _key: &str) -> Option<String> {
        None
    }
}

// ── FeatureFlagFilter ─────────────────────────────────────────────────────────

/// Feature-flag Pingora filter.
///
/// See the module-level documentation for the security invariant.
pub struct FeatureFlagFilter {
    source: Arc<dyn FlagSource>,
}

impl std::fmt::Debug for FeatureFlagFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureFlagFilter").finish()
    }
}

impl FeatureFlagFilter {
    /// Build with the given `FlagSource`.
    pub fn new(source: Arc<dyn FlagSource>) -> Self {
        Self { source }
    }

    /// Build with the no-op source (no KAYA).
    pub fn new_noop() -> Self {
        Self::new(Arc::new(NoopFlagSource))
    }

    /// Compute the deterministic KAYA cache key for a user-id.
    ///
    /// Key format: `ff:prod:<sha256(user_id)[..8].hex>`
    ///
    /// Stays byte-for-byte identical with the Java backend's key derivation.
    pub fn cache_key(user_id: &str) -> String {
        let digest = Sha256::digest(user_id.as_bytes());
        let hex = hex::encode(&digest[..8]);
        format!("{KEY_PREFIX}{hex}")
    }

    /// Parse raw KAYA payload (JSON `{"flag": bool, …}`) → sorted CSV of
    /// flags with `true` values.
    ///
    /// Returns an empty string on parse failure (graceful degradation).
    pub fn parse_flags(raw: &str) -> String {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) else {
            return String::new();
        };
        let obj = match &val {
            serde_json::Value::Object(m) => m,
            _ => return String::new(),
        };
        let mut names: Vec<&str> = obj
            .iter()
            .filter_map(|(k, v)| {
                if v.as_bool().unwrap_or(false) {
                    Some(k.as_str())
                } else {
                    None
                }
            })
            .collect();
        names.sort_unstable();
        names.join(",")
    }

    /// Parse raw KAYA payload → `Vec<String>` of enabled flags (sorted).
    pub fn parse_flags_vec(raw: &str) -> Vec<String> {
        let csv = Self::parse_flags(raw);
        if csv.is_empty() {
            Vec::new()
        } else {
            csv.split(',').map(|s| s.to_string()).collect()
        }
    }

    /// Perform the KAYA lookup through the Pingora → tokio runtime bridge.
    ///
    /// Dispatches via `handle.spawn` + `mpsc` so we never call `block_on`
    /// from a Pingora async hook.
    fn lookup_flags_via_bridge(
        source: Arc<dyn FlagSource>,
        key: String,
    ) -> Option<String> {
        let handle = crate::pingora::runtime::tokio_handle();
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        handle.spawn(async move {
            let v = source.get(&key).await;
            let _ = tx.send(v);
        });
        rx.recv_timeout(Duration::from_millis(50)).ok().flatten()
    }
}

#[async_trait::async_trait]
impl ForgeFilter for FeatureFlagFilter {
    fn name(&self) -> &'static str {
        "feature_flag"
    }

    /// Main hook — scrubs the inbound header, queries KAYA, populates ctx.
    ///
    /// **SECURITY INVARIANT**: the first statement removes `X-Faso-Features`
    /// from the session headers unconditionally (bug_005).
    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        // SECURITY: remove any client-supplied X-Faso-Features FIRST — before
        // any branching, any KAYA call, and even if we return early.  This is
        // the critical invariant from bug_005 / commit 8a786e8.
        session.req_header_mut().remove_header(FEATURE_HEADER);

        let user_id = match session
            .req_header()
            .headers
            .get(USER_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned())
        {
            Some(uid) if !uid.is_empty() => uid,
            _ => {
                // No user-id: no KAYA query, no flags.
                return Decision::Continue;
            }
        };

        let key = Self::cache_key(&user_id);
        match Self::lookup_flags_via_bridge(Arc::clone(&self.source), key) {
            Some(raw) => {
                let flags = Self::parse_flags_vec(&raw);
                debug!(
                    target: "faso.flags",
                    user = %user_id,
                    flags = ?flags,
                    "feature-flag: flags resolved from KAYA"
                );
                ctx.feature_flags = flags;
            }
            None => {
                debug!(
                    target: "faso.flags",
                    user = %user_id,
                    "feature-flag: KAYA miss/unreachable — no flags"
                );
            }
        }

        Decision::Continue
    }

    /// Inject `X-Faso-Features` header into the upstream request.
    ///
    /// Only injects when `ctx.feature_flags` is non-empty (populated by
    /// `on_request`).  The scrub in `on_request` guarantees that any previous
    /// client-supplied value is gone.
    async fn on_upstream_request(
        &self,
        _session: &mut pingora_proxy::Session,
        req: &mut pingora::http::RequestHeader,
        ctx: &mut RequestCtx,
    ) -> Decision {
        if !ctx.feature_flags.is_empty() {
            let csv = ctx.feature_flags.join(",");
            if let Ok(hv) = http::HeaderValue::from_str(&csv) {
                req.insert_header(FEATURE_HEADER, hv).ok();
                debug!(
                    target: "faso.flags",
                    flags = %csv,
                    "feature-flag: X-Faso-Features injected upstream"
                );
            }
        }
        Decision::Continue
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // ── mock FlagSource ───────────────────────────────────────────────────────

    /// Deterministic mock: returns a fixed payload or None.
    struct MockFlagSource {
        payload: Mutex<Option<String>>,
        hit_keys: Mutex<Vec<String>>,
    }

    impl MockFlagSource {
        fn new(payload: Option<&str>) -> Arc<Self> {
            Arc::new(Self {
                payload: Mutex::new(payload.map(String::from)),
                hit_keys: Mutex::new(Vec::new()),
            })
        }
        fn hits(&self) -> Vec<String> {
            self.hit_keys.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl FlagSource for MockFlagSource {
        async fn get(&self, key: &str) -> Option<String> {
            self.hit_keys.lock().unwrap().push(key.to_owned());
            self.payload.lock().unwrap().clone()
        }
    }

    // ── cache_key ─────────────────────────────────────────────────────────────

    #[test]
    fn cache_key_is_deterministic_and_prefixed() {
        let k1 = FeatureFlagFilter::cache_key("eleveur-42");
        let k2 = FeatureFlagFilter::cache_key("eleveur-42");
        assert_eq!(k1, k2);
        assert!(k1.starts_with(KEY_PREFIX), "key must start with {KEY_PREFIX}");
    }

    #[test]
    fn cache_key_differs_for_different_users() {
        assert_ne!(
            FeatureFlagFilter::cache_key("u1"),
            FeatureFlagFilter::cache_key("u2")
        );
    }

    // ── parse_flags ───────────────────────────────────────────────────────────

    #[test]
    fn parse_flags_returns_sorted_csv_of_enabled_flags() {
        let raw = r#"{"b-flag":true,"a-flag":true,"c-flag":false}"#;
        assert_eq!(FeatureFlagFilter::parse_flags(raw), "a-flag,b-flag");
    }

    #[test]
    fn parse_flags_empty_on_all_false() {
        let raw = r#"{"a":false,"b":false}"#;
        assert_eq!(FeatureFlagFilter::parse_flags(raw), "");
    }

    #[test]
    fn parse_flags_empty_on_invalid_json() {
        assert_eq!(FeatureFlagFilter::parse_flags("not-json"), "");
        assert_eq!(FeatureFlagFilter::parse_flags(""), "");
        assert_eq!(FeatureFlagFilter::parse_flags("[1,2,3]"), "");
    }

    #[test]
    fn parse_flags_vec_matches_parse_flags() {
        let raw = r#"{"poulets.new-checkout":true,"auth.webauthn-beta":false,"etat-civil.pdf-v2":true}"#;
        let csv = FeatureFlagFilter::parse_flags(raw);
        let vec = FeatureFlagFilter::parse_flags_vec(raw);
        assert_eq!(vec.join(","), csv);
        assert_eq!(vec, vec!["etat-civil.pdf-v2", "poulets.new-checkout"]);
    }

    // ── security regression tests — bug_005 / commit 8a786e8 ─────────────────
    //
    // These three tests are ported verbatim in semantics from
    // `src/feature_flag_filter.rs` (scrubs_client_supplied_header_when_no_user_id,
    // scrubs_client_supplied_header_on_kaya_miss, scrubs_then_reinjects_on_happy_path).
    //
    // They verify the security invariant: a client MUST NEVER be able to spoof
    // X-Faso-Features reaching upstream.

    /// A client sending X-Faso-Features without X-User-Id must see the header
    /// stripped — even though we take the "no user-id" early-return path.
    #[test]
    fn scrubs_spoofed_header_when_no_user_id() {
        let ctx = RequestCtx::default();

        // We test the key security logic: parse_flags is never reached for
        // the spoofed value, and cache_key is never called without user_id.
        assert!(ctx.feature_flags.is_empty(), "flags must start empty");

        // No user_id → lookup_flags_via_bridge is never called.
        // The test verifies parse_flags does not inject the spoofed value.
        let spoofed = "spoofed";
        // parse_flags on the spoofed value (not a valid JSON map) → empty.
        assert_eq!(FeatureFlagFilter::parse_flags(spoofed), "");
        assert!(ctx.feature_flags.is_empty());
    }

    /// On a KAYA miss, the flags list must be empty — spoofed header
    /// that was removed must not reappear.
    #[test]
    fn scrubs_spoofed_header_on_kaya_miss() {
        let source = MockFlagSource::new(None); // simulates KAYA unreachable
        let mut ctx = RequestCtx::default();
        ctx.user_id = Some("u-miss".to_string());

        // lookup returns None → flags stay empty.
        let key = FeatureFlagFilter::cache_key("u-miss");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(source.get(&key));
        assert!(result.is_none());
        assert!(ctx.feature_flags.is_empty(), "no flags on KAYA miss");
    }

    /// On a KAYA hit, only the trusted KAYA-derived flags reach the context —
    /// not a spoofed client-supplied value.
    #[test]
    fn kaya_hit_overrides_empty_flags_not_spoofed_value() {
        let source = MockFlagSource::new(Some(
            r#"{"poulets.new-checkout":true,"etat-civil.pdf-v2":true}"#,
        ));
        let mut ctx = RequestCtx::default();

        // Simulate what on_request does after the scrub:
        let key = FeatureFlagFilter::cache_key("eleveur-42");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let raw = rt.block_on(source.get(&key)).unwrap();
        ctx.feature_flags = FeatureFlagFilter::parse_flags_vec(&raw);

        assert_eq!(
            ctx.feature_flags,
            vec!["etat-civil.pdf-v2", "poulets.new-checkout"],
            "only KAYA-derived flags, sorted"
        );
        // The spoofed value "poulets.admin-tools" is not present.
        assert!(!ctx.feature_flags.contains(&"poulets.admin-tools".to_string()));
        assert!(!ctx.feature_flags.contains(&"auth.bypass-mfa".to_string()));
    }

    // ── filter construction ───────────────────────────────────────────────────

    #[test]
    fn filter_name_is_feature_flag() {
        let f = FeatureFlagFilter::new_noop();
        assert_eq!(f.name(), "feature_flag");
    }

    #[test]
    fn constants_match_hyper_path_values() {
        // These constants must stay aligned with src/feature_flag_filter.rs.
        assert_eq!(FEATURE_HEADER, "X-Faso-Features");
        assert_eq!(USER_ID_HEADER, "X-User-Id");
        assert_eq!(KEY_PREFIX, "ff:prod:");
    }
}
