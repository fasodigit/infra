//! JWT ES384 validation with JWKS fetching and caching.
//!
//! Fetches JWKS from auth-ms, caches for 300s, validates JWT tokens
//! using ES384 (ECDSA P-384 + SHA-384), and extracts claims.

use armageddon_common::error::{ArmageddonError, Result};
use armageddon_common::types::JwtConfig;
use dashmap::DashMap;
use http_body_util::BodyExt;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Cached JWKS entry.
struct JwksCache {
    keys: Vec<Jwk>,
    fetched_at: Instant,
    ttl_secs: u64,
}

impl JwksCache {
    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed().as_secs() >= self.ttl_secs
    }
}

/// A single JWK (JSON Web Key) -- supports EC keys for ES384.
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
    /// For RSA keys
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
    #[serde(rename = "use", default)]
    pub use_: Option<String>,
}

/// JWKS response from auth-ms.
#[derive(Debug, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<Jwk>,
}

/// JWT claims that we extract after validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmageddonClaims {
    /// Subject (user ID)
    pub sub: String,
    /// Tenant ID
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Roles
    #[serde(default)]
    pub roles: Vec<String>,
    /// JWT ID (for revocation checking)
    #[serde(default)]
    pub jti: Option<String>,
    /// Issuer
    #[serde(default)]
    pub iss: Option<String>,
    /// Audience
    #[serde(default)]
    pub aud: Option<serde_json::Value>,
    /// Issued at
    #[serde(default)]
    pub iat: Option<u64>,
    /// Expiration
    #[serde(default)]
    pub exp: Option<u64>,
}

/// Validates JWTs using ES384 with JWKS from auth-ms.
pub struct JwtValidator {
    config: JwtConfig,
    /// Cache is keyed by JWKS URI.
    cache: Arc<DashMap<String, JwksCache>>,
}

impl JwtValidator {
    pub fn new(config: JwtConfig) -> Self {
        Self {
            config,
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Extract the Bearer token from an Authorization header value.
    pub fn extract_bearer(auth_header: &str) -> Option<&str> {
        let trimmed = auth_header.trim();
        if trimmed.len() > 7 && trimmed[..7].eq_ignore_ascii_case("bearer ") {
            Some(trimmed[7..].trim())
        } else {
            None
        }
    }

    /// Validate a JWT token and return the extracted claims as a HashMap.
    ///
    /// Steps:
    /// 1. Decode header to get `kid`
    /// 2. Fetch/cache JWKS from auth-ms
    /// 3. Find matching key by `kid`
    /// 4. Verify signature with ES384
    /// 5. Validate claims (iss, aud, exp, iat)
    pub async fn validate(&self, token: &str) -> Result<HashMap<String, serde_json::Value>> {
        // 1. Decode the token header to get `kid`
        let header = decode_header(token)
            .map_err(|e| ArmageddonError::JwtInvalid(format!("invalid JWT header: {}", e)))?;

        let kid = header.kid.clone();

        // 2. Fetch JWKS (cached)
        let jwks = self.fetch_jwks().await?;

        // 3. Find the matching key
        let jwk = if let Some(ref kid_val) = kid {
            jwks.iter()
                .find(|k| k.kid.as_deref() == Some(kid_val))
                .ok_or_else(|| {
                    ArmageddonError::JwtInvalid(format!("no JWK found for kid: {}", kid_val))
                })?
        } else {
            // No kid in header: use first EC key
            jwks.iter()
                .find(|k| k.kty == "EC")
                .ok_or_else(|| {
                    ArmageddonError::JwtInvalid("no EC key found in JWKS".to_string())
                })?
        };

        // 4. Build decoding key from JWK EC coordinates
        let decoding_key = self.build_decoding_key(jwk)?;

        // 5. Set up validation
        let algorithm = match self.config.algorithm.as_str() {
            "ES384" => Algorithm::ES384,
            "ES256" => Algorithm::ES256,
            "RS256" => Algorithm::RS256,
            "RS384" => Algorithm::RS384,
            _ => Algorithm::ES384, // default
        };

        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&[&self.config.issuer]);
        if !self.config.audiences.is_empty() {
            validation.set_audience(&self.config.audiences);
        }
        validation.validate_exp = true;

        // Decode and validate
        let token_data = decode::<HashMap<String, serde_json::Value>>(
            token,
            &decoding_key,
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => ArmageddonError::JwtExpired,
            _ => ArmageddonError::JwtInvalid(format!("JWT validation failed: {}", e)),
        })?;

        // Verify required claims are present
        for claim in &self.config.require_claims {
            if !token_data.claims.contains_key(claim) {
                return Err(ArmageddonError::JwtInvalid(format!(
                    "missing required claim: {}",
                    claim
                )));
            }
        }

        Ok(token_data.claims)
    }

