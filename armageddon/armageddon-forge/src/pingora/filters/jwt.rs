// SPDX-License-Identifier: AGPL-3.0-or-later
//! JWT filter — ES384 Bearer-token validation with JWKS caching via KAYA.
//!
//! ## Behaviour
//!
//! On every inbound request the filter checks for an `Authorization: Bearer
//! <token>` header.
//!
//! - **Public routes** (`ctx.cluster == "public"` or the incoming request
//!   carries no `Authorization` header on a cluster flagged public): passes
//!   through with [`Decision::Continue`].
//! - **Token present and valid**: populates [`RequestCtx::user_id`],
//!   [`RequestCtx::tenant_id`], [`RequestCtx::roles`], and
//!   [`RequestCtx::bearer_token`].  Returns [`Decision::Continue`].
//! - **Token present but invalid** (bad signature, expired, unknown kid):
//!   returns [`Decision::Deny(401)`].
//! - **Token absent on a non-public route**: returns [`Decision::Deny(401)`].
//!
//! ## JWKS cache
//!
//! JWKS documents are cached keyed by `kid` in KAYA at
//! `jwt:jwks:<kid>` (TTL 300 s).  The KAYA lookup is dispatched through the
//! Pingora → tokio runtime bridge (`crate::pingora::runtime::tokio_handle()`)
//! so the async RESP3 client does not interfere with Pingora's scheduler.
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | KAYA unavailable | Fall back to in-process memory cache; if miss, fetch JWKS from auth-ms directly |
//! | JWKS endpoint unreachable | Return `Deny(401)` — fail-closed |
//! | Token expired | Return `Deny(401)` |
//! | Quorum loss (no KAYA + no auth-ms) | Return `Deny(401)` |

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

// ── public routes set ─────────────────────────────────────────────────────────

/// Cluster names that are always treated as public (no Bearer token required).
///
/// Operators add to this list at construction time via
/// [`JwtFilterConfig::public_clusters`].
const DEFAULT_PUBLIC_CLUSTERS: &[&str] = &["public", "health"];

// ── JWK types (mirrors src/jwt.rs) ────────────────────────────────────────────

/// A single JSON Web Key (EC or RSA).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
    pub kty: String,
    #[serde(default)]
    pub kid: Option<String>,
    #[serde(default)]
    pub alg: Option<String>,
    #[serde(default)]
    pub crv: Option<String>,
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
    #[serde(rename = "use", default)]
    pub use_: Option<String>,
}

/// JWKS endpoint response.
#[derive(Debug, Serialize, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<Jwk>,
}

// ── claims ────────────────────────────────────────────────────────────────────

/// JWT claims extracted after ES384 validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmageddonClaims {
    pub sub: String,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub jti: Option<String>,
    #[serde(default)]
    pub iss: Option<String>,
    #[serde(default)]
    pub aud: Option<serde_json::Value>,
    #[serde(default)]
    pub iat: Option<u64>,
    #[serde(default)]
    pub exp: Option<u64>,
}

// ── in-process JWKS cache ─────────────────────────────────────────────────────

/// In-process memory cache entry for a JWKS doc.
struct JwksCacheEntry {
    keys: Vec<Jwk>,
    fetched_at: Instant,
    ttl: Duration,
}

impl JwksCacheEntry {
    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() >= self.ttl
    }
}

// ── FlagSource — KAYA backend abstraction ────────────────────────────────────

/// Abstraction over the KAYA RESP3 backend for JWKS caching.
///
/// The real implementation dispatches via
/// `crate::pingora::runtime::tokio_handle()`.  Tests inject a
/// [`MockKayaJwtBackend`].
#[async_trait::async_trait]
pub trait KayaJwtBackend: Send + Sync + 'static {
    /// Retrieve a cached JWKS JSON string for the given key, or `None` on
    /// miss / error.
    async fn get(&self, key: &str) -> Option<String>;

    /// Store a JWKS JSON string with the given TTL seconds.
    async fn set(&self, key: &str, value: &str, ttl_secs: u64);
}

/// No-op KAYA backend — used when KAYA is not configured (falls back to
/// in-process cache only).
pub struct NoopKayaBackend;

#[async_trait::async_trait]
impl KayaJwtBackend for NoopKayaBackend {
    async fn get(&self, _key: &str) -> Option<String> {
        None
    }
    async fn set(&self, _key: &str, _value: &str, _ttl_secs: u64) {}
}

// ── configuration ─────────────────────────────────────────────────────────────

