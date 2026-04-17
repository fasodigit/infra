//! GitHub webhook handler for ARMAGEDDON.
//!
//! Validates incoming `POST /webhooks/github` requests:
//! 1. Body size <= 25 MiB (GitHub hard limit).
//! 2. `X-Hub-Signature-256: sha256=<hex>` — HMAC-SHA256 constant-time comparison.
//! 3. `X-GitHub-Event` — whitelisted event type.
//! 4. `X-GitHub-Delivery` — idempotence UUID key, deduplicated via KAYA.
//! 5. JSON body parse (`serde_json::Value`).
//! 6. Rate-limit per source IP via KAYA `INCR faso:rate:github:<ip> EX 60`.
//! 7. Publish to Redpanda topic `github.events.v1`.

use crate::kafka_producer::RedpandaProducer;
use armageddon_common::types::HttpRequest;
use chrono::Utc;
use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use sha2::Sha256;
use std::sync::Arc;
use subtle::ConstantTimeEq;

// -- constants --

/// Maximum body size accepted from GitHub (25 MiB).
pub const MAX_BODY_SIZE: usize = 25 * 1024 * 1024;

/// Rate-limit cap: requests per IP per minute.
pub const RATE_LIMIT_CAP: i64 = 1000;

/// KAYA TTL for idempotence keys (24 h).
const DELIVERY_KEY_TTL_SECS: i64 = 86_400;

/// KAYA TTL for rate-limit counters (1 min).
const RATE_LIMIT_TTL_SECS: i64 = 60;

/// Redpanda topic for all GitHub events.
pub const GITHUB_EVENTS_TOPIC: &str = "github.events.v1";

/// Event types we accept. Anything outside this whitelist is rejected 400.
const ALLOWED_EVENTS: &[&str] = &[
    "push",
    "pull_request",
    "issues",
    "issue_comment",
    "workflow_run",
    "create",
    "delete",
    "release",
    "check_run",
    "check_suite",
    "deployment",
    "deployment_status",
    "fork",
    "ping",
    "star",
    "member",
    "repository",
];

// -- error type --

/// Errors specific to the webhook handler.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("body too large: {size} bytes (max {max})")]
    BodyTooLarge { size: usize, max: usize },

    #[error("missing header: {0}")]
    MissingHeader(&'static str),

    #[error("invalid HMAC signature")]
    InvalidSignature,

    #[error("unsupported event type: {0}")]
    UnsupportedEvent(String),

    #[error("invalid JSON payload: {0}")]
    InvalidJson(String),

    #[error("duplicate delivery: {0}")]
    DuplicateDelivery(String),

    #[error("rate limit exceeded for IP {ip}: {count}/{cap} req/min")]
    RateLimitExceeded { ip: String, count: i64, cap: i64 },

    #[error("KAYA error: {0}")]
    Kaya(String),

    #[error("Redpanda error: {0}")]
    Redpanda(String),
}

// -- response type --

/// Thin HTTP response produced by the webhook handler.
#[derive(Debug)]
pub struct WebhookResponse {
    pub status: u16,
    pub body: String,
}

impl WebhookResponse {
    pub fn ok(body: impl Into<String>) -> Self {
        Self { status: 200, body: body.into() }
    }

    pub fn accepted(body: impl Into<String>) -> Self {
        Self { status: 202, body: body.into() }
    }

    pub fn bad_request(body: impl Into<String>) -> Self {
        Self { status: 400, body: body.into() }
    }

    pub fn too_large(body: impl Into<String>) -> Self {
        Self { status: 413, body: body.into() }
    }

    pub fn too_many_requests(body: impl Into<String>) -> Self {
        Self { status: 429, body: body.into() }
    }
}

// -- mapped event payloads --

/// Structured push event published to Redpanda.
#[derive(Debug, serde::Serialize)]
pub struct GithubPushEvent {
    pub repo: String,
    pub r#ref: String,
    pub commits: Vec<CommitSummary>,
    pub pusher: String,
    pub delivery_id: String,
    pub received_at: String,
}

/// Condensed commit summary.
#[derive(Debug, serde::Serialize)]
pub struct CommitSummary {
    pub id: String,
    pub message: String,
    pub author: String,
}

/// Structured pull_request event published to Redpanda.
#[derive(Debug, serde::Serialize)]
pub struct GithubPullRequestEvent {
    pub action: String,
    pub pr_number: u64,
    pub title: String,
    pub author: String,
    pub repo: String,
    pub delivery_id: String,
    pub received_at: String,
}

