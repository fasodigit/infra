// SPDX-License-Identifier: AGPL-3.0-or-later
//! HTTP-backed LLM provider implementations for the ARMAGEDDON AI pipeline.
//!
//! # Providers
//!
//! | Provider | Backend | Auth |
//! |---|---|---|
//! | [`AnthropicAiProvider`] | `https://api.anthropic.com/v1/messages` | `ANTHROPIC_API_KEY` env var |
//! | [`OllamaAiProvider`] | `http://localhost:11434/api/chat` | none |
//!
//! Both providers implement the [`AiProvider`] trait defined in
//! `super::ai_adapter`.  They share the same core behaviour:
//!
//! 1. Build a compact, privacy-preserving prompt from [`RequestCtx`] fields.
//! 2. POST to the LLM endpoint with a JSON body shaped for each API.
//! 3. Parse the structured verdict (`score`, `labels`, `evidence`) from the
//!    response JSON.
//! 4. Return `None` (fail-open) on any error: timeout, HTTP error, parse
//!    failure, or rate-limit headroom exhausted.
//!
//! # Fail-open invariant
//!
//! **No LLM call ever blocks the request pipeline.**  If the provider is
//! unreachable, overloaded, or returns garbage, the adapter falls back to
//! the heuristic score already computed by the AI engine.  This is by
//! design: LLM inference is enrichment, not gate-keeping.
//!
//! # QPS limiter
//!
//! Each provider tracks the number of calls dispatched in the current
//! second using an atomic counter + timestamp.  When `max_qps` is
//! exceeded the call is skipped and `None` is returned immediately
//! (outcome label `rate_limited`).
//!
//! # LRU cache
//!
//! Responses are cached for 60 seconds keyed by
//! `blake3(tenant_id + path + method + score_bucket)`.  A request
//! that matches a cached verdict skips the HTTP round-trip entirely
//! (outcome label `cache_hit`).  Cache capacity is bounded at 1 024
//! entries to cap RAM.
//!
//! # Metrics
//!
//! | Metric | Labels | Type |
//! |---|---|---|
//! | `armageddon_ai_provider_calls_total` | `provider, outcome` | Counter |
//! | `armageddon_ai_provider_latency_seconds` | `provider` | Histogram |
//! | `armageddon_ai_provider_tokens_used_total` | `provider, direction` | Counter |
//!
//! `outcome` values: `success`, `timeout`, `error`, `cache_hit`, `rate_limited`.
//! `direction` values: `input`, `output`.
//!
//! # Failure modes
//!
//! * **API timeout**: returns `None`, increments `outcome=timeout`.
//! * **HTTP 4xx/5xx**: returns `None`, increments `outcome=error`.
//! * **Malformed JSON**: returns `None`, increments `outcome=error`.
//! * **Rate limit**: returns `None`, increments `outcome=rate_limited`.
//! * **Cache hit**: returns cached value, increments `outcome=cache_hit`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::ai_adapter::AiProvider;

// ── LRU cache (simple bounded HashMap + insertion-order eviction) ─────────────

const CACHE_CAPACITY: usize = 1024;
const CACHE_TTL_SECS: u64 = 60;

struct CacheEntry {
    value: String,
    inserted_at: Instant,
}

struct LruCache {
    map: HashMap<u64, CacheEntry>,
    capacity: usize,
}

impl LruCache {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&self, key: u64) -> Option<&str> {
        if let Some(entry) = self.map.get(&key) {
            if entry.inserted_at.elapsed().as_secs() < CACHE_TTL_SECS {
                return Some(&entry.value);
            }
        }
        None
    }

    fn insert(&mut self, key: u64, value: String) {
        if self.map.len() >= self.capacity {
            // Simple eviction: remove the first entry (non-LRU but bounded).
            if let Some(k) = self.map.keys().next().copied() {
                self.map.remove(&k);
            }
        }
        self.map.insert(
            key,
            CacheEntry {
                value,
                inserted_at: Instant::now(),
            },
        );
    }
}

// ── QPS limiter ────────────────────────────────────────────────────────────────

struct QpsLimiter {
    max_qps: u32,
    /// Window start (unix second).
    window_start: u64,
    /// Calls dispatched in the current second.
    count: u32,
}

