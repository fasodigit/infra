// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

//! Response cache backed by KAYA.
//!
//! `ResponseCache` wraps an `AsyncKeyValue` store (typically `KayaClient`) and
//! implements HTTP caching semantics: ETag, `If-None-Match`, `Vary`,
//! `Cache-Control`, and TTL management.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use prometheus::{CounterVec, Opts, Registry};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use crate::error::CacheError;
use crate::key::{self, CacheKeyInput};
use crate::policy::{CacheControl, CachePolicy};
use crate::vary::{is_wildcard, parse_vary, project_vary_headers};

// -- section: backing store trait --

/// Minimal async key-value contract.  The real implementation delegates to
/// `KayaClient`; the test implementation uses an in-memory map.
#[async_trait]
pub trait AsyncKeyValue: Send + Sync {
    /// Fetch a string value by key. Returns `None` when the key does not exist
    /// or has expired.
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError>;

    /// Store a string value with a time-to-live.
    async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), CacheError>;

    /// Delete a key.
    async fn del(&self, key: &str) -> Result<(), CacheError>;
}

// -- section: KayaClient adapter --

/// Blanket adapter from `armageddon_nexus::kaya::KayaClient` to `AsyncKeyValue`.
///
/// This is the only place where `armageddon-nexus` is referenced inside this
/// crate; all other code works against the `AsyncKeyValue` trait.
pub struct KayaAdapter(pub Arc<armageddon_nexus::kaya::KayaClient>);

#[async_trait]
impl AsyncKeyValue for KayaAdapter {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        kaya_raw_get(&self.0, key).await
    }

    async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), CacheError> {
        kaya_raw_set_ex(&self.0, key, value, ttl_secs).await
    }

    async fn del(&self, key: &str) -> Result<(), CacheError> {
        kaya_raw_del(&self.0, key).await
    }
}

// -- section: KayaClient raw helpers --
//
// These free functions speak directly to KAYA using the `redis` crate's async
// commands (the same mechanism used by KayaClient internally).  We do not add
// new methods to KayaClient so that the nexus crate is not modified.

async fn kaya_raw_get(
    client: &armageddon_nexus::kaya::KayaClient,
    key: &str,
) -> Result<Option<String>, CacheError> {
    // KayaClient does not yet expose a generic GET.  For production use,
    // extend KayaClient with get/set_ex; in tests use InMemoryKv directly.
    //
    // NOTE: This adapter exists only to satisfy the type system when wiring
    // real KAYA. The unit tests use InMemoryKv and do NOT exercise this code.
    let _ = (client, key); // suppress unused-variable warnings
    Err(CacheError::Kaya(
        "KayaAdapter::get requires KayaClient generic API — use InMemoryKv for tests".to_string(),
    ))
}

async fn kaya_raw_set_ex(
    client: &armageddon_nexus::kaya::KayaClient,
    key: &str,
    value: &str,
    _ttl_secs: u64,
) -> Result<(), CacheError> {
    let _ = (client, key, value);
    Err(CacheError::Kaya(
        "KayaAdapter::set_ex requires KayaClient generic API".to_string(),
    ))
}

async fn kaya_raw_del(
    client: &armageddon_nexus::kaya::KayaClient,
    key: &str,
) -> Result<(), CacheError> {
    let _ = (client, key);
    Err(CacheError::Kaya(
        "KayaAdapter::del requires KayaClient generic API".to_string(),
    ))
}

// -- section: in-memory mock (also used by integration tests) --

/// Thread-safe in-memory key-value store used in tests without a live KAYA instance.
#[derive(Default)]
pub struct InMemoryKv {
    inner: parking_lot::RwLock<BTreeMap<String, (String, Option<std::time::Instant>)>>,
}

impl InMemoryKv {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AsyncKeyValue for InMemoryKv {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let map = self.inner.read();
        match map.get(key) {
            None => Ok(None),
            Some((value, expires_at)) => {
                if let Some(exp) = expires_at {
                    if std::time::Instant::now() > *exp {
                        return Ok(None); // expired
                    }
                }
                Ok(Some(value.clone()))
            }
        }
    }

    async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), CacheError> {
        let mut map = self.inner.write();
        let expires_at = if ttl_secs > 0 {
            Some(std::time::Instant::now() + Duration::from_secs(ttl_secs))
        } else {
            None
        };
        map.insert(key.to_string(), (value.to_string(), expires_at));
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<(), CacheError> {
        self.inner.write().remove(key);
        Ok(())
    }
}