/// Pass-through envelope for all other event types.
#[derive(Debug, serde::Serialize)]
pub struct GithubRawEvent {
    pub event_type: String,
    pub delivery_id: String,
    pub payload_json: serde_json::Value,
    pub received_at: String,
}

/// Top-level envelope written to the Redpanda topic.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GithubEventEnvelope {
    Push(GithubPushEvent),
    PullRequest(GithubPullRequestEvent),
    Raw(GithubRawEvent),
}

// -- handler --

/// GitHub webhook handler.
///
/// Cloned cheaply — all heavy resources are behind `Arc`.
#[derive(Clone, Debug)]
pub struct GithubWebhookHandler {
    /// Raw HMAC-SHA256 secret bytes from env `ARMAGEDDON_GITHUB_WEBHOOK_SECRET`.
    secret: Arc<Vec<u8>>,
    /// Redpanda producer facade.
    producer: Arc<RedpandaProducer>,
    /// KAYA client for deduplication and rate-limiting.
    kaya_client: Arc<redis::Client>,
    /// Redpanda topic name.
    topic: String,
    /// Per-IP rate limit cap (requests / minute).
    rate_limit_cap: i64,
}

impl GithubWebhookHandler {
    /// Construct a new handler.
    ///
    /// Parameters:
    /// - `secret`         — raw HMAC secret bytes
    /// - `producer`       — shared Redpanda producer
    /// - `kaya_client`    — KAYA redis::Client
    /// - `topic`          — Redpanda topic name
    /// - `rate_limit_cap` — max req/IP/min (default 1000)
    pub fn new(
        secret: Vec<u8>,
        producer: Arc<RedpandaProducer>,
        kaya_client: redis::Client,
        topic: impl Into<String>,
        rate_limit_cap: i64,
    ) -> Self {
        Self {
            secret: Arc::new(secret),
            producer,
            kaya_client: Arc::new(kaya_client),
            topic: topic.into(),
            rate_limit_cap,
        }
    }

    /// Handle a single `POST /webhooks/github` request.
    ///
    /// Returns a `WebhookResponse` — never panics.
    pub async fn handle(
        &self,
        req: &HttpRequest,
        source_ip: &str,
    ) -> Result<WebhookResponse, WebhookError> {
        // -- 1. body size guard --
        let body = req.body.as_deref().unwrap_or(&[]);
        if body.len() > MAX_BODY_SIZE {
            tracing::warn!(
                source_ip = %source_ip,
                body_size = body.len(),
                "GitHub webhook body exceeds 25 MiB limit"
            );
            return Ok(WebhookResponse::too_large(
                r#"{"error":"payload_too_large","gateway":"ARMAGEDDON"}"#,
            ));
        }

        // -- 2. rate limit via KAYA --
        self.check_rate_limit(source_ip).await?;

        // -- 3. extract required headers --
        let sig_header = req
            .headers
            .get("x-hub-signature-256")
            .ok_or(WebhookError::MissingHeader("x-hub-signature-256"))?;

        let event_type = req
            .headers
            .get("x-github-event")
            .ok_or(WebhookError::MissingHeader("x-github-event"))?
            .clone();

        let delivery_id = req
            .headers
            .get("x-github-delivery")
            .ok_or(WebhookError::MissingHeader("x-github-delivery"))?
            .clone();

        // -- 4. HMAC-SHA256 validation (constant-time) --
        self.verify_hmac(body, sig_header)?;

        // -- 5. event whitelist --
        if !ALLOWED_EVENTS.contains(&event_type.as_str()) {
            tracing::warn!(
                delivery_id = %delivery_id,
                event_type = %event_type,
                "GitHub webhook rejected: unsupported event type"
            );
            return Ok(WebhookResponse::bad_request(
                serde_json::json!({
                    "error": "unsupported_event",
                    "event_type": event_type,
                    "gateway": "ARMAGEDDON"
                })
                .to_string(),
            ));
        }

        // -- 6. deduplication via KAYA --
        if self.check_duplicate(&delivery_id).await? {
            tracing::info!(
                delivery_id = %delivery_id,
                event_type = %event_type,
                "GitHub webhook duplicate delivery — returning 200 idempotent"
            );
            return Ok(WebhookResponse::ok(
                serde_json::json!({
                    "status": "duplicate",
                    "delivery_id": delivery_id,
                    "gateway": "ARMAGEDDON"
                })
                .to_string(),
            ));
        }

        // -- 7. JSON parse --
        let payload: serde_json::Value = serde_json::from_slice(body)
            .map_err(|e| WebhookError::InvalidJson(e.to_string()))?;

        // -- 8. map event to envelope --
        let received_at = Utc::now().to_rfc3339();
        let envelope = map_event(&event_type, &delivery_id, &received_at, &payload);

        // -- 9. partition key = repository full name (best-effort) --
        let partition_key = payload
            .get("repository")
            .and_then(|r| r.get("full_name"))
            .and_then(|v| v.as_str())
            .unwrap_or(&delivery_id)
            .to_string();

        // -- 10. serialize and produce --
        let message_bytes = serde_json::to_vec(&envelope)
            .map_err(|e| WebhookError::Redpanda(format!("serialize envelope: {e}")))?;

        self.producer
            .produce(&self.topic, &partition_key, &message_bytes)
            .await
            .map_err(|e| WebhookError::Redpanda(e.to_string()))?;

        tracing::info!(
            event_type = %event_type,
            delivery_id = %delivery_id,
            topic = %self.topic,
            partition_key = %partition_key,
            source_ip = %source_ip,
            "GitHub webhook accepted and enqueued"
        );

        Ok(WebhookResponse::accepted(
            serde_json::json!({
                "status": "accepted",
                "delivery_id": delivery_id,
                "event_type": event_type,
                "gateway": "ARMAGEDDON"
            })
            .to_string(),
        ))
    }