impl QpsLimiter {
    fn new(max_qps: u32) -> Self {
        Self {
            max_qps,
            window_start: 0,
            count: 0,
        }
    }

    /// Returns `true` if the call may proceed; `false` if rate-limited.
    fn allow(&mut self) -> bool {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now_secs != self.window_start {
            self.window_start = now_secs;
            self.count = 0;
        }

        if self.count < self.max_qps {
            self.count += 1;
            true
        } else {
            false
        }
    }
}

// ── Prometheus metrics ─────────────────────────────────────────────────────────

/// Prometheus metrics for LLM provider telemetry.
///
/// Shared via `Arc` between all provider instances.
#[derive(Clone, Debug)]
pub struct LlmProviderMetrics {
    /// `armageddon_ai_provider_calls_total{provider, outcome}`
    pub calls_total: IntCounterVec,
    /// `armageddon_ai_provider_latency_seconds{provider}`
    pub latency_seconds: HistogramVec,
    /// `armageddon_ai_provider_tokens_used_total{provider, direction}`
    pub tokens_used_total: IntCounterVec,
}

impl LlmProviderMetrics {
    /// Register metrics on the given registry.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let calls_total = IntCounterVec::new(
            Opts::new(
                "armageddon_ai_provider_calls_total",
                "Total LLM provider calls by provider and outcome",
            ),
            &["provider", "outcome"],
        )?;
        registry.register(Box::new(calls_total.clone()))?;

        let latency_seconds = HistogramVec::new(
            HistogramOpts::new(
                "armageddon_ai_provider_latency_seconds",
                "LLM provider call latency in seconds",
            )
            .buckets(prometheus::exponential_buckets(0.001, 2.0, 14).unwrap()),
            &["provider"],
        )?;
        registry.register(Box::new(latency_seconds.clone()))?;

        let tokens_used_total = IntCounterVec::new(
            Opts::new(
                "armageddon_ai_provider_tokens_used_total",
                "Total LLM tokens used by provider and direction (input/output)",
            ),
            &["provider", "direction"],
        )?;
        registry.register(Box::new(tokens_used_total.clone()))?;

        Ok(Self {
            calls_total,
            latency_seconds,
            tokens_used_total,
        })
    }

    /// Convenience: register on the default prometheus registry.
    pub fn with_default_registry() -> Result<Self, prometheus::Error> {
        Self::new(prometheus::default_registry())
    }

    /// No-op metrics (for tests or when metrics are disabled).
    pub fn noop() -> Self {
        let r = Registry::new();
        Self::new(&r).expect("noop metrics registration failed")
    }
}

// ── LLM verdict ───────────────────────────────────────────────────────────────

/// Parsed verdict returned by an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmVerdict {
    /// Risk score 0.0–1.0.
    pub score: f32,
    /// Zero or more categorical labels (e.g. `["prompt_injection", "jailbreak"]`).
    #[serde(default)]
    pub labels: Vec<String>,
    /// Free-text evidence / reasoning from the model.
    #[serde(default)]
    pub evidence: String,
}

// ── cache key ─────────────────────────────────────────────────────────────────

/// Compute a 64-bit cache key from a set of request attributes.
///
/// Uses blake3 internally; truncated to 64 bits (collision risk negligible
/// at cache sizes < 10 000 entries).
fn cache_key(tenant_id: &str, path: &str, method: &str, score_bucket: u8) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(tenant_id.as_bytes());
    h.update(b"\0");
    h.update(path.as_bytes());
    h.update(b"\0");
    h.update(method.as_bytes());
    h.update(b"\0");
    h.update(&[score_bucket]);
    let digest = h.finalize();
    let bytes = digest.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

// ── Anthropic provider ────────────────────────────────────────────────────────

/// Anthropic Claude HTTP provider configuration.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// Messages endpoint.  Default: `https://api.anthropic.com/v1/messages`.
    pub endpoint: String,
    /// Environment variable that holds the API key.  Default: `ANTHROPIC_API_KEY`.
    pub api_key_env: String,
    /// Model to invoke.  Default: `claude-haiku-4-5-20251001`.
    pub model: String,
    /// HTTP call timeout in milliseconds.  Default: 2 000.
    pub timeout_ms: u64,
    /// Maximum tokens in the completion.  Default: 256.
    pub max_tokens: u32,
    /// Maximum calls per second.  Default: 10.
    pub max_qps: u32,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://api.anthropic.com/v1/messages".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            model: "claude-haiku-4-5-20251001".to_string(),
            timeout_ms: 2000,
            max_tokens: 256,
            max_qps: 10,
        }
    }
}

