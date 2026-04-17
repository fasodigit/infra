// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

//! Cache policy: decides if a request/response pair may be stored and for how long.
//!
//! Parses the `Cache-Control` response header according to RFC 9111 and applies
//! the configured allow-lists for methods and status codes.

use std::time::Duration;

// -- section: configuration --

/// Configures when and how responses are cached.
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Default TTL when no `max-age` directive is present in the response.
    pub default_ttl: Duration,
    /// Maximum body size in bytes that will be stored. Larger bodies bypass the cache.
    pub max_body_size: usize,
    /// HTTP methods that are eligible for caching (e.g. `["GET", "HEAD"]`).
    pub cacheable_methods: Vec<String>,
    /// HTTP status codes that are eligible for caching (e.g. `[200, 203, 204, 301, 404]`).
    pub cacheable_status: Vec<u16>,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(60),
            max_body_size: 1024 * 1024, // 1 MiB
            cacheable_methods: vec!["GET".to_string(), "HEAD".to_string()],
            cacheable_status: vec![200, 203, 204, 301, 302, 404, 410],
        }
    }
}

// -- section: Cache-Control directive parsing --

/// Parsed `Cache-Control` directives that affect caching decisions.
#[derive(Debug, Default, Clone)]
pub struct CacheControl {
    /// `no-store` — must not cache the response.
    pub no_store: bool,
    /// `no-cache` — may cache but must revalidate on every use.
    pub no_cache: bool,
    /// `private` — must not share with intermediary caches.
    pub private: bool,
    /// `public` — explicitly marked as cacheable.
    pub public: bool,
    /// `max-age=N` in seconds (response directive).
    pub max_age: Option<u64>,
    /// `must-revalidate` — stale entries must not be served.
    pub must_revalidate: bool,
}

impl CacheControl {
    /// Parse the value of a `Cache-Control` header (may appear multiple times; pass the
    /// comma-joined value).
    ///
    /// # Example
    /// ```
    /// # use armageddon_cache::policy::CacheControl;
    /// let cc = CacheControl::parse("public, max-age=300");
    /// assert_eq!(cc.max_age, Some(300));
    /// assert!(cc.public);
    /// ```
    pub fn parse(header_value: &str) -> Self {
        let mut cc = CacheControl::default();
        for token in header_value.split(',') {
            let token = token.trim().to_ascii_lowercase();
            if token == "no-store" {
                cc.no_store = true;
            } else if token == "no-cache" {
                cc.no_cache = true;
            } else if token == "private" {
                cc.private = true;
            } else if token == "public" {
                cc.public = true;
            } else if token == "must-revalidate" {
                cc.must_revalidate = true;
            } else if let Some(rest) = token.strip_prefix("max-age=") {
                if let Ok(n) = rest.trim().parse::<u64>() {
                    cc.max_age = Some(n);
                }
            }
        }
        cc
    }
}

impl CachePolicy {
    /// Return `true` if the request method is allowed to be cached.
    pub fn is_method_cacheable(&self, method: &str) -> bool {
        self.cacheable_methods
            .iter()
            .any(|m| m.eq_ignore_ascii_case(method))
    }

    /// Return `true` if the response status code is eligible for caching.
    pub fn is_status_cacheable(&self, status: u16) -> bool {
        self.cacheable_status.contains(&status)
    }

    /// Compute the effective TTL for a response given its `Cache-Control` directives.
    ///
    /// Returns `None` when the response must not be stored.
    pub fn effective_ttl(&self, cc: &CacheControl) -> Option<Duration> {
        if cc.no_store || cc.private {
            return None;
        }
        // Honour explicit max-age when present.
        if let Some(secs) = cc.max_age {
            if secs == 0 {
                return None; // max-age=0 effectively means do-not-cache
            }
            return Some(Duration::from_secs(secs));
        }
        // Fall back to the configured default.
        Some(self.default_ttl)
    }
}

// -- section: tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_public_max_age() {
        let cc = CacheControl::parse("public, max-age=300");
        assert!(cc.public);
        assert_eq!(cc.max_age, Some(300));
        assert!(!cc.no_store);
    }

    #[test]
    fn parse_no_store() {
        let cc = CacheControl::parse("no-store, no-cache");
        assert!(cc.no_store);
        assert!(cc.no_cache);
    }

    #[test]
    fn parse_private() {
        let cc = CacheControl::parse("private, must-revalidate");
        assert!(cc.private);
        assert!(cc.must_revalidate);
        let policy = CachePolicy::default();
        assert!(policy.effective_ttl(&cc).is_none());
    }

    #[test]
    fn effective_ttl_no_store_returns_none() {
        let policy = CachePolicy::default();
        let cc = CacheControl { no_store: true, ..Default::default() };
        assert!(policy.effective_ttl(&cc).is_none());
    }

    #[test]
    fn effective_ttl_max_age_zero_returns_none() {
        let policy = CachePolicy::default();
        let cc = CacheControl { max_age: Some(0), ..Default::default() };
        assert!(policy.effective_ttl(&cc).is_none());
    }

    #[test]
    fn effective_ttl_uses_default_when_no_max_age() {
        let policy = CachePolicy::default();
        let cc = CacheControl { public: true, ..Default::default() };
        assert_eq!(policy.effective_ttl(&cc), Some(policy.default_ttl));
    }

    #[test]
    fn method_allow_list() {
        let policy = CachePolicy::default();
        assert!(policy.is_method_cacheable("GET"));
        assert!(policy.is_method_cacheable("get")); // case-insensitive
        assert!(!policy.is_method_cacheable("POST"));
        assert!(!policy.is_method_cacheable("DELETE"));
    }
}