// -- section: cached response payload --

/// The payload stored inside KAYA for one cached HTTP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPayload {
    pub status: u16,
    /// Response headers as a sorted map.
    pub headers: BTreeMap<String, String>,
    /// Response body stored as a base64 string (for JSON-safe binary transport).
    pub body_b64: String,
    /// ETag for conditional request support.
    pub etag: String,
}

/// A cached HTTP response returned to the caller.
#[derive(Debug, Clone)]
pub struct CachedResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: bytes::Bytes,
    pub etag: String,
}

// -- section: conditional response enum --

/// Result of `handle_if_none_match`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionalResponse {
    /// Client already has the current version; respond with 304 and no body.
    NotModified,
    /// Cache entry is stale or absent; forward to upstream.
    Forward,
}

// -- section: Prometheus metrics --

/// Prometheus counters for cache operations.
#[derive(Clone)]
pub struct CacheMetrics {
    pub hits: CounterVec,
    pub misses: CounterVec,
    pub evictions: CounterVec,
    pub etag_304: CounterVec,
}

impl CacheMetrics {
    fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let hits = CounterVec::new(
            Opts::new(
                "armageddon_cache_hits_total",
                "Number of cache hits by route",
            ),
            &["route"],
        )?;
        let misses = CounterVec::new(
            Opts::new(
                "armageddon_cache_misses_total",
                "Number of cache misses by route",
            ),
            &["route"],
        )?;
        let evictions = CounterVec::new(
            Opts::new(
                "armageddon_cache_evictions_total",
                "Number of cache evictions (key deletions)",
            ),
            &["route"],
        )?;
        let etag_304 = CounterVec::new(
            Opts::new(
                "armageddon_cache_etag_304_total",
                "Number of 304 Not Modified responses from ETag match",
            ),
            &["route"],
        )?;

        registry.register(Box::new(hits.clone()))?;
        registry.register(Box::new(misses.clone()))?;
        registry.register(Box::new(evictions.clone()))?;
        registry.register(Box::new(etag_304.clone()))?;

        Ok(Self { hits, misses, evictions, etag_304 })
    }
}

// -- section: ResponseCache --

/// Response cache backed by any `AsyncKeyValue` implementation (typically KAYA).
pub struct ResponseCache {
    kv: Arc<dyn AsyncKeyValue>,
    policy: CachePolicy,
    metrics: CacheMetrics,
}

impl ResponseCache {
    /// Create a `ResponseCache` backed by a concrete key-value store.
    ///
    /// `registry` is a Prometheus `Registry`; pass the global registry in
    /// production or a fresh `Registry::new()` in tests.
    pub fn new(
        kv: Arc<dyn AsyncKeyValue>,
        policy: CachePolicy,
        registry: &Registry,
    ) -> Result<Self, prometheus::Error> {
        let metrics = CacheMetrics::new(registry)?;
        Ok(Self { kv, policy, metrics })
    }

    // -- etag --

    /// Compute an ETag from a response body: truncated blake3 hex (16 chars).
    pub fn make_etag(body: &[u8]) -> String {
        let full = blake3::hash(body).to_hex();
        full[..16].to_string()
    }

    // -- conditional request --

    /// Compare the `If-None-Match` header value against the cached ETag.
    ///
    /// Returns `ConditionalResponse::NotModified` when the client's ETag
    /// matches the cached one, indicating that a 304 can be sent without
    /// forwarding to upstream.
    pub fn handle_if_none_match(
        &self,
        if_none_match: Option<&str>,
        cached_etag: &str,
    ) -> ConditionalResponse {
        match if_none_match {
            None => ConditionalResponse::Forward,
            Some(v) => {
                // Strip optional surrounding quotes and weak-validator prefix `W/`.
                let stripped = v.trim().trim_matches('"').trim_start_matches("W/").trim_matches('"');
                if stripped == cached_etag {
                    ConditionalResponse::NotModified
                } else {
                    ConditionalResponse::Forward
                }
            }
        }
    }

    // -- get --

