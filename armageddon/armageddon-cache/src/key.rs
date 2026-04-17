// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

//! Cache key computation using blake3 hashing.
//!
//! A cache key is derived from the tuple `(method, path, query, sorted_vary_headers)`.
//! This ensures that two requests that differ only in a header listed in the upstream
//! `Vary` response header produce different cache keys.

use std::collections::BTreeMap;

// -- section: key builder --

/// Input used to compute a cache key.
#[derive(Debug)]
pub struct CacheKeyInput<'a> {
    /// HTTP method (e.g. `"GET"`).
    pub method: &'a str,
    /// Request path (e.g. `"/api/products"`).
    pub path: &'a str,
    /// Raw query string, if any (e.g. `"page=1&limit=20"`).
    pub query: Option<&'a str>,
    /// Selected request headers that affect the response according to the upstream `Vary` header.
    /// Pass an empty map when the response does not carry a `Vary` header.
    pub varied_headers: BTreeMap<String, String>,
}

/// Compute the blake3-based cache key for the given input.
///
/// The key is a lowercase hex string of 32 bytes (64 characters) and is
/// guaranteed to be stable across process restarts for the same input.
pub fn compute(input: &CacheKeyInput<'_>) -> String {
    let mut hasher = blake3::Hasher::new();

    // Feed each component separated by a null byte to prevent collisions
    // across fields of different length.
    hasher.update(input.method.as_bytes());
    hasher.update(b"\x00");
    hasher.update(input.path.as_bytes());
    hasher.update(b"\x00");
    if let Some(q) = input.query {
        hasher.update(q.as_bytes());
    }
    hasher.update(b"\x00");

    // Sort header entries so that key order is deterministic.
    for (name, value) in &input.varied_headers {
        hasher.update(name.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\x00");
    }

    hasher.finalize().to_hex().to_string()
}

/// Build the KAYA storage key for a given blake3 hash.
///
/// Pattern: `armageddon:resp:<blake3_hex>`
pub fn kaya_key(blake3_hex: &str) -> String {
    format!("armageddon:resp:{}", blake3_hex)
}

// -- section: tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_produces_same_key() {
        let input1 = CacheKeyInput {
            method: "GET",
            path: "/api/v1/poulets",
            query: Some("limit=10"),
            varied_headers: BTreeMap::new(),
        };
        let input2 = CacheKeyInput {
            method: "GET",
            path: "/api/v1/poulets",
            query: Some("limit=10"),
            varied_headers: BTreeMap::new(),
        };
        assert_eq!(compute(&input1), compute(&input2));
    }

    #[test]
    fn different_method_produces_different_key() {
        let mut a = CacheKeyInput {
            method: "GET",
            path: "/api/v1/poulets",
            query: None,
            varied_headers: BTreeMap::new(),
        };
        let b = CacheKeyInput {
            method: "HEAD",
            path: "/api/v1/poulets",
            query: None,
            varied_headers: BTreeMap::new(),
        };
        assert_ne!(compute(&a), compute(&b));
        // Change method in `a` to HEAD and verify they now match.
        a.method = "HEAD";
        assert_eq!(compute(&a), compute(&b));
    }

    #[test]
    fn varied_header_changes_key() {
        let mut headers_fr = BTreeMap::new();
        headers_fr.insert("accept-language".to_string(), "fr".to_string());

        let mut headers_en = BTreeMap::new();
        headers_en.insert("accept-language".to_string(), "en".to_string());

        let key_fr = compute(&CacheKeyInput {
            method: "GET",
            path: "/api/v1/poulets",
            query: None,
            varied_headers: headers_fr,
        });
        let key_en = compute(&CacheKeyInput {
            method: "GET",
            path: "/api/v1/poulets",
            query: None,
            varied_headers: headers_en,
        });
        assert_ne!(key_fr, key_en);
    }

    #[test]
    fn kaya_key_format() {
        let hex = "aabbccdd";
        assert_eq!(kaya_key(hex), "armageddon:resp:aabbccdd");
    }
}
