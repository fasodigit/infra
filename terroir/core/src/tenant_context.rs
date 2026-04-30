// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tenant context extraction for Axum handlers.
//!
//! Extracts `TenantContext` from the incoming request via:
//!   1. `X-Tenant-Slug` header (for M2M / internal calls without JWT)
//!   2. JWT claim `tenant_slug` (for mobile agents via Kratos JWT)
//!
//! JWT validation uses the JWKS endpoint of auth-ms (`:8801`) with a
//! 10-minute in-memory cache keyed by the key ID (`kid`) in the JWT header.
//!
//! In development, if `TERROIR_SKIP_JWT_VALIDATION=true` is set the JWT
//! signature check is skipped and claims are read as-is. **Never enable in
//! production.**

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::state::AppState;

// ---------------------------------------------------------------------------
// TenantContext
// ---------------------------------------------------------------------------

/// Context carried by every authenticated request.
#[derive(Debug, Clone)]
pub struct TenantContext {
    /// Tenant schema slug (e.g. `t_pilot`).
    pub slug: String,
    /// User UUID from JWT `sub` claim (or `"anonymous"` for M2M no-JWT path).
    pub user_id: String,
    /// Role from JWT `role` claim.
    pub role: String,
}

impl TenantContext {
    /// Returns the PostgreSQL schema name for this tenant.
    pub fn schema_name(&self) -> String {
        format!("terroir_t_{}", self.slug)
    }

    /// Returns the audit schema name.
    pub fn audit_schema_name(&self) -> String {
        format!("audit_t_{}", self.slug)
    }
}

// ---------------------------------------------------------------------------
// JWT claims
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct TerrainClaims {
    sub: String,
    #[serde(default)]
    role: String,
    tenant_slug: Option<String>,
    exp: usize,
}

// ---------------------------------------------------------------------------
// JWKS cache
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CachedKey {
    key: DecodingKey,
    cached: Instant,
}

/// In-process JWKS cache keyed by `kid`.
pub struct JwksCache {
    inner: RwLock<HashMap<String, CachedKey>>,
    jwks_uri: String,
    ttl: Duration,
}

impl JwksCache {
    pub fn new(jwks_uri: String) -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(HashMap::new()),
            jwks_uri,
            ttl: Duration::from_secs(600), // 10 min
        })
    }

    /// Retrieve or fetch the decoding key for a given `kid`.
    async fn get_key(&self, kid: &str, client: &reqwest::Client) -> anyhow::Result<DecodingKey> {
        // Fast path: check cache.
        {
            let guard = self.inner.read();
            if let Some(cached) = guard.get(kid)
                && cached.cached.elapsed() < self.ttl
            {
                return Ok(cached.key.clone());
            }
        }

        // Slow path: fetch JWKS.
        debug!(kid = kid, "JWKS cache miss — fetching {}", self.jwks_uri);
        let jwks: serde_json::Value = client.get(&self.jwks_uri).send().await?.json().await?;

        let keys = jwks["keys"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("JWKS response missing 'keys' array"))?;

        let mut guard = self.inner.write();
        for k in keys {
            if let Some(k_kid) = k["kid"].as_str()
                && let (Some(n), Some(e)) = (k["n"].as_str(), k["e"].as_str())
            {
                let decoding_key = DecodingKey::from_rsa_components(n, e)?;
                guard.insert(
                    k_kid.to_owned(),
                    CachedKey {
                        key: decoding_key,
                        cached: Instant::now(),
                    },
                );
            }
        }

        guard
            .get(kid)
            .map(|c| c.key.clone())
            .ok_or_else(|| anyhow::anyhow!("kid '{}' not found in JWKS", kid))
    }
}

// ---------------------------------------------------------------------------
// Axum extractor
// ---------------------------------------------------------------------------

impl FromRequestParts<Arc<AppState>> for TenantContext {
    type Rejection = (StatusCode, axum::Json<serde_json::Value>);

    fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        // We take owned clones of everything so the future is `'static + Send`.
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(ToOwned::to_owned);

        let tenant_slug_header = parts
            .headers
            .get("X-Tenant-Slug")
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);

        let state = state.clone();

        async move {
            let reject = |msg: &str| {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({
                        "error": "unauthorized",
                        "message": msg
                    })),
                )
            };

            if let Some(token) = auth_header {
                return extract_from_jwt_owned(token, &state)
                    .await
                    .map_err(|e| reject(&e.to_string()));
            }

            if let Some(slug) = tenant_slug_header {
                if !is_valid_slug(&slug) {
                    return Err(reject("invalid X-Tenant-Slug header"));
                }
                return Ok(TenantContext {
                    slug,
                    user_id: "anonymous".into(),
                    role: "agent".into(),
                });
            }

            Err(reject("missing Authorization header or X-Tenant-Slug"))
        }
    }
}

/// Validate JWT and extract claims.
async fn extract_from_jwt_owned(
    token: String,
    state: &Arc<AppState>,
) -> anyhow::Result<TenantContext> {
    extract_from_jwt(&token, state).await
}

/// Internal JWT validation logic.
async fn extract_from_jwt(token: &str, state: &Arc<AppState>) -> anyhow::Result<TenantContext> {
    // Dev bypass.
    if std::env::var("TERROIR_SKIP_JWT_VALIDATION")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        warn!("TERROIR_SKIP_JWT_VALIDATION=true — skipping JWT signature check (dev only)");
        let parts = token.split('.').collect::<Vec<_>>();
        if parts.len() != 3 {
            anyhow::bail!("malformed JWT");
        }
        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|e| anyhow::anyhow!("base64 decode JWT payload: {e}"))?;
        let claims: TerrainClaims = serde_json::from_slice(&payload)?;
        let slug = claims
            .tenant_slug
            .ok_or_else(|| anyhow::anyhow!("JWT missing tenant_slug claim"))?;
        return Ok(TenantContext {
            slug,
            user_id: claims.sub,
            role: claims.role,
        });
    }

    // Production path: decode header to find `kid`, then validate signature.
    let header = jsonwebtoken::decode_header(token)?;
    let kid = header
        .kid
        .ok_or_else(|| anyhow::anyhow!("JWT header missing kid"))?;

    let key = state.jwks_cache.get_key(&kid, &state.http_client).await?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&["terroir-core"]);

    let token_data = jsonwebtoken::decode::<TerrainClaims>(token, &key, &validation)?;

    let claims = token_data.claims;
    let slug = claims
        .tenant_slug
        .ok_or_else(|| anyhow::anyhow!("JWT missing tenant_slug claim"))?;

    if !is_valid_slug(&slug) {
        anyhow::bail!("invalid tenant_slug in JWT: {slug}");
    }

    Ok(TenantContext {
        slug,
        user_id: claims.sub,
        role: claims.role,
    })
}

/// Validate that a slug contains only lowercase alphanumeric + `_` chars.
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::is_valid_slug;

    #[test]
    fn valid_slugs() {
        assert!(is_valid_slug("t_pilot"));
        assert!(is_valid_slug("uph_hounde_2024"));
        assert!(is_valid_slug("coop01"));
    }

    #[test]
    fn invalid_slugs() {
        assert!(!is_valid_slug(""));
        assert!(!is_valid_slug("T_PILOT")); // uppercase not allowed
        assert!(!is_valid_slug("t-pilot")); // hyphen not allowed
        assert!(!is_valid_slug("t pilot")); // space not allowed
        assert!(!is_valid_slug(&"a".repeat(64))); // too long
    }
}