/// Anthropic request body (messages API).
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Anthropic response — subset we care about.
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    #[serde(default)]
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize, Default)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

/// Production Anthropic Claude provider.
///
/// # Thread safety
///
/// The LRU cache and QPS limiter are protected by a `Mutex`; contention is
/// low because calls are rate-limited to `max_qps` per second.
pub struct AnthropicAiProvider {
    config: AnthropicConfig,
    metrics: LlmProviderMetrics,
    cache: Mutex<LruCache>,
    qps: Mutex<QpsLimiter>,
}

impl AnthropicAiProvider {
    /// Create a provider with the given config and metrics.
    pub fn new(config: AnthropicConfig, metrics: LlmProviderMetrics) -> Self {
        let max_qps = config.max_qps;
        Self {
            config,
            metrics,
            cache: Mutex::new(LruCache::new(CACHE_CAPACITY)),
            qps: Mutex::new(QpsLimiter::new(max_qps)),
        }
    }

    /// Create with defaults and noop metrics (for tests / dev).
    pub fn with_defaults() -> Self {
        Self::new(AnthropicConfig::default(), LlmProviderMetrics::noop())
    }

    fn provider_name(&self) -> &'static str {
        "anthropic"
    }

    /// Build the user prompt for the security analysis request.
    fn build_prompt(
        &self,
        user_id: Option<&str>,
        path: &str,
        score: f32,
    ) -> String {
        let masked_uid = user_id
            .map(|id| {
                if id.len() > 4 {
                    format!("{}****", &id[..4])
                } else {
                    "****".to_string()
                }
            })
            .unwrap_or_else(|| "anonymous".to_string());

        format!(
            "Analyse the following HTTP request for security threats. \
             Respond ONLY with a JSON object containing exactly these fields: \
             {{\"score\": <float 0.0-1.0>, \"labels\": [<string>, ...], \"evidence\": \"<string>\"}}. \
             Score 0.0 = benign, 1.0 = confirmed attack. \
             Request summary: user={masked_uid} path={path} heuristic_score={score:.2}. \
             Focus on: prompt injection, jailbreak, command injection, path traversal, SSRF."
        )
    }

    /// Dispatch the HTTP call synchronously via the tokio bridge.
    ///
    /// This method is called from `contextualise`, which is synchronous.
    /// We bridge through the forge tokio runtime, wait with a timeout, and
    /// return `None` on any failure.
    fn call_sync(
        &self,
        user_id: Option<&str>,
        path: &str,
        score: f32,
    ) -> Option<AnthropicResponse> {
        let api_key = match std::env::var(&self.config.api_key_env) {
            Ok(k) if !k.is_empty() => k,
            _ => {
                debug!(
                    api_key_env = %self.config.api_key_env,
                    "anthropic: API key env var not set or empty — fail-open"
                );
                return None;
            }
        };

        let prompt = self.build_prompt(user_id, path, score);
        let body = AnthropicRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt,
            }],
        };
        let body_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "anthropic: failed to serialise request body");
                return None;
            }
        };

        let endpoint = self.config.endpoint.clone();
        let timeout_ms = self.config.timeout_ms;

        let handle = crate::pingora::runtime::tokio_handle();
        let (tx, rx) = std::sync::mpsc::channel::<Result<AnthropicResponse, String>>();

        handle.spawn(async move {
            let result = call_anthropic_http(&endpoint, &api_key, body_bytes, timeout_ms).await;
            let _ = tx.send(result);
        });

        let deadline = Duration::from_millis(timeout_ms + 200);
        match rx.recv_timeout(deadline) {
            Ok(Ok(resp)) => Some(resp),
            Ok(Err(e)) => {
                warn!(error = %e, "anthropic: HTTP call failed");
                None
            }
            Err(_) => {
                warn!("anthropic: bridge timeout waiting for HTTP response");
                None
            }
        }
    }
}