/// Configuration for [`JwtFilter`].
#[derive(Debug, Clone)]
pub struct JwtFilterConfig {
    /// JWKS URI (e.g. `http://auth-ms:8801/.well-known/jwks.json`).
    pub jwks_uri: String,
    /// Expected token issuer.
    pub issuer: String,
    /// Expected audiences.  Empty = audience validation disabled.
    pub audiences: Vec<String>,
    /// Signature algorithm.
    pub algorithm: Algorithm,
    /// JWKS TTL in seconds (default 300).
    pub jwks_ttl_secs: u64,
    /// HTTP fetch timeout.
    pub fetch_timeout: Duration,
    /// Cluster names exempt from auth (e.g. `["public", "health"]`).
    pub public_clusters: Vec<String>,
}

impl Default for JwtFilterConfig {
    fn default() -> Self {
        Self {
            jwks_uri: "http://auth-ms:8801/.well-known/jwks.json".to_string(),
            issuer: "armageddon".to_string(),
            audiences: Vec::new(),
            algorithm: Algorithm::ES384,
            jwks_ttl_secs: 300,
            fetch_timeout: Duration::from_secs(10),
            public_clusters: DEFAULT_PUBLIC_CLUSTERS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }
    }
}

// ── JwtFilter ─────────────────────────────────────────────────────────────────

/// JWT filter — ES384 validation + claims extraction.
///
/// # Thread safety
///
/// The in-process JWKS cache is protected by a `Mutex`; contention is low
/// because the cache is only written once every `jwks_ttl_secs` seconds.
///
/// # Failure modes
///
/// - **Leader loss / KAYA partition**: falls back to in-process cache; if
///   also expired, re-fetches from auth-ms directly.
/// - **auth-ms unreachable**: `Deny(401)` — fail-closed is the correct
///   posture for an authentication gate.
/// - **Token expired**: `Deny(401)`.
pub struct JwtFilter {
    config: JwtFilterConfig,
    /// In-process JWKS cache.  Keyed by `kid` (or `""` when token has no kid).
    cache: Arc<Mutex<HashMap<String, JwksCacheEntry>>>,
    /// KAYA backend for distributed JWKS cache.
    kaya: Arc<dyn KayaJwtBackend>,
}

impl std::fmt::Debug for JwtFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtFilter")
            .field("jwks_uri", &self.config.jwks_uri)
            .field("issuer", &self.config.issuer)
            .field("algorithm", &self.config.algorithm)
            .finish()
    }
}

impl JwtFilter {
    /// Build a JWT filter with the given config and KAYA backend.
    pub fn new(config: JwtFilterConfig, kaya: Arc<dyn KayaJwtBackend>) -> Self {
        Self {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
            kaya,
        }
    }

    /// Build with the default no-op KAYA backend (in-process cache only).
    pub fn new_without_kaya(config: JwtFilterConfig) -> Self {
        Self::new(config, Arc::new(NoopKayaBackend))
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Extract the Bearer token from an `Authorization` header value.
    ///
    /// Trims whitespace, checks the `Bearer ` prefix case-insensitively.
    pub fn extract_bearer(auth_header: &str) -> Option<&str> {
        let trimmed = auth_header.trim();
        if trimmed.len() > 7 && trimmed[..7].eq_ignore_ascii_case("bearer ") {
            Some(trimmed[7..].trim())
        } else {
            None
        }
    }

    /// Is the given cluster exempt from authentication?
    fn is_public(&self, cluster: &str) -> bool {
        self.config
            .public_clusters
            .iter()
            .any(|c| c == cluster)
    }

    /// Validate `token` and return extracted claims.
    ///
    /// Dispatches JWKS fetch through the tokio bridge and KAYA.
    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<HashMap<String, serde_json::Value>, JwtError> {
        // 1. Decode the JWT header to extract `kid`.
        let header =
            decode_header(token).map_err(|e| JwtError::Invalid(format!("header: {e}")))?;
        let kid = header.kid.clone().unwrap_or_default();

        // 2. Fetch JWKS (with KAYA + in-process cache).
        let jwks = self.fetch_jwks(&kid).await?;

        // 3. Find the matching key.
        let jwk = if !kid.is_empty() {
            jwks.iter()
                .find(|k| k.kid.as_deref().unwrap_or("") == kid)
                .ok_or_else(|| JwtError::Invalid(format!("no JWK for kid={kid}")))?
        } else {
            jwks.iter()
                .find(|k| k.kty == "EC")
                .ok_or_else(|| JwtError::Invalid("no EC key in JWKS".to_string()))?
        };

        // 4. Build the decoding key.
        let decoding_key = build_decoding_key(jwk)?;

        // 5. Configure validation.
        let mut validation = Validation::new(self.config.algorithm);
        validation.set_issuer(&[&self.config.issuer]);
        if !self.config.audiences.is_empty() {
            validation.set_audience(&self.config.audiences);
        }
        validation.validate_exp = true;

        // 6. Decode and verify.
        let token_data =
            decode::<HashMap<String, serde_json::Value>>(token, &decoding_key, &validation)
                .map_err(|e| match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
                    _ => JwtError::Invalid(format!("validation: {e}")),
                })?;

        Ok(token_data.claims)
    }

