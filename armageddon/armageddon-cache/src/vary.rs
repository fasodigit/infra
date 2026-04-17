// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

//! `Vary` header handling.
//!
//! When an upstream response includes `Vary: Accept-Language, Accept-Encoding`,
//! the corresponding request headers become part of the cache key so that
//! clients with different language/encoding preferences receive the correct
//! cached copy.

use std::collections::BTreeMap;

// -- section: parsing --

/// Parse the `Vary` response header value and return the list of normalised
/// (lower-case) header names.
///
/// If the value is `"*"` an empty list is returned; callers must treat `"*"` as
/// uncacheable (no stable key can be derived).
///
/// # Example
/// ```
/// # use armageddon_cache::vary::parse_vary;
/// let names = parse_vary("Accept-Language, Accept-Encoding");
/// assert_eq!(names, vec!["accept-language", "accept-encoding"]);
/// ```
pub fn parse_vary(vary_value: &str) -> Vec<String> {
    let trimmed = vary_value.trim();
    if trimmed == "*" {
        return Vec::new();
    }
    trimmed
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Returns `true` when the `Vary` header value is `"*"`, meaning no stable
/// cache key can be derived and the response must not be stored.
pub fn is_wildcard(vary_value: &str) -> bool {
    vary_value.trim() == "*"
}

// -- section: key projection --

/// Extract the request header values relevant to the given list of `Vary` names
/// and return them as a sorted map suitable for `CacheKeyInput::varied_headers`.
///
/// `request_headers` is a map of normalised (lower-case) header names → values.
pub fn project_vary_headers(
    vary_names: &[String],
    request_headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    vary_names
        .iter()
        .filter_map(|name| {
            request_headers
                .get(name.as_str())
                .map(|v| (name.clone(), v.clone()))
        })
        .collect()
}

// -- section: tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_two_fields() {
        let names = parse_vary("Accept-Language, Accept-Encoding");
        assert_eq!(names, vec!["accept-language", "accept-encoding"]);
    }

    #[test]
    fn parse_wildcard_returns_empty() {
        assert!(parse_vary("*").is_empty());
        assert!(is_wildcard("*"));
        assert!(is_wildcard(" * "));
    }

    #[test]
    fn project_selects_only_vary_headers() {
        let vary = vec!["accept-language".to_string()];
        let mut req_headers = BTreeMap::new();
        req_headers.insert("accept-language".to_string(), "fr".to_string());
        req_headers.insert("authorization".to_string(), "Bearer xyz".to_string());

        let projected = project_vary_headers(&vary, &req_headers);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected["accept-language"], "fr");
    }

    #[test]
    fn project_missing_header_is_omitted() {
        let vary = vec!["accept-encoding".to_string()];
        let req_headers = BTreeMap::new();
        let projected = project_vary_headers(&vary, &req_headers);
        assert!(projected.is_empty());
    }
}