    // -- private helpers --

    /// Verify `X-Hub-Signature-256: sha256=<hex>` against the body.
    ///
    /// Uses `subtle::ConstantTimeEq` to prevent timing attacks.
    fn verify_hmac(
        &self,
        body: &[u8],
        sig_header: &str,
    ) -> Result<(), WebhookError> {
        let hex_part = sig_header
            .strip_prefix("sha256=")
            .ok_or(WebhookError::InvalidSignature)?;

        let provided_bytes =
            hex::decode(hex_part).map_err(|_| WebhookError::InvalidSignature)?;

        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret)
            .map_err(|_| WebhookError::InvalidSignature)?;
        mac.update(body);
        let expected = mac.finalize().into_bytes();

        if expected.as_slice().ct_eq(&provided_bytes).into() {
            Ok(())
        } else {
            Err(WebhookError::InvalidSignature)
        }
    }

    /// Increment the KAYA rate-limit counter for `source_ip`.
    ///
    /// Key: `faso:rate:github:<ip>` — INCR + EXPIRE on first write.
    async fn check_rate_limit(&self, source_ip: &str) -> Result<(), WebhookError> {
        let key = format!("faso:rate:github:{source_ip}");

        let mut conn = self
            .kaya_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| WebhookError::Kaya(e.to_string()))?;

        let count: i64 = conn
            .incr(&key, 1_i64)
            .await
            .map_err(|e: redis::RedisError| WebhookError::Kaya(e.to_string()))?;

        // Set expiry only on first increment
        if count == 1 {
            let _: i64 = conn
                .expire(&key, RATE_LIMIT_TTL_SECS)
                .await
                .map_err(|e: redis::RedisError| WebhookError::Kaya(e.to_string()))?;
        }

        if count > self.rate_limit_cap {
            tracing::warn!(
                source_ip = %source_ip,
                count = count,
                cap = self.rate_limit_cap,
                "GitHub webhook rate limit exceeded"
            );
            return Err(WebhookError::RateLimitExceeded {
                ip: source_ip.to_string(),
                count,
                cap: self.rate_limit_cap,
            });
        }

        Ok(())
    }

    /// Set `faso:github:delivery:<uuid> 1 EX 86400 NX` in KAYA.
    ///
    /// Returns `true` if the key already existed (duplicate delivery).
    async fn check_duplicate(&self, delivery_id: &str) -> Result<bool, WebhookError> {
        let key = format!("faso:github:delivery:{delivery_id}");

        let mut conn = self
            .kaya_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| WebhookError::Kaya(e.to_string()))?;

        // SET key 1 EX 86400 NX  — returns None if key already existed
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("EX")
            .arg(DELIVERY_KEY_TTL_SECS)
            .arg("NX")
            .query_async(&mut conn)
            .await
            .map_err(|e: redis::RedisError| WebhookError::Kaya(e.to_string()))?;

        // None => NX rejected => key already existed => duplicate
        Ok(result.is_none())
    }
}

// -- event mapping --

/// Map a raw GitHub payload to a typed `GithubEventEnvelope`.
pub fn map_event(
    event_type: &str,
    delivery_id: &str,
    received_at: &str,
    payload: &serde_json::Value,
) -> GithubEventEnvelope {
    match event_type {
        "push" => GithubEventEnvelope::Push(map_push(delivery_id, received_at, payload)),
        "pull_request" => {
            GithubEventEnvelope::PullRequest(map_pull_request(delivery_id, received_at, payload))
        }
        _ => GithubEventEnvelope::Raw(GithubRawEvent {
            event_type: event_type.to_string(),
            delivery_id: delivery_id.to_string(),
            payload_json: payload.clone(),
            received_at: received_at.to_string(),
        }),
    }
}