    /// Fetch JWKS, consulting KAYA then the in-process cache before hitting
    /// the auth-ms HTTP endpoint.
    async fn fetch_jwks(&self, kid: &str) -> Result<Vec<Jwk>, JwtError> {
        // a. In-process cache lookup (under a lock, so released immediately).
        {
            let cache = self.cache.lock().expect("jwt cache poisoned");
            if let Some(entry) = cache.get(kid) {
                if !entry.is_expired() {
                    debug!("jwt: JWKS in-process cache hit (kid={kid})");
                    return Ok(entry.keys.clone());
                }
            }
        }

        // b. Try KAYA distributed cache via the tokio bridge.
        //
        // Pattern: spawn on bridge handle, await the JoinHandle from Pingora's
        // scheduler.  We use `spawn` + JoinHandle (which impls `Future`) so
        // we never call `block_on` from inside a Pingora async hook.
        let kaya = Arc::clone(&self.kaya);
        let kaya_key = format!("jwt:jwks:{kid}");
        let handle = crate::pingora::runtime::tokio_handle();
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        let kaya_key_clone = kaya_key.clone();
        handle.spawn(async move {
            let v = kaya.get(&kaya_key_clone).await;
            let _ = tx.send(v);
        });

        if let Ok(Some(raw)) = rx.recv_timeout(Duration::from_millis(50)) {
            if let Ok(parsed) = serde_json::from_str::<JwksResponse>(&raw) {
                debug!("jwt: JWKS KAYA cache hit (kid={kid})");
                let keys = parsed.keys;
                let mut cache = self.cache.lock().expect("jwt cache poisoned");
                cache.insert(
                    kid.to_string(),
                    JwksCacheEntry {
                        keys: keys.clone(),
                        fetched_at: Instant::now(),
                        ttl: Duration::from_secs(self.config.jwks_ttl_secs),
                    },
                );
                return Ok(keys);
            }
        }

        // c. Fetch from auth-ms (synchronously dispatched through bridge).
        let uri_str = self.config.jwks_uri.clone();
        let timeout = self.config.fetch_timeout;
        let (ftx, frx) = std::sync::mpsc::channel::<Result<JwksResponse, JwtError>>();
        handle.spawn(async move {
            let result = fetch_jwks_http(&uri_str, timeout).await;
            let _ = ftx.send(result);
        });

        let jwks_resp = frx
            .recv_timeout(timeout + Duration::from_secs(1))
            .map_err(|_| JwtError::JwksFetch("JWKS fetch bridge timeout".to_string()))?
            .map_err(|e| e)?;

        let keys = jwks_resp.keys;

        // d. Populate both caches.
        if let Ok(serialized) = serde_json::to_string(&JwksResponse { keys: keys.clone() }) {
            let kaya2 = Arc::clone(&self.kaya);
            let ttl = self.config.jwks_ttl_secs;
            handle.spawn(async move {
                kaya2.set(&kaya_key, &serialized, ttl).await;
            });
        }
        {
            let mut cache = self.cache.lock().expect("jwt cache poisoned");
            cache.insert(
                kid.to_string(),
                JwksCacheEntry {
                    keys: keys.clone(),
                    fetched_at: Instant::now(),
                    ttl: Duration::from_secs(self.config.jwks_ttl_secs),
                },
            );
        }

        Ok(keys)
    }
}

// ── free helpers ──────────────────────────────────────────────────────────────