    /// Build a `DecodingKey` from an EC JWK.
    fn build_decoding_key(&self, jwk: &Jwk) -> Result<DecodingKey> {
        match jwk.kty.as_str() {
            "EC" => {
                let x = jwk.x.as_ref().ok_or_else(|| {
                    ArmageddonError::JwtInvalid("EC JWK missing 'x' coordinate".to_string())
                })?;
                let y = jwk.y.as_ref().ok_or_else(|| {
                    ArmageddonError::JwtInvalid("EC JWK missing 'y' coordinate".to_string())
                })?;
                DecodingKey::from_ec_components(x, y).map_err(|e| {
                    ArmageddonError::JwtInvalid(format!(
                        "failed to build EC decoding key: {}",
                        e
                    ))
                })
            }
            "RSA" => {
                let n = jwk.n.as_ref().ok_or_else(|| {
                    ArmageddonError::JwtInvalid("RSA JWK missing 'n'".to_string())
                })?;
                let e = jwk.e.as_ref().ok_or_else(|| {
                    ArmageddonError::JwtInvalid("RSA JWK missing 'e'".to_string())
                })?;
                DecodingKey::from_rsa_components(n, e).map_err(|err| {
                    ArmageddonError::JwtInvalid(format!(
                        "failed to build RSA decoding key: {}",
                        err
                    ))
                })
            }
            other => Err(ArmageddonError::JwtInvalid(format!(
                "unsupported key type: {}",
                other
            ))),
        }
    }

