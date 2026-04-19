// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Bearer-token authentication middleware for the admin API.
//!
//! The token is compared in constant time using `subtle::ConstantTimeEq`
//! to defeat timing side channels. When no token is configured AND the
//! server is bound to a loopback address, auth is skipped (development
//! ergonomics); when the server binds on a non-loopback address the
//! constructor refuses to start without a token.

use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use subtle::ConstantTimeEq;

/// Shared auth state.
#[derive(Clone, Debug)]
pub struct AuthState {
    /// Expected bearer token (if any). When `None`, authentication is
    /// disabled — only legal on loopback binds.
    pub token: Option<Arc<str>>,
}

impl AuthState {
    /// Create an auth layer with an expected token.
    pub fn with_token(token: impl Into<Arc<str>>) -> Self {
        Self {
            token: Some(token.into()),
        }
    }

    /// Auth disabled (loopback dev only).
    pub fn disabled() -> Self {
        Self { token: None }
    }

    /// True iff authentication is required.
    pub fn enabled(&self) -> bool {
        self.token.is_some()
    }

    /// Constant-time comparison of a candidate bearer token.
    pub fn verify(&self, candidate: &str) -> bool {
        let Some(expected) = self.token.as_ref() else {
            // Disabled → always pass.
            return true;
        };
        let a = expected.as_bytes();
        let b = candidate.as_bytes();
        if a.len() != b.len() {
            // ConstantTimeEq requires equal-length slices; an up-front
            // length check leaks at most the length, which is public
            // information (server-side configuration).
            return false;
        }
        a.ct_eq(b).into()
    }
}

/// Extract the bearer token from an `Authorization: Bearer <token>` header.
fn extract_bearer<B>(req: &Request<B>) -> Option<&str> {
    let value = req.headers().get(header::AUTHORIZATION)?;
    let s = value.to_str().ok()?;
    let (scheme, token) = s.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    Some(token.trim())
}

/// Axum middleware enforcing bearer-token auth.
pub async fn bearer_auth(
    State(auth): State<Arc<AuthState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if !auth.enabled() {
        // Pass through — caller guarantees the bind is loopback.
        return Ok(next.run(req).await);
    }

    let Some(token) = extract_bearer(&req) else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if auth.verify(token) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_disabled_allows_any() {
        let state = AuthState::disabled();
        assert!(state.verify(""));
        assert!(state.verify("anything"));
    }

    #[test]
    fn verify_enabled_exact_match() {
        let state = AuthState::with_token("secret-token");
        assert!(state.verify("secret-token"));
    }

    #[test]
    fn verify_enabled_rejects_wrong_token() {
        let state = AuthState::with_token("secret-token");
        assert!(!state.verify("nope"));
        assert!(!state.verify(""));
        assert!(!state.verify("secret-token-extra"));
    }

    #[test]
    fn verify_enabled_rejects_length_mismatch() {
        let state = AuthState::with_token("abc");
        assert!(!state.verify("abcd"));
        assert!(!state.verify("ab"));
    }

    #[test]
    fn extract_bearer_happy_path() {
        let req = http::Request::builder()
            .header(header::AUTHORIZATION, "Bearer my-token")
            .body(())
            .unwrap();
        assert_eq!(extract_bearer(&req), Some("my-token"));
    }

    #[test]
    fn extract_bearer_case_insensitive_scheme() {
        let req = http::Request::builder()
            .header(header::AUTHORIZATION, "bearer my-token")
            .body(())
            .unwrap();
        assert_eq!(extract_bearer(&req), Some("my-token"));
    }

    #[test]
    fn extract_bearer_rejects_basic() {
        let req = http::Request::builder()
            .header(header::AUTHORIZATION, "Basic Zm9vOmJhcg==")
            .body(())
            .unwrap();
        assert_eq!(extract_bearer(&req), None);
    }

    #[test]
    fn extract_bearer_missing_header() {
        let req = http::Request::builder().body(()).unwrap();
        assert_eq!(extract_bearer(&req), None);
    }
}