impl AiProvider for AnthropicAiProvider {
    fn contextualise(&self, user_id: Option<&str>, path: &str, score: f32) -> Option<String> {
        let provider = self.provider_name();

        // ── Cache lookup ──────────────────────────────────────────────────────
        let tenant_bucket = (score * 10.0) as u8; // bucket: 0–10
        let key = cache_key(
            user_id.unwrap_or(""),
            path,
            "", // method not available in this sig; use empty string
            tenant_bucket,
        );

        {
            let cache = self.cache.lock().expect("llm cache poisoned");
            if let Some(cached) = cache.get(key) {
                debug!(provider, "LLM cache hit");
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "cache_hit"])
                    .inc();
                return Some(cached.to_owned());
            }
        }

        // ── QPS gate ─────────────────────────────────────────────────────────
        {
            let mut qps = self.qps.lock().expect("qps limiter poisoned");
            if !qps.allow() {
                debug!(provider, "LLM QPS limit exceeded — fail-open");
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "rate_limited"])
                    .inc();
                return None;
            }
        }

        // ── HTTP call with latency measurement ────────────────────────────────
        let t0 = Instant::now();
        let result = self.call_sync(user_id, path, score);
        let elapsed = t0.elapsed().as_secs_f64();

        self.metrics
            .latency_seconds
            .with_label_values(&[provider])
            .observe(elapsed);

        match result {
            Some(resp) => {
                // Record token usage.
                self.metrics
                    .tokens_used_total
                    .with_label_values(&[provider, "input"])
                    .inc_by(resp.usage.input_tokens);
                self.metrics
                    .tokens_used_total
                    .with_label_values(&[provider, "output"])
                    .inc_by(resp.usage.output_tokens);

                // Extract the text from the first content block.
                let text = resp
                    .content
                    .into_iter()
                    .find(|c| c.content_type == "text")
                    .map(|c| c.text)
                    .unwrap_or_default();

                // Try to parse the JSON verdict from the model response.
                let verdict_str = extract_verdict_json(&text);
                match serde_json::from_str::<LlmVerdict>(&verdict_str) {
                    Ok(v) => {
                        let summary = format!(
                            "score={:.2} labels={:?} evidence={}",
                            v.score, v.labels, v.evidence
                        );
                        debug!(provider, %summary, "LLM verdict parsed");
                        self.metrics
                            .calls_total
                            .with_label_values(&[provider, "success"])
                            .inc();
                        // Populate cache.
                        let mut cache = self.cache.lock().expect("llm cache poisoned");
                        cache.insert(key, summary.clone());
                        Some(summary)
                    }
                    Err(e) => {
                        warn!(error = %e, raw = %text, "anthropic: failed to parse LLM JSON verdict — fail-open");
                        self.metrics
                            .calls_total
                            .with_label_values(&[provider, "error"])
                            .inc();
                        None
                    }
                }
            }
            None => {
                // call_sync already logged the error; emit metric.
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "error"])
                    .inc();
                None
            }
        }
    }
}

/// Fire the actual HTTP POST to the Anthropic messages endpoint.
///
/// Returns the deserialized response or a string error (fail-open callers
/// log the error and return `None`).
async fn call_anthropic_http(
    endpoint: &str,
    api_key: &str,
    body_bytes: Vec<u8>,
    timeout_ms: u64,
) -> Result<AnthropicResponse, String> {
    let uri: hyper::Uri = endpoint
        .parse()
        .map_err(|e| format!("invalid endpoint URI: {e}"))?;

    let client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build_http();

    let req = hyper::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .body(http_body_util::Full::new(bytes::Bytes::from(body_bytes)))
        .map_err(|e| format!("build request: {e}"))?;

    use http_body_util::BodyExt as _;

    let response = tokio::time::timeout(Duration::from_millis(timeout_ms), client.request(req))
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(|e| format!("HTTP: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP status {}", response.status()));
    }

    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("body collect: {e}"))?
        .to_bytes();

    serde_json::from_slice::<AnthropicResponse>(&body)
        .map_err(|e| format!("parse response JSON: {e}"))
}

// ── Ollama provider ───────────────────────────────────────────────────────────