    /// Fetch JWKS from the configured URI, with caching.
    async fn fetch_jwks(&self) -> Result<Vec<Jwk>> {
        // Check cache first
        if let Some(cached) = self.cache.get(&self.config.jwks_uri) {
            if !cached.is_expired() {
                return Ok(cached.keys.clone());
            }
        }

        tracing::debug!("fetching JWKS from {}", self.config.jwks_uri);

        // Use hyper to fetch JWKS
        let uri: hyper::Uri = self
            .config
            .jwks_uri
            .parse()
            .map_err(|e| ArmageddonError::JwksFetchFailed(format!("invalid JWKS URI: {}", e)))?;

        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let req = hyper::Request::builder()
            .uri(&uri)
            .header("accept", "application/json")
            .body(http_body_util::Full::new(bytes::Bytes::new()))
            .map_err(|e| {
                ArmageddonError::JwksFetchFailed(format!("failed to build JWKS request: {}", e))
            })?;

        let response = tokio::time::timeout(std::time::Duration::from_secs(10), client.request(req))
            .await
            .map_err(|_| ArmageddonError::JwksFetchFailed("JWKS fetch timed out".to_string()))?
            .map_err(|e| ArmageddonError::JwksFetchFailed(format!("JWKS fetch failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ArmageddonError::JwksFetchFailed(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        let body_collected: http_body_util::Collected<bytes::Bytes> = response
            .into_body()
            .collect()
            .await
            .map_err(|e| {
                ArmageddonError::JwksFetchFailed(format!("failed to read JWKS body: {}", e))
            })?;
        let body = body_collected.to_bytes();

        let jwks: JwksResponse = serde_json::from_slice(&body).map_err(|e| {
            ArmageddonError::JwksFetchFailed(format!("failed to parse JWKS JSON: {}", e))
        })?;

        // Update cache
        self.cache.insert(
            self.config.jwks_uri.clone(),
            JwksCache {
                keys: jwks.keys.clone(),
                fetched_at: Instant::now(),
                ttl_secs: self.config.cache_ttl_secs,
            },
        );

        tracing::info!(
            "JWKS fetched and cached ({} keys, TTL {}s)",
            jwks.keys.len(),
            self.config.cache_ttl_secs,
        );

        Ok(jwks.keys)
    }
}

// ---------------------------------------------------------------------------
// Kratos Session Validator
// ---------------------------------------------------------------------------

use armageddon_common::types::KratosConfig;

/// Kratos session identity (parsed from /sessions/whoami response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KratosSession {
    pub user_id: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub tenant_id: Option<String>,
}

/// Validates Kratos session cookies against the /sessions/whoami endpoint.
pub struct KratosSessionValidator {
    config: KratosConfig,
}

impl KratosSessionValidator {
    pub fn new(config: KratosConfig) -> Self {
        Self { config }
    }

    /// Extract a specific session cookie value from a Cookie header.
    pub fn extract_session_cookie<'a>(&self, cookie_header: &'a str) -> Option<&'a str> {
        for part in cookie_header.split(';') {
            let trimmed = part.trim();
            if let Some(value) = trimmed.strip_prefix(&format!("{}=", self.config.session_cookie))
            {
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
        None
    }

    /// Validate a session by calling the Kratos whoami endpoint.
    ///
    /// Reuses the same hyper HTTP client pattern from `fetch_jwks()`.
    pub async fn validate_session(&self, cookie_header: &str) -> Result<KratosSession> {
        let uri: hyper::Uri = self.config.whoami_url.parse().map_err(|e| {
            ArmageddonError::KratosUnavailable(format!("invalid Kratos whoami URI: {}", e))
        })?;

        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let req = hyper::Request::builder()
            .uri(&uri)
            .header("cookie", cookie_header)
            .header("accept", "application/json")
            .body(http_body_util::Full::new(bytes::Bytes::new()))
            .map_err(|e| {
                ArmageddonError::KratosUnavailable(format!(
                    "failed to build Kratos request: {}",
                    e
                ))
            })?;

        let timeout_duration = std::time::Duration::from_millis(self.config.timeout_ms);
        let response = tokio::time::timeout(timeout_duration, client.request(req))
            .await
            .map_err(|_| {
                ArmageddonError::KratosUnavailable("Kratos session check timed out".to_string())
            })?
            .map_err(|e| {
                ArmageddonError::KratosUnavailable(format!("Kratos request failed: {}", e))
            })?;

        match response.status().as_u16() {
            200 => {}
            401 => {
                return Err(ArmageddonError::KratosSessionInvalid(
                    "session not authenticated".to_string(),
                ));
            }
            403 => {
                return Err(ArmageddonError::KratosSessionExpired);
            }
            status => {
                return Err(ArmageddonError::KratosUnavailable(format!(
                    "Kratos returned unexpected status {}",
                    status
                )));
            }
        }

        let body_collected: http_body_util::Collected<bytes::Bytes> =
            response.into_body().collect().await.map_err(|e| {
                ArmageddonError::KratosUnavailable(format!(
                    "failed to read Kratos response body: {}",
                    e
                ))
            })?;
        let body = body_collected.to_bytes();

        let session_json: serde_json::Value =
            serde_json::from_slice(&body).map_err(|e| {
                ArmageddonError::KratosSessionInvalid(format!(
                    "failed to parse Kratos session JSON: {}",
                    e
                ))
            })?;

        // Check that session is active
        let active = session_json
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !active {
            return Err(ArmageddonError::KratosSessionExpired);
        }

        // Extract identity fields
        let identity = session_json.get("identity").ok_or_else(|| {
            ArmageddonError::KratosSessionInvalid(
                "Kratos response missing 'identity' field".to_string(),
            )
        })?;

        let user_id = identity
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if user_id.is_empty() {
            return Err(ArmageddonError::KratosSessionInvalid(
                "Kratos identity missing 'id'".to_string(),
            ));
        }

        let traits = identity.get("traits");
        let email = traits
            .and_then(|t| t.get("email"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let roles = traits
            .and_then(|t| t.get("role"))
            .map(|v| match v {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => vec![],
            })
            .unwrap_or_default();

        let tenant_id = traits
            .and_then(|t| t.get("tenant_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        tracing::debug!(
            user_id = %user_id,
            email = ?email,
            roles = ?roles,
            "Kratos session validated"
        );

        Ok(KratosSession {
            user_id,
            email,
            roles,
            tenant_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer() {
        assert_eq!(
            JwtValidator::extract_bearer("Bearer eyJhbGciOiJ..."),
            Some("eyJhbGciOiJ...")
        );
        assert_eq!(
            JwtValidator::extract_bearer("bearer   eyJhbGciOiJ..."),
            Some("eyJhbGciOiJ...")
        );
        assert_eq!(JwtValidator::extract_bearer("Basic dXNlcjpwYXNz"), None);
        assert_eq!(JwtValidator::extract_bearer(""), None);
        assert_eq!(JwtValidator::extract_bearer("Bearer"), None);
    }

    #[tokio::test]
    async fn test_jwt_validate_invalid_token_returns_error() {
        let config = JwtConfig::default();
        let validator = JwtValidator::new(config);

        // A completely garbage token should fail at header decode
        let result = validator.validate("not-a-jwt-token").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ArmageddonError::JwtInvalid(msg) => {
                assert!(
                    msg.contains("invalid JWT header"),
                    "expected 'invalid JWT header' in error, got: {}",
                    msg
                );
            }
            other => panic!("expected JwtInvalid, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_jwt_validate_malformed_three_parts_returns_error() {
        let config = JwtConfig::default();
        let validator = JwtValidator::new(config);

        // A token with 3 parts but invalid base64 in the header
        let result = validator.validate("aaa.bbb.ccc").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_session_cookie() {
        let config = KratosConfig {
            whoami_url: "http://kratos:4433/sessions/whoami".to_string(),
            session_cookie: "ory_kratos_session".to_string(),
            timeout_ms: 3000,
            cache_ttl_secs: 60,
        };
        let validator = KratosSessionValidator::new(config);

        assert_eq!(
            validator.extract_session_cookie("ory_kratos_session=abc123; other=xyz"),
            Some("abc123")
        );
        assert_eq!(
            validator.extract_session_cookie("other=xyz; ory_kratos_session=def456"),
            Some("def456")
        );
        assert_eq!(
            validator.extract_session_cookie("other=xyz"),
            None
        );
        assert_eq!(
            validator.extract_session_cookie("ory_kratos_session="),
            None
        );
    }
}