    /// Attempt to retrieve a cached response for the given request.
    ///
    /// Returns `Ok(Some(_))` on a cache hit, `Ok(None)` on a miss.
    #[instrument(skip(self, req), fields(method = %req.method, path = %req.path))]
    pub async fn get(
        &self,
        req: &armageddon_common::types::HttpRequest,
    ) -> Result<Option<CachedResponse>, CacheError> {
        let route = req.path.clone();

        if !self.policy.is_method_cacheable(&req.method) {
            debug!(method = %req.method, "method not cacheable — bypassing cache");
            self.metrics.misses.with_label_values(&[&route]).inc();
            return Ok(None);
        }

        let cache_key = self.build_key(req, &BTreeMap::new());
        let kaya_key = key::kaya_key(&cache_key);

        match self.kv.get(&kaya_key).await? {
            None => {
                debug!(key = %kaya_key, "cache miss");
                self.metrics.misses.with_label_values(&[&route]).inc();
                Ok(None)
            }
            Some(json) => {
                let payload: CachedPayload = serde_json::from_str(&json)?;
                let body_bytes = base64_decode(&payload.body_b64)?;
                debug!(key = %kaya_key, status = payload.status, "cache hit");
                self.metrics.hits.with_label_values(&[&route]).inc();
                Ok(Some(CachedResponse {
                    status: payload.status,
                    headers: payload.headers,
                    body: bytes::Bytes::from(body_bytes),
                    etag: payload.etag,
                }))
            }
        }
    }

    // -- put --

    /// Store a response in the cache.
    ///
    /// No-ops (without error) when:
    /// - The method is not cacheable.
    /// - The status code is not cacheable.
    /// - `Cache-Control: no-store` or `private`.
    /// - The body exceeds `policy.max_body_size`.
    /// - The response carries `Vary: *`.
    #[instrument(skip(self, req, resp), fields(method = %req.method, path = %req.path, status = resp.status))]
    pub async fn put(
        &self,
        req: &armageddon_common::types::HttpRequest,
        resp: &armageddon_common::types::HttpResponse,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        if !self.policy.is_method_cacheable(&req.method) {
            debug!(method = %req.method, "method not cacheable — skipping put");
            return Ok(());
        }

        if !self.policy.is_status_cacheable(resp.status) {
            debug!(status = resp.status, "status not cacheable — skipping put");
            return Ok(());
        }

        // Parse Cache-Control from response headers.
        let cc = resp
            .headers
            .get("cache-control")
            .map(|v| CacheControl::parse(v))
            .unwrap_or_default();

        let effective_ttl = match self.policy.effective_ttl(&cc) {
            None => {
                debug!("Cache-Control prevents caching — skipping put");
                return Ok(());
            }
            Some(t) => {
                // Honour the shorter of the provided TTL and the policy-derived one.
                t.min(ttl)
            }
        };

        // Honour Vary: * — no stable key.
        if let Some(vary_val) = resp.headers.get("vary") {
            if is_wildcard(vary_val) {
                debug!("Vary: * — skipping put");
                return Ok(());
            }
        }

        let body = resp.body.as_deref().unwrap_or(&[]);

        if body.len() > self.policy.max_body_size {
            warn!(
                size = body.len(),
                limit = self.policy.max_body_size,
                "body too large for cache"
            );
            return Err(CacheError::BodyTooLarge {
                size: body.len(),
                limit: self.policy.max_body_size,
            });
        }

        // Compute vary-aware key.
        let vary_headers = self.extract_vary_headers(req, resp);
        let cache_key = self.build_key_with_vary(req, &vary_headers);
        let kaya_key = key::kaya_key(&cache_key);

        let etag = Self::make_etag(body);

        let mut headers_sorted: BTreeMap<String, String> = resp
            .headers
            .iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();
        // Always store the ETag in the cached headers.
        headers_sorted.insert("etag".to_string(), format!("\"{}\"", etag));

        let payload = CachedPayload {
            status: resp.status,
            headers: headers_sorted,
            body_b64: base64_encode(body),
            etag,
        };

        let json = serde_json::to_string(&payload)?;
        let ttl_secs = effective_ttl.as_secs().max(1);

        debug!(key = %kaya_key, ttl_secs, "storing response in cache");
        self.kv.set_ex(&kaya_key, &json, ttl_secs).await
    }

    // -- evict --

    /// Explicitly remove a cached entry by request.
    pub async fn evict(
        &self,
        req: &armageddon_common::types::HttpRequest,
    ) -> Result<(), CacheError> {
        let cache_key = self.build_key(req, &BTreeMap::new());
        let kaya_key = key::kaya_key(&cache_key);
        self.metrics
            .evictions
            .with_label_values(&[&req.path])
            .inc();
        self.kv.del(&kaya_key).await
    }

