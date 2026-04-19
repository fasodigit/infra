// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Integration test: rate limit filter wired into a minimal ForgeServer.
//!
//! Verifies the 429 path end-to-end from `RateLimitFilter` through
//! `ForgeServer::rate_limit_filter()` — without a real upstream (the
//! limit is hit before the proxy step).
//!
//! Test plan:
//! 1. Build a `ForgeServer` with `new_with_rate_limit`, rule limit = 2.
//! 2. Call `filter.check()` twice → both `Allow`.
//! 3. Third call → `Deny { retry_after_secs }`.
//! 4. Verify `retry_after_secs` is non-zero.

use armageddon_common::types::{
    HttpRequest, HttpVersion, RateLimitConfig, RateLimitFallback, RateLimitMode, RateLimitRule,
};
use armageddon_forge::ForgeServer;
use armageddon_ratelimit::RateLimitDecision;
use prometheus::Registry;
use std::collections::HashMap;

/// Minimal helpers to build a `ForgeServer` with rate limiting.

fn make_forge_with_rl(limit: u64, window_secs: u64) -> (ForgeServer, Registry) {
    use armageddon_common::types::{JwtConfig, KratosConfig};
    use armageddon_config::gateway::ExtAuthzConfig;

    let rl_cfg = RateLimitConfig {
        enabled: true,
        mode: RateLimitMode::Local,
        fallback: RateLimitFallback::FailOpen,
        shadow: false,
        rules: vec![RateLimitRule {
            descriptor: "route:/api/test".to_string(),
            requests_per_window: limit,
            window_secs,
            burst: None,
        }],
    };

    let registry = Registry::new();

    let jwt_cfg = JwtConfig {
        jwks_uri: "http://localhost/jwks".to_string(),
        issuer: "test".to_string(),
        audiences: vec!["test".to_string()],
        algorithm: "ES384".to_string(),
        cache_ttl_secs: 300,
        require_claims: vec![],
    };

    let forge = ForgeServer::new_with_rate_limit(
        vec![],
        vec![],
        vec![],
        jwt_cfg,
        KratosConfig::default(),
        vec![],
        ExtAuthzConfig::default(),
        Some(&rl_cfg),
        &registry,
    );

    (forge, registry)
}

fn make_req(path: &str) -> HttpRequest {
    HttpRequest {
        method: "GET".to_string(),
        uri: path.to_string(),
        path: path.to_string(),
        query: None,
        headers: HashMap::new(),
        body: None,
        version: HttpVersion::Http11,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// First two requests must pass; third must be denied with a Retry-After.
#[tokio::test]
async fn test_rate_limit_allow_then_deny() {
    let (forge, _reg) = make_forge_with_rl(2, 60);
    let filter = forge
        .rate_limit_filter()
        .expect("rate limit filter must be present");

    let req = make_req("/api/test");

    // Request 1 — allow
    assert_eq!(
        filter.check(&req).await,
        RateLimitDecision::Allow,
        "first request must be allowed"
    );

    // Request 2 — allow
    assert_eq!(
        filter.check(&req).await,
        RateLimitDecision::Allow,
        "second request must be allowed"
    );

    // Request 3 — deny
    let decision = filter.check(&req).await;
    match decision {
        RateLimitDecision::Deny { retry_after_secs } => {
            assert!(retry_after_secs > 0, "retry_after_secs must be positive");
        }
        other => panic!("expected Deny, got {:?}", other),
    }
}

/// When rate limiting is absent from the config (None), the filter is not built.
#[tokio::test]
async fn test_rate_limit_absent_when_disabled() {
    use armageddon_common::types::{JwtConfig, KratosConfig};
    use armageddon_config::gateway::ExtAuthzConfig;

    let jwt_cfg = JwtConfig {
        jwks_uri: "http://localhost/jwks".to_string(),
        issuer: "test".to_string(),
        audiences: vec!["test".to_string()],
        algorithm: "ES384".to_string(),
        cache_ttl_secs: 300,
        require_claims: vec![],
    };

    let forge = ForgeServer::new_with_rate_limit(
        vec![],
        vec![],
        vec![],
        jwt_cfg,
        KratosConfig::default(),
        vec![],
        ExtAuthzConfig::default(),
        None, // no rate limit config
        &Registry::new(),
    );

    assert!(
        forge.rate_limit_filter().is_none(),
        "filter must be None when config is absent"
    );
}

/// When `enabled: false`, the filter must also be absent.
#[tokio::test]
async fn test_rate_limit_absent_when_enabled_false() {
    use armageddon_common::types::{JwtConfig, KratosConfig};
    use armageddon_config::gateway::ExtAuthzConfig;

    let rl_cfg = RateLimitConfig {
        enabled: false, // explicitly disabled
        ..Default::default()
    };

    let jwt_cfg = JwtConfig {
        jwks_uri: "http://localhost/jwks".to_string(),
        issuer: "test".to_string(),
        audiences: vec!["test".to_string()],
        algorithm: "ES384".to_string(),
        cache_ttl_secs: 300,
        require_claims: vec![],
    };

    let forge = ForgeServer::new_with_rate_limit(
        vec![],
        vec![],
        vec![],
        jwt_cfg,
        KratosConfig::default(),
        vec![],
        ExtAuthzConfig::default(),
        Some(&rl_cfg),
        &Registry::new(),
    );

    assert!(
        forge.rate_limit_filter().is_none(),
        "filter must be None when enabled=false"
    );
}