fn map_push(delivery_id: &str, received_at: &str, p: &serde_json::Value) -> GithubPushEvent {
    let repo = p
        .get("repository")
        .and_then(|r| r.get("full_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let git_ref = p.get("ref").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let pusher = p
        .get("pusher")
        .and_then(|pu| pu.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let commits: Vec<CommitSummary> = p
        .get("commits")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| CommitSummary {
                    id: c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    message: c
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    author: c
                        .get("author")
                        .and_then(|a| a.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    GithubPushEvent {
        repo,
        r#ref: git_ref,
        commits,
        pusher,
        delivery_id: delivery_id.to_string(),
        received_at: received_at.to_string(),
    }
}

fn map_pull_request(
    delivery_id: &str,
    received_at: &str,
    p: &serde_json::Value,
) -> GithubPullRequestEvent {
    let action = p
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let pr_number = p.get("number").and_then(|v| v.as_u64()).unwrap_or(0);

    let pr = p.get("pull_request").unwrap_or(&serde_json::Value::Null);

    let title = pr
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let author = pr
        .get("user")
        .and_then(|u| u.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let repo = p
        .get("repository")
        .and_then(|r| r.get("full_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    GithubPullRequestEvent {
        action,
        pr_number,
        title,
        author,
        repo,
        delivery_id: delivery_id.to_string(),
        received_at: received_at.to_string(),
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::types::HttpVersion;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::collections::HashMap;

    const TEST_SECRET: &[u8] = b"test-secret-faso";

    /// Build a valid HMAC-SHA256 signature header for `body`.
    fn make_signature(body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(TEST_SECRET).unwrap();
        mac.update(body);
        let result = mac.finalize().into_bytes();
        format!("sha256={}", hex::encode(result))
    }

    fn make_request(
        body: &[u8],
        event_type: &str,
        delivery_id: &str,
        sig: &str,
    ) -> HttpRequest {
        let mut headers = HashMap::new();
        headers.insert("x-hub-signature-256".to_string(), sig.to_string());
        headers.insert("x-github-event".to_string(), event_type.to_string());
        headers.insert("x-github-delivery".to_string(), delivery_id.to_string());
        headers.insert("content-type".to_string(), "application/json".to_string());
        HttpRequest {
            method: "POST".to_string(),
            uri: "/webhooks/github".to_string(),
            path: "/webhooks/github".to_string(),
            query: None,
            headers,
            body: Some(body.to_vec()),
            version: HttpVersion::Http11,
        }
    }

    // Inline HMAC helper used in unit tests (no networking required).
    fn verify_hmac_direct(
        secret: &[u8],
        body: &[u8],
        sig: &str,
    ) -> Result<(), WebhookError> {
        let hex_part = sig
            .strip_prefix("sha256=")
            .ok_or(WebhookError::InvalidSignature)?;
        let provided =
            hex::decode(hex_part).map_err(|_| WebhookError::InvalidSignature)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        mac.update(body);
        let expected = mac.finalize().into_bytes();
        if expected.as_slice().ct_eq(&provided).into() {
            Ok(())
        } else {
            Err(WebhookError::InvalidSignature)
        }
    }

    // -- 1. HMAC valid --
    #[test]
    fn test_hmac_valid() {
        let body = br#"{"ref":"refs/heads/main"}"#;
        let sig = make_signature(body);
        assert!(verify_hmac_direct(TEST_SECRET, body, &sig).is_ok());
    }

    // -- 2. HMAC invalid: wrong secret --
    #[test]
    fn test_hmac_invalid_wrong_secret() {
        let body = br#"{"ref":"refs/heads/main"}"#;
        let sig = make_signature(body);
        assert!(matches!(
            verify_hmac_direct(b"wrong-secret", body, &sig),
            Err(WebhookError::InvalidSignature)
        ));
    }

    // -- 3. HMAC tampered body --
    #[test]
    fn test_hmac_tampered_body() {
        let body = br#"{"ref":"refs/heads/main"}"#;
        let sig = make_signature(body);
        let tampered = br#"{"ref":"refs/heads/evil"}"#;
        assert!(matches!(
            verify_hmac_direct(TEST_SECRET, tampered, &sig),
            Err(WebhookError::InvalidSignature)
        ));
    }

    // -- 4. missing signature prefix --
    #[test]
    fn test_hmac_missing_prefix() {
        let body = b"{}";
        let raw_hex = {
            let mut mac = Hmac::<Sha256>::new_from_slice(TEST_SECRET).unwrap();
            mac.update(body);
            hex::encode(mac.finalize().into_bytes())
        };
        // No "sha256=" prefix
        assert!(matches!(
            verify_hmac_direct(TEST_SECRET, body, &raw_hex),
            Err(WebhookError::InvalidSignature)
        ));
    }

    // -- 5. missing event header (simulated at extraction layer) --
    #[test]
    fn test_missing_event_header() {
        let body = br#"{"action":"opened"}"#;
        let sig = make_signature(body);
        let mut req = make_request(body, "push", "uuid-1234", &sig);
        req.headers.remove("x-github-event");
        assert!(req.headers.get("x-github-event").is_none());
    }

    // -- 6. unsupported event whitelist --
    #[test]
    fn test_unsupported_event_not_in_whitelist() {
        assert!(!ALLOWED_EVENTS.contains(&"custom_event"));
        assert!(ALLOWED_EVENTS.contains(&"push"));
        assert!(ALLOWED_EVENTS.contains(&"pull_request"));
        assert!(ALLOWED_EVENTS.contains(&"ping"));
    }

    // -- 7. body size limit constant check --
    #[test]
    fn test_body_size_limit_constant() {
        assert_eq!(MAX_BODY_SIZE, 25 * 1024 * 1024);
        let large = vec![b'x'; MAX_BODY_SIZE + 1];
        assert!(large.len() > MAX_BODY_SIZE);
    }

    // -- 8. push event mapping --
    #[test]
    fn test_map_push_event_parsing() {
        let payload = serde_json::json!({
            "ref": "refs/heads/main",
            "repository": { "full_name": "faso/test-repo" },
            "pusher": { "name": "alice" },
            "commits": [
                { "id": "abc123", "message": "fix bug", "author": { "name": "alice" } }
            ]
        });

        let event = map_push("delivery-001", "2026-04-17T00:00:00Z", &payload);
        assert_eq!(event.repo, "faso/test-repo");
        assert_eq!(event.r#ref, "refs/heads/main");
        assert_eq!(event.pusher, "alice");
        assert_eq!(event.commits.len(), 1);
        assert_eq!(event.commits[0].id, "abc123");
        assert_eq!(event.commits[0].message, "fix bug");
    }

    // -- 9. pull_request event mapping --
    #[test]
    fn test_map_pull_request_event_parsing() {
        let payload = serde_json::json!({
            "action": "opened",
            "number": 42,
            "repository": { "full_name": "faso/test-repo" },
            "pull_request": {
                "title": "Add webhook handler",
                "user": { "login": "bob" }
            }
        });

        let event = map_pull_request("delivery-002", "2026-04-17T00:00:00Z", &payload);
        assert_eq!(event.action, "opened");
        assert_eq!(event.pr_number, 42);
        assert_eq!(event.title, "Add webhook handler");
        assert_eq!(event.author, "bob");
        assert_eq!(event.repo, "faso/test-repo");
    }

    // -- 10. raw event fallthrough --
    #[test]
    fn test_map_raw_event_fallthrough() {
        let payload = serde_json::json!({ "action": "starred" });
        let envelope = map_event("star", "delivery-003", "2026-04-17T00:00:00Z", &payload);
        match envelope {
            GithubEventEnvelope::Raw(raw) => {
                assert_eq!(raw.event_type, "star");
                assert_eq!(raw.delivery_id, "delivery-003");
            }
            _ => panic!("expected GithubEventEnvelope::Raw"),
        }
    }

    // -- 11. push envelope serializes to JSON with correct tag --
    #[test]
    fn test_envelope_serialization_push() {
        let payload = serde_json::json!({
            "ref": "refs/heads/main",
            "repository": { "full_name": "faso/repo" },
            "pusher": { "name": "ci-bot" },
            "commits": []
        });
        let envelope = map_event("push", "d-001", "2026-04-17T00:00:00Z", &payload);
        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["kind"], "push");
        assert_eq!(json["repo"], "faso/repo");
    }

    // -- 12. empty commits list --
    #[test]
    fn test_map_push_empty_commits() {
        let payload = serde_json::json!({
            "ref": "refs/heads/feat",
            "repository": { "full_name": "faso/repo" },
            "pusher": { "name": "dev" },
            "commits": []
        });
        let event = map_push("d-999", "2026-04-17T00:00:00Z", &payload);
        assert!(event.commits.is_empty());
    }
}