    // -- helpers --

    fn build_key(
        &self,
        req: &armageddon_common::types::HttpRequest,
        varied_headers: &BTreeMap<String, String>,
    ) -> String {
        key::compute(&CacheKeyInput {
            method: &req.method,
            path: &req.path,
            query: req.query.as_deref(),
            varied_headers: varied_headers.clone(),
        })
    }

    fn build_key_with_vary(
        &self,
        req: &armageddon_common::types::HttpRequest,
        varied_headers: &BTreeMap<String, String>,
    ) -> String {
        key::compute(&CacheKeyInput {
            method: &req.method,
            path: &req.path,
            query: req.query.as_deref(),
            varied_headers: varied_headers.clone(),
        })
    }

    fn extract_vary_headers(
        &self,
        req: &armageddon_common::types::HttpRequest,
        resp: &armageddon_common::types::HttpResponse,
    ) -> BTreeMap<String, String> {
        match resp.headers.get("vary") {
            None => BTreeMap::new(),
            Some(vary_val) => {
                let vary_names = parse_vary(vary_val);
                let req_headers_lower: BTreeMap<String, String> = req
                    .headers
                    .iter()
                    .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
                    .collect();
                project_vary_headers(&vary_names, &req_headers_lower)
            }
        }
    }
}

// -- section: base64 helpers (no external dep, uses standard alphabet) --

fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 2 < data.len() {
        let a = data[i] as usize;
        let b = data[i + 1] as usize;
        let c = data[i + 2] as usize;
        let _ = write!(
            out,
            "{}{}{}{}",
            TABLE[a >> 2] as char,
            TABLE[((a & 3) << 4) | (b >> 4)] as char,
            TABLE[((b & 0xf) << 2) | (c >> 6)] as char,
            TABLE[c & 0x3f] as char,
        );
        i += 3;
    }
    if i + 1 == data.len() {
        let a = data[i] as usize;
        let _ = write!(
            out,
            "{}{}==",
            TABLE[a >> 2] as char,
            TABLE[(a & 3) << 4] as char,
        );
    } else if i + 2 == data.len() {
        let a = data[i] as usize;
        let b = data[i + 1] as usize;
        let _ = write!(
            out,
            "{}{}{}=",
            TABLE[a >> 2] as char,
            TABLE[((a & 3) << 4) | (b >> 4)] as char,
            TABLE[(b & 0xf) << 2] as char,
        );
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, CacheError> {
    // Minimal RFC 4648 decoder — enough for our own output.
    let s = s.trim_end_matches('=');
    let s: Vec<u8> = s.bytes().collect();
    let mut out = Vec::with_capacity(s.len() * 3 / 4 + 3);

    let val = |c: u8| -> Result<u8, CacheError> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(CacheError::InvalidPayload(
                format!("invalid base64 char: {}", c as char),
            )),
        }
    };

    let mut i = 0;
    while i + 3 < s.len() {
        let a = val(s[i])?;
        let b = val(s[i + 1])?;
        let c = val(s[i + 2])?;
        let d = val(s[i + 3])?;
        out.push((a << 2) | (b >> 4));
        out.push(((b & 0xf) << 4) | (c >> 2));
        out.push(((c & 3) << 6) | d);
        i += 4;
    }
    match s.len() - i {
        2 => {
            let a = val(s[i])?;
            let b = val(s[i + 1])?;
            out.push((a << 2) | (b >> 4));
        }
        3 => {
            let a = val(s[i])?;
            let b = val(s[i + 1])?;
            let c = val(s[i + 2])?;
            out.push((a << 2) | (b >> 4));
            out.push(((b & 0xf) << 4) | (c >> 2));
        }
        0 => {}
        _ => {
            return Err(CacheError::InvalidPayload("invalid base64 length".to_string()));
        }
    }
    Ok(out)
}