/// Ollama local LLM provider configuration.
///
/// Used for sovereign / air-gapped deployments where no cloud API key is
/// available.  The Ollama API at `/api/chat` is OpenAI-compatible.
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// Chat endpoint.  Default: `http://localhost:11434/api/chat`.
    pub endpoint: String,
    /// Model name.  Default: `llama3.2:3b`.
    pub model: String,
    /// HTTP call timeout in milliseconds.  Default: 5 000.
    pub timeout_ms: u64,
    /// Maximum calls per second.  Default: 5.
    pub max_qps: u32,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434/api/chat".to_string(),
            model: "llama3.2:3b".to_string(),
            timeout_ms: 5000,
            max_qps: 5,
        }
    }
}

/// Ollama request body (`/api/chat`).
#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

/// Ollama response — we only need `message.content`.
#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

/// Local Ollama LLM provider (sovereign / air-gapped use-case).
pub struct OllamaAiProvider {
    config: OllamaConfig,
    metrics: LlmProviderMetrics,
    cache: Mutex<LruCache>,
    qps: Mutex<QpsLimiter>,
}

impl OllamaAiProvider {
    /// Create a provider with the given config and metrics.
    pub fn new(config: OllamaConfig, metrics: LlmProviderMetrics) -> Self {
        let max_qps = config.max_qps;
        Self {
            config,
            metrics,
            cache: Mutex::new(LruCache::new(CACHE_CAPACITY)),
            qps: Mutex::new(QpsLimiter::new(max_qps)),
        }
    }

    /// Create with defaults and noop metrics (for tests / dev).
    pub fn with_defaults() -> Self {
        Self::new(OllamaConfig::default(), LlmProviderMetrics::noop())
    }

    fn provider_name(&self) -> &'static str {
        "ollama"
    }

    fn build_prompt(&self, user_id: Option<&str>, path: &str, score: f32) -> String {
        let masked_uid = user_id
            .map(|id| {
                if id.len() > 4 {
                    format!("{}****", &id[..4])
                } else {
                    "****".to_string()
                }
            })
            .unwrap_or_else(|| "anonymous".to_string());

        format!(
            "Analyse this HTTP request for security threats. \
             Reply ONLY with JSON: {{\"score\": <0.0-1.0>, \"labels\": [], \"evidence\": \"\"}}. \
             Request: user={masked_uid} path={path} heuristic_score={score:.2}."
        )
    }

    fn call_sync(&self, user_id: Option<&str>, path: &str, score: f32) -> Option<LlmVerdict> {
        let prompt = self.build_prompt(user_id, path, score);
        let body = OllamaRequest {
            model: self.config.model.clone(),
            messages: vec![OllamaMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            stream: false,
        };
        let body_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "ollama: failed to serialise request body");
                return None;
            }
        };

        let endpoint = self.config.endpoint.clone();
        let timeout_ms = self.config.timeout_ms;

        let handle = crate::pingora::runtime::tokio_handle();
        let (tx, rx) = std::sync::mpsc::channel::<Result<OllamaResponse, String>>();

        handle.spawn(async move {
            let result = call_ollama_http(&endpoint, body_bytes, timeout_ms).await;
            let _ = tx.send(result);
        });

        let deadline = Duration::from_millis(timeout_ms + 200);
        match rx.recv_timeout(deadline) {
            Ok(Ok(resp)) => {
                let verdict_str = extract_verdict_json(&resp.message.content);
                match serde_json::from_str::<LlmVerdict>(&verdict_str) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!(error = %e, "ollama: failed to parse verdict JSON");
                        None
                    }
                }
            }
            Ok(Err(e)) => {
                warn!(error = %e, "ollama: HTTP call failed");
                None
            }
            Err(_) => {
                warn!("ollama: bridge timeout");
                None
            }
        }
    }
}