/// Build a `DecodingKey` from an EC or RSA JWK.
fn build_decoding_key(jwk: &Jwk) -> Result<DecodingKey, JwtError> {
    match jwk.kty.as_str() {
        "EC" => {
            let x = jwk
                .x
                .as_ref()
                .ok_or_else(|| JwtError::Invalid("EC JWK missing 'x'".to_string()))?;
            let y = jwk
                .y
                .as_ref()
                .ok_or_else(|| JwtError::Invalid("EC JWK missing 'y'".to_string()))?;
            DecodingKey::from_ec_components(x, y)
                .map_err(|e| JwtError::Invalid(format!("EC key: {e}")))
        }
        "RSA" => {
            let n = jwk
                .n
                .as_ref()
                .ok_or_else(|| JwtError::Invalid("RSA JWK missing 'n'".to_string()))?;
            let e = jwk
                .e
                .as_ref()
                .ok_or_else(|| JwtError::Invalid("RSA JWK missing 'e'".to_string()))?;
            DecodingKey::from_rsa_components(n, e)
                .map_err(|e| JwtError::Invalid(format!("RSA key: {e}")))
        }
        other => Err(JwtError::Invalid(format!("unsupported kty: {other}"))),
    }
}

/// HTTP fetch of a JWKS endpoint using hyper.
async fn fetch_jwks_http(uri_str: &str, timeout: Duration) -> Result<JwksResponse, JwtError> {
    let uri: hyper::Uri = uri_str
        .parse()
        .map_err(|e| JwtError::JwksFetch(format!("invalid URI: {e}")))?;

    let client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build_http();

    let req = hyper::Request::builder()
        .uri(uri)
        .header("accept", "application/json")
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .map_err(|e| JwtError::JwksFetch(format!("build request: {e}")))?;

    use http_body_util::BodyExt as _;

    let response = tokio::time::timeout(timeout, client.request(req))
        .await
        .map_err(|_| JwtError::JwksFetch("timeout".to_string()))?
        .map_err(|e| JwtError::JwksFetch(format!("HTTP: {e}")))?;

    if !response.status().is_success() {
        return Err(JwtError::JwksFetch(format!(
            "status {}",
            response.status()
        )));
    }

    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|e| JwtError::JwksFetch(format!("body: {e}")))?
        .to_bytes();

    serde_json::from_slice(&body)
        .map_err(|e| JwtError::JwksFetch(format!("parse JWKS JSON: {e}")))
}

// ── error type ────────────────────────────────────────────────────────────────

/// Internal error type for JWT validation.
#[derive(Debug)]
enum JwtError {
    Invalid(String),
    Expired,
    JwksFetch(String),
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::Invalid(m) => write!(f, "invalid JWT: {m}"),
            JwtError::Expired => write!(f, "JWT expired"),
            JwtError::JwksFetch(m) => write!(f, "JWKS fetch failed: {m}"),
        }
    }
}