// -- section: unit tests --

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::types::{HttpRequest, HttpResponse, HttpVersion};
    use prometheus::Registry;
    use std::collections::HashMap;

    fn make_registry() -> Registry {
        Registry::new()
    }

    fn make_cache() -> ResponseCache {
        let kv = Arc::new(InMemoryKv::new());
        let policy = CachePolicy::default();
        ResponseCache::new(kv, policy, &make_registry()).expect("metrics registration failed")
    }

    fn get_req(path: &str) -> HttpRequest {
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

    fn ok_resp(body: &[u8]) -> HttpResponse {
        let mut headers = HashMap::new();
        headers.insert("cache-control".to_string(), "public, max-age=60".to_string());
        HttpResponse {
            status: 200,
            headers,
            body: Some(body.to_vec()),
        }
    }

    // -- test 1: GET cacheable repeated → first miss, second hit --
    #[tokio::test]
    async fn test_get_miss_then_hit() {
        let cache = make_cache();
        let req = get_req("/api/poulets");
        let resp = ok_resp(b"hello poulets");

        // First access: miss.
        let result = cache.get(&req).await.unwrap();
        assert!(result.is_none(), "expected cache miss on first access");

        // Store it.
        cache.put(&req, &resp, Duration::from_secs(60)).await.unwrap();

        // Second access: hit.
        let result = cache.get(&req).await.unwrap();
        assert!(result.is_some(), "expected cache hit on second access");
        let cached = result.unwrap();
        assert_eq!(cached.status, 200);
        assert_eq!(cached.body.as_ref(), b"hello poulets");
    }

    // -- test 2: POST not cacheable → bypass cache --
    #[tokio::test]
    async fn test_post_not_cacheable() {
        let cache = make_cache();
        let mut req = get_req("/api/orders");
        req.method = "POST".to_string();
        let resp = ok_resp(b"created");

        cache.put(&req, &resp, Duration::from_secs(60)).await.unwrap();
        let result = cache.get(&req).await.unwrap();
        assert!(result.is_none(), "POST must not be cached");
    }

    // -- test 3: Cache-Control: no-store → no put --
    #[tokio::test]
    async fn test_no_store_skips_put() {
        let cache = make_cache();
        let req = get_req("/api/private");
        let mut resp = ok_resp(b"secret data");
        resp.headers.insert(
            "cache-control".to_string(),
            "no-store".to_string(),
        );

        cache.put(&req, &resp, Duration::from_secs(60)).await.unwrap();
        let result = cache.get(&req).await.unwrap();
        assert!(result.is_none(), "no-store must prevent caching");
    }

    // -- test 4: Vary: Accept-Language → two languages = two different keys --
    //
    // HTTP caching semantics: the KAYA key is derived at `put()` time using the
    // `Vary` names declared in the response headers.  Two requests that differ
    // only in a header listed in `Vary` therefore land in two separate KAYA entries.
    // We validate this by directly computing the keys and verifying they differ,
    // then storing both entries in the raw KV and confirming independent retrieval.
    #[tokio::test]
    async fn test_vary_two_languages_different_keys() {
        let kv = Arc::new(InMemoryKv::new());

        // Build the two cache keys that `put()` would derive.
        let mut headers_fr = BTreeMap::new();
        headers_fr.insert("accept-language".to_string(), "fr".to_string());
        let mut headers_en = BTreeMap::new();
        headers_en.insert("accept-language".to_string(), "en".to_string());

        let key_fr = key::compute(&CacheKeyInput {
            method: "GET",
            path: "/api/products",
            query: None,
            varied_headers: headers_fr,
        });
        let key_en = key::compute(&CacheKeyInput {
            method: "GET",
            path: "/api/products",
            query: None,
            varied_headers: headers_en,
        });

        // Different languages must produce different keys.
        assert_ne!(key_fr, key_en, "Vary header must differentiate cache keys");

        // Store two distinct payloads at the two different KAYA keys.
        let payload_fr = CachedPayload {
            status: 200,
            headers: BTreeMap::new(),
            body_b64: base64_encode(b"bonjour"),
            etag: ResponseCache::make_etag(b"bonjour"),
        };
        let payload_en = CachedPayload {
            status: 200,
            headers: BTreeMap::new(),
            body_b64: base64_encode(b"hello"),
            etag: ResponseCache::make_etag(b"hello"),
        };
        kv.set_ex(&key::kaya_key(&key_fr), &serde_json::to_string(&payload_fr).unwrap(), 60).await.unwrap();
        kv.set_ex(&key::kaya_key(&key_en), &serde_json::to_string(&payload_en).unwrap(), 60).await.unwrap();

        // Independent retrieval confirms the two entries don't collide.
        let val_fr = kv.get(&key::kaya_key(&key_fr)).await.unwrap().expect("fr entry missing");
        let val_en = kv.get(&key::kaya_key(&key_en)).await.unwrap().expect("en entry missing");
        assert_ne!(val_fr, val_en, "fr and en entries must be different");
        assert!(val_fr.contains("bonjour") || val_fr.contains(&base64_encode(b"bonjour")));
        assert!(val_en.contains("hello") || val_en.contains(&base64_encode(b"hello")));
    }

    // -- test 5: If-None-Match match → 304 without body --
    #[tokio::test]
    async fn test_if_none_match_returns_304() {
        let cache = make_cache();
        let body = b"stable content";
        let etag = ResponseCache::make_etag(body);

        let result = cache.handle_if_none_match(Some(&format!("\"{}\"", etag)), &etag);
        assert_eq!(result, ConditionalResponse::NotModified);
    }

    // -- test 6: If-None-Match no match → Forward --
    #[tokio::test]
    async fn test_if_none_match_mismatch_forwards() {
        let cache = make_cache();
        let result = cache.handle_if_none_match(Some("\"stale-etag-1234\""), "fresh-etag-5678");
        assert_eq!(result, ConditionalResponse::Forward);
    }

    // -- test 7: max_body_size exceeded → error, no put --
    #[tokio::test]
    async fn test_body_too_large_returns_error() {
        let kv = Arc::new(InMemoryKv::new());
        let policy = CachePolicy {
            max_body_size: 10, // tiny limit
            ..CachePolicy::default()
        };
        let cache = ResponseCache::new(kv, policy, &make_registry()).unwrap();

        let req = get_req("/api/big");
        let resp = ok_resp(b"this body is definitely longer than 10 bytes");

        let err = cache.put(&req, &resp, Duration::from_secs(60)).await;
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), CacheError::BodyTooLarge { .. }));
    }

    // -- test 8: ETag format stable (blake3 hex, 16 chars) --
    #[tokio::test]
    async fn test_etag_format() {
        let etag = ResponseCache::make_etag(b"faso digitalisation");
        assert_eq!(etag.len(), 16, "ETag must be 16 hex chars");
        assert!(etag.chars().all(|c| c.is_ascii_hexdigit()), "ETag must be hex");
        // Deterministic for same input.
        assert_eq!(etag, ResponseCache::make_etag(b"faso digitalisation"));
    }

    // -- test 9: TTL expiry → get returns None --
    #[tokio::test]
    async fn test_ttl_expiry() {
        let cache = make_cache();
        let req = get_req("/api/ephemeral");
        let resp = ok_resp(b"temporary");

        // Store with 1-second TTL.
        cache.put(&req, &resp, Duration::from_secs(1)).await.unwrap();

        // Artificially expire by writing with 0s TTL directly.
        let kv = Arc::new(InMemoryKv::new());
        let policy = CachePolicy::default();
        let cache2 = ResponseCache::new(kv.clone(), policy, &Registry::new()).unwrap();
        let req2 = get_req("/api/ephemeral2");
        let _resp2 = ok_resp(b"temporary2");

        // Force-insert with immediate expiry by using the raw InMemoryKv.
        // set_ex with ttl=1 means it will expire in 1s.
        let cache_key = key::compute(&CacheKeyInput {
            method: "GET",
            path: "/api/ephemeral2",
            query: None,
            varied_headers: BTreeMap::new(),
        });
        let kaya_key = key::kaya_key(&cache_key);
        let etag = ResponseCache::make_etag(b"temporary2");
        let payload = CachedPayload {
            status: 200,
            headers: BTreeMap::new(),
            body_b64: base64_encode(b"temporary2"),
            etag,
        };
        let json = serde_json::to_string(&payload).unwrap();
        // Write with past expiry (0s TTL maps to no expiry in InMemoryKv, so we simulate
        // expiry by checking the InMemoryKv get logic for a key with an expired Instant).
        // We test expiry by inserting directly into the map with a past timestamp.
        {
            let mut map = kv.inner.write();
            let past = std::time::Instant::now()
                .checked_sub(Duration::from_secs(10))
                .unwrap_or(std::time::Instant::now());
            map.insert(kaya_key.clone(), (json.clone(), Some(past)));
        }
        let result = cache2.get(&req2).await.unwrap();
        assert!(result.is_none(), "expired entry must return None");
    }

    // -- test 10: base64 round-trip --
    #[test]
    fn test_base64_roundtrip() {
        let original = b"ARMAGEDDON cache payload \x00\x01\x02\xff";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }
}
