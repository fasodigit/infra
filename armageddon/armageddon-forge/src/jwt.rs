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
}