// ── ForgeFilter impl ──────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl ForgeFilter for JwtFilter {
    fn name(&self) -> &'static str {
        "jwt"
    }

    /// Validate the Bearer token present in `Authorization`.
    ///
    /// Populates `ctx.user_id`, `ctx.tenant_id`, `ctx.roles`, and
    /// `ctx.bearer_token` on success.
    ///
    /// Returns `Deny(401)` when the token is invalid / expired / absent on a
    /// non-public route.
    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        let auth_header = session
            .req_header()
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        let token = match auth_header.as_deref().and_then(Self::extract_bearer) {
            Some(t) => t.to_owned(),
            None => {
                // No Bearer token.
                if self.is_public(&ctx.cluster) {
                    debug!("jwt: no token on public cluster '{}' — pass", ctx.cluster);
                    return Decision::Continue;
                }
                warn!(
                    cluster = %ctx.cluster,
                    request_id = %ctx.request_id,
                    "jwt: missing Bearer token on protected route"
                );
                return Decision::Deny(401);
            }
        };

        match self.validate_token(&token).await {
            Ok(claims) => {
                let user_id = claims
                    .get("sub")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let tenant_id = claims
                    .get("tenant_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let roles: Vec<String> = claims
                    .get("roles")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|r| r.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                debug!(
                    user_id = %user_id,
                    tenant_id = ?tenant_id,
                    roles = ?roles,
                    request_id = %ctx.request_id,
                    "jwt: token validated"
                );

                ctx.user_id = Some(user_id);
                ctx.tenant_id = tenant_id;
                ctx.roles = roles;
                ctx.bearer_token = Some(token);
                Decision::Continue
            }
            Err(e) => {
                warn!(
                    error = %e,
                    request_id = %ctx.request_id,
                    cluster = %ctx.cluster,
                    "jwt: token rejected"
                );
                Decision::Deny(401)
            }
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_bearer helper ─────────────────────────────────────────────────

    #[test]
    fn extract_bearer_standard_case() {
        assert_eq!(
            JwtFilter::extract_bearer("Bearer eyJhbGciOiJ..."),
            Some("eyJhbGciOiJ...")
        );
    }

    #[test]
    fn extract_bearer_lowercase_prefix() {
        assert_eq!(
            JwtFilter::extract_bearer("bearer eyJhbGciOiJ..."),
            Some("eyJhbGciOiJ...")
        );
    }

    #[test]
    fn extract_bearer_trims_whitespace() {
        assert_eq!(
            JwtFilter::extract_bearer("Bearer   token123  "),
            Some("token123")
        );
    }

    #[test]
    fn extract_bearer_rejects_basic() {
        assert_eq!(JwtFilter::extract_bearer("Basic dXNlcjpwYXNz"), None);
    }

    #[test]
    fn extract_bearer_rejects_empty() {
        assert_eq!(JwtFilter::extract_bearer(""), None);
    }

    #[test]
    fn extract_bearer_rejects_bare_bearer() {
        assert_eq!(JwtFilter::extract_bearer("Bearer"), None);
    }

    // ── public-cluster check ──────────────────────────────────────────────────

    #[test]
    fn is_public_returns_true_for_configured_clusters() {
        let cfg = JwtFilterConfig::default();
        let f = JwtFilter::new_without_kaya(cfg);
        assert!(f.is_public("public"));
        assert!(f.is_public("health"));
        assert!(!f.is_public("api"));
        assert!(!f.is_public("graphql"));
    }

    // ── build_decoding_key ────────────────────────────────────────────────────

    #[test]
    fn build_decoding_key_rejects_unknown_kty() {
        let jwk = Jwk {
            kty: "DH".to_string(),
            kid: None,
            alg: None,
            crv: None,
            x: None,
            y: None,
            n: None,
            e: None,
            use_: None,
        };
        assert!(matches!(build_decoding_key(&jwk), Err(JwtError::Invalid(_))));
    }

    #[test]
    fn build_decoding_key_rejects_ec_missing_x() {
        let jwk = Jwk {
            kty: "EC".to_string(),
            kid: None,
            alg: None,
            crv: Some("P-384".to_string()),
            x: None,
            y: Some("y_val".to_string()),
            n: None,
            e: None,
            use_: None,
        };
        assert!(matches!(build_decoding_key(&jwk), Err(JwtError::Invalid(_))));
    }

    // ── validate_token on garbage input ──────────────────────────────────────

    #[tokio::test]
    async fn validate_token_rejects_garbage() {
        let f = JwtFilter::new_without_kaya(JwtFilterConfig::default());
        let result = f.validate_token("not-a-jwt").await;
        assert!(matches!(result, Err(JwtError::Invalid(_))));
    }

    #[tokio::test]
    async fn validate_token_rejects_malformed_three_parts() {
        let f = JwtFilter::new_without_kaya(JwtFilterConfig::default());
        let result = f.validate_token("aaa.bbb.ccc").await;
        assert!(matches!(result, Err(JwtError::Invalid(_))));
    }

    // ── mock KAYA backend ─────────────────────────────────────────────────────

    /// A mock KAYA backend backed by a `HashMap` for unit testing.
    pub struct MockKayaJwtBackend {
        store: Mutex<HashMap<String, String>>,
    }

    impl MockKayaJwtBackend {
        pub fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }
        pub fn seed(&self, key: &str, value: &str) {
            self.store.lock().unwrap().insert(key.to_string(), value.to_string());
        }
    }

    #[async_trait::async_trait]
    impl KayaJwtBackend for MockKayaJwtBackend {
        async fn get(&self, key: &str) -> Option<String> {
            self.store.lock().unwrap().get(key).cloned()
        }
        async fn set(&self, key: &str, value: &str, _ttl: u64) {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
    }

    #[test]
    fn mock_kaya_backend_stores_and_retrieves() {
        let backend = MockKayaJwtBackend::new();
        backend.seed("jwt:jwks:kid123", r#"{"keys":[]}"#);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let val = rt.block_on(backend.get("jwt:jwks:kid123"));
        assert_eq!(val.as_deref(), Some(r#"{"keys":[]}"#));
    }

    #[test]
    fn filter_construction() {
        let f = JwtFilter::new(
            JwtFilterConfig::default(),
            Arc::new(MockKayaJwtBackend::new()),
        );
        assert_eq!(f.name(), "jwt");
        assert!(!f.config.jwks_uri.is_empty());
    }
}