impl AiProvider for OllamaAiProvider {
    fn contextualise(&self, user_id: Option<&str>, path: &str, score: f32) -> Option<String> {
        let provider = self.provider_name();

        // ── Cache lookup ──────────────────────────────────────────────────────
        let key = cache_key(user_id.unwrap_or(""), path, "", (score * 10.0) as u8);
        {
            let cache = self.cache.lock().expect("ollama cache poisoned");
            if let Some(cached) = cache.get(key) {
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "cache_hit"])
                    .inc();
                return Some(cached.to_owned());
            }
        }

        // ── QPS gate ─────────────────────────────────────────────────────────
        {
            let mut qps = self.qps.lock().expect("ollama qps limiter poisoned");
            if !qps.allow() {
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "rate_limited"])
                    .inc();
                return None;
            }
        }

        // ── HTTP call ─────────────────────────────────────────────────────────
        let t0 = Instant::now();
        let result = self.call_sync(user_id, path, score);
        let elapsed = t0.elapsed().as_secs_f64();
        self.metrics
            .latency_seconds
            .with_label_values(&[provider])
            .observe(elapsed);

        match result {
            Some(v) => {
                let summary = format!(
                    "score={:.2} labels={:?} evidence={}",
                    v.score, v.labels, v.evidence
                );
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "success"])
                    .inc();
                let mut cache = self.cache.lock().expect("ollama cache poisoned");
                cache.insert(key, summary.clone());
                Some(summary)
            }
            None => {
                self.metrics
                    .calls_total
                    .with_label_values(&[provider, "error"])
                    .inc();
                None
            }
        }
    }
}

/// Fire the actual HTTP POST to the Ollama `/api/chat` endpoint.
async fn call_ollama_http(
    endpoint: &str,
    body_bytes: Vec<u8>,
    timeout_ms: u64,
) -> Result<OllamaResponse, String> {
    let uri: hyper::Uri = endpoint
        .parse()
        .map_err(|e| format!("invalid Ollama endpoint URI: {e}"))?;

    let client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build_http();

    let req = hyper::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(bytes::Bytes::from(body_bytes)))
        .map_err(|e| format!("build request: {e}"))?;

    use http_body_util::BodyExt as _;

    let response = tokio::time::timeout(Duration::from_millis(timeout_ms), client.request(req))
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(|e| format!("HTTP: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP status {}", response.status()));
    }

    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("body collect: {e}"))?
        .to_bytes();

    serde_json::from_slice::<OllamaResponse>(&body)
        .map_err(|e| format!("parse Ollama JSON: {e}"))
}

// ── JSON extraction helper ─────────────────────────────────────────────────────

/// Extract the first `{...}` JSON object from a potentially padded LLM response.
///
/// Models sometimes wrap JSON in backtick code fences or add prose before/after.
/// This function finds the first `{` and last `}` and returns the slice.
fn extract_verdict_json(text: &str) -> String {
    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());
    text[start..end].to_string()
}

// ── tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    // ── extract_verdict_json ──────────────────────────────────────────────────

    #[test]
    fn extract_verdict_json_bare_object() {
        let s = r#"{"score":0.9,"labels":["injection"],"evidence":"test"}"#;
        assert_eq!(extract_verdict_json(s), s);
    }

    #[test]
    fn extract_verdict_json_strips_prose() {
        let s = r#"Here is the analysis: {"score":0.1,"labels":[],"evidence":"clean"} done."#;
        let extracted = extract_verdict_json(s);
        assert!(extracted.starts_with('{'));
        assert!(extracted.ends_with('}'));
    }

    // ── cache ─────────────────────────────────────────────────────────────────

    #[test]
    fn lru_cache_get_returns_none_on_miss() {
        let cache = LruCache::new(10);
        assert!(cache.get(42).is_none());
    }

    #[test]
    fn lru_cache_insert_and_hit() {
        let mut cache = LruCache::new(10);
        cache.insert(1, "hello".to_string());
        assert_eq!(cache.get(1), Some("hello"));
    }

    #[test]
    fn lru_cache_evicts_when_full() {
        let mut cache = LruCache::new(3);
        cache.insert(1, "a".to_string());
        cache.insert(2, "b".to_string());
        cache.insert(3, "c".to_string());
        cache.insert(4, "d".to_string()); // triggers eviction
        assert_eq!(cache.map.len(), 3);
    }

    // ── QPS limiter ───────────────────────────────────────────────────────────

    #[test]
    fn qps_limiter_allows_up_to_max() {
        let mut lim = QpsLimiter::new(3);
        assert!(lim.allow());
        assert!(lim.allow());
        assert!(lim.allow());
        // 4th call in same second → denied
        assert!(!lim.allow());
    }

    #[test]
    fn qps_limiter_allows_unlimited_with_max_u32() {
        let mut lim = QpsLimiter::new(u32::MAX);
        for _ in 0..1000 {
            assert!(lim.allow());
        }
    }

    // ── AnthropicAiProvider — no API key → fail-open ───────────────────────────

    #[test]
    fn anthropic_no_api_key_returns_none() {
        // Ensure the env var is unset.
        std::env::remove_var("ANTHROPIC_API_KEY");
        let mut cfg = AnthropicConfig::default();
        cfg.api_key_env = "ANTHROPIC_API_KEY".to_string();
        let provider = AnthropicAiProvider::new(cfg, LlmProviderMetrics::noop());
        // Should return None (fail-open) because API key is missing.
        let result = provider.contextualise(Some("user42"), "/api/v1/foo", 0.7);
        assert!(result.is_none(), "expected None when API key is absent");
    }

    // ── AnthropicAiProvider — rate limit → fail-open ──────────────────────────

    #[test]
    fn anthropic_rate_limit_fail_open() {
        std::env::set_var("TEST_ANTHROPIC_KEY", "fake-key");
        let mut cfg = AnthropicConfig::default();
        cfg.api_key_env = "TEST_ANTHROPIC_KEY".to_string();
        cfg.max_qps = 0; // allow nothing
        let provider = AnthropicAiProvider::new(cfg, LlmProviderMetrics::noop());
        let result = provider.contextualise(Some("user1"), "/path", 0.8);
        assert!(result.is_none(), "expected None when rate limited");
        std::env::remove_var("TEST_ANTHROPIC_KEY");
    }

    // ── AnthropicAiProvider — cache hit avoids HTTP ────────────────────────────

    #[test]
    fn anthropic_cache_hit_returns_cached_value() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        let cfg = AnthropicConfig::default();
        let provider = AnthropicAiProvider::new(cfg, LlmProviderMetrics::noop());

        // Seed the cache manually.
        let key = cache_key("", "/cached-path", "", 7);
        {
            let mut cache = provider.cache.lock().unwrap();
            cache.insert(key, "score=0.7 labels=[\"test\"] evidence=cached".to_string());
        }

        // Call contextualise — should get the cached value, not make an HTTP call.
        let result = provider.contextualise(None, "/cached-path", 0.7);
        assert!(result.is_some(), "expected cache hit");
        assert!(result.unwrap().contains("cached"), "expected cached content");
    }

    // ── AnthropicAiProvider — mock HTTP server (Anthropic-shaped response) ─────

    #[tokio::test]
    async fn anthropic_mock_server_returns_parsed_verdict() {
        // Spin up a minimal HTTP mock server.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            use std::io::{Read, Write};
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf);

            let response_body = serde_json::json!({
                "id": "msg_01",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "text",
                    "text": "{\"score\":0.85,\"labels\":[\"prompt_injection\"],\"evidence\":\"suspicious pattern\"}"
                }],
                "usage": {"input_tokens": 100, "output_tokens": 30}
            });
            let body_str = response_body.to_string();
            let http_resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            let _ = stream.write_all(http_resp.as_bytes());
        });

        // Call the HTTP function directly.
        let body = serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 256,
            "messages": [{"role": "user", "content": "test"}]
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let endpoint = format!("http://{}", addr);

        let result = call_anthropic_http(&endpoint, "fake-key", body_bytes, 2000).await;
        assert!(result.is_ok(), "expected Ok from mock server: {:?}", result.err());
        let resp = result.unwrap();
        let text = resp.content.into_iter().find(|c| c.content_type == "text").map(|c| c.text).unwrap_or_default();
        let verdict: LlmVerdict = serde_json::from_str(&text).expect("parse verdict");
        assert!((verdict.score - 0.85).abs() < 0.01);
        assert_eq!(verdict.labels, vec!["prompt_injection"]);
    }

    // ── AnthropicAiProvider — timeout → fail-open ─────────────────────────────

    #[tokio::test]
    async fn anthropic_timeout_returns_none() {
        // Mock server that never responds.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(60)); // never responds
        });

        let endpoint = format!("http://{}", addr);
        let result = call_anthropic_http(&endpoint, "fake-key", b"{}".to_vec(), 50).await;
        assert!(result.is_err(), "expected timeout error");
        let err = result.unwrap_err();
        assert!(err.contains("timeout"), "expected timeout message, got: {err}");
    }

    // ── AnthropicAiProvider — malformed JSON → fail-open ──────────────────────

    #[tokio::test]
    async fn anthropic_malformed_json_returns_none() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = b"NOT_VALID_JSON!!!";
            let http_resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(http_resp.as_bytes());
            let _ = stream.write_all(body);
        });

        let endpoint = format!("http://{}", addr);
        let result = call_anthropic_http(&endpoint, "fake", b"{}".to_vec(), 2000).await;
        assert!(result.is_err(), "malformed JSON must return Err");
    }

    // ── OllamaAiProvider — basic construction ────────────────────────────────

    #[test]
    fn ollama_provider_constructs_with_defaults() {
        let p = OllamaAiProvider::with_defaults();
        assert_eq!(p.provider_name(), "ollama");
    }

    // ── OllamaAiProvider — rate limit → fail-open ─────────────────────────────

    #[test]
    fn ollama_rate_limit_fail_open() {
        let mut cfg = OllamaConfig::default();
        cfg.max_qps = 0;
        let provider = OllamaAiProvider::new(cfg, LlmProviderMetrics::noop());
        let result = provider.contextualise(None, "/api/foo", 0.5);
        assert!(result.is_none(), "expected None when rate limited");
    }

    // ── OllamaAiProvider — cache hit ──────────────────────────────────────────

    #[test]
    fn ollama_cache_hit_returns_cached_value() {
        let cfg = OllamaConfig::default();
        let provider = OllamaAiProvider::new(cfg, LlmProviderMetrics::noop());

        let key = cache_key("", "/ollama-cached", "", 5);
        {
            let mut cache = provider.cache.lock().unwrap();
            cache.insert(key, "score=0.5 labels=[] evidence=ollama-cached".to_string());
        }

        let result = provider.contextualise(None, "/ollama-cached", 0.5);
        assert!(result.is_some());
        assert!(result.unwrap().contains("ollama-cached"));
    }

    // ── OllamaAiProvider — mock HTTP server ───────────────────────────────────

    #[tokio::test]
    async fn ollama_mock_server_returns_parsed_verdict() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf);

            let body = serde_json::json!({
                "model": "llama3.2:3b",
                "message": {
                    "role": "assistant",
                    "content": "{\"score\":0.2,\"labels\":[],\"evidence\":\"clean\"}"
                }
            });
            let body_str = body.to_string();
            let http_resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            let _ = stream.write_all(http_resp.as_bytes());
        });

        let endpoint = format!("http://{}", addr);
        let result = call_ollama_http(&endpoint, b"{}".to_vec(), 2000).await;
        assert!(result.is_ok(), "expected Ok: {:?}", result.err());
        let resp = result.unwrap();
        let verdict: LlmVerdict = serde_json::from_str(&resp.message.content).expect("parse");
        assert!((verdict.score - 0.2).abs() < 0.01);
    }

    // ── metrics registration ──────────────────────────────────────────────────

    #[test]
    fn llm_provider_metrics_register_successfully() {
        let r = prometheus::Registry::new();
        LlmProviderMetrics::new(&r).expect("metrics registration ok");
    }

    #[test]
    fn llm_provider_metrics_counters_increment() {
        let r = prometheus::Registry::new();
        let m = LlmProviderMetrics::new(&r).unwrap();
        m.calls_total
            .with_label_values(&["anthropic", "success"])
            .inc();
        m.calls_total
            .with_label_values(&["anthropic", "cache_hit"])
            .inc_by(5);
        let families = r.gather();
        let fam = families
            .iter()
            .find(|f| f.get_name() == "armageddon_ai_provider_calls_total")
            .expect("counter must exist");
        let total: f64 = fam.get_metric().iter().map(|m| m.get_counter().get_value()).sum();
        assert_eq!(total, 6.0);
    }

    // ── cache_key determinism ─────────────────────────────────────────────────

    #[test]
    fn cache_key_is_deterministic() {
        let k1 = cache_key("tenant1", "/api/foo", "GET", 7);
        let k2 = cache_key("tenant1", "/api/foo", "GET", 7);
        assert_eq!(k1, k2);
    }

    #[test]
    fn cache_key_differs_on_different_inputs() {
        let k1 = cache_key("t1", "/a", "GET", 7);
        let k2 = cache_key("t2", "/a", "GET", 7);
        assert_ne!(k1, k2);
    }
}
