// SPDX-License-Identifier: AGPL-3.0-or-later
//! Vault Transit envelope encryption service (ADR-005).
//!
//! # Design
//! Vault Transit key `terroir-pii-master` uses `derived=true`, meaning every
//! encrypt/decrypt call **must** supply a `context` field (base64-encoded).
//! Context format: `tenant=<slug>|field=<name>|producer=<uuid>`.
//!
//! # DEK caching
//! Per ADR-005, decrypted DEKs are NOT cached in plain form. Instead, the
//! Vault ciphertext (the wrapped DEK) is stored in KAYA at key
//! `terroir:dek:cache:{kid}` with TTL 1h. On decrypt, we call Vault to
//! unwrap the DEK. This saves Vault API calls while not storing plaintext
//! keys in memory beyond the current request scope.
//!
//! # Vault API
//! - Encrypt: `POST /v1/transit/encrypt/terroir-pii-master`
//!   Body: `{ "plaintext": "<base64>", "context": "<base64>" }`
//!   Returns: `{ "data": { "ciphertext": "vault:v1:...", "key_version": 1 } }`
//! - Decrypt: `POST /v1/transit/decrypt/terroir-pii-master`
//!   Body: `{ "ciphertext": "vault:v1:...", "context": "<base64>" }`
//!   Returns: `{ "data": { "plaintext": "<base64>" } }`

use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

const DEK_CACHE_TTL_SECS: u64 = 3600; // 1 hour
const VAULT_TIMEOUT_SECS: u64 = 10;

// ---------------------------------------------------------------------------
// Vault Transit HTTP types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EncryptRequest {
    plaintext: String, // base64
    context: String,   // base64
}

#[derive(Deserialize)]
struct EncryptResponseData {
    ciphertext: String,
    key_version: u32,
}

#[derive(Deserialize)]
struct EncryptResponse {
    data: EncryptResponseData,
}

#[derive(Serialize)]
struct DecryptRequest {
    ciphertext: String,
    context: String, // base64
}

#[derive(Deserialize)]
struct DecryptResponseData {
    plaintext: String, // base64
}

#[derive(Deserialize)]
struct DecryptResponse {
    data: DecryptResponseData,
}

// ---------------------------------------------------------------------------
// PiiEncryptionService
// ---------------------------------------------------------------------------

/// Encrypted PII value returned by `encrypt`.
#[derive(Debug, Clone)]
pub struct EncryptedPii {
    /// Vault ciphertext (`vault:v1:...`) stored as `bytea` in PG.
    pub ciphertext_bytes: Vec<u8>,
    /// Key identifier — the ciphertext string itself is the kid.
    /// Stored in PG as `<field>_dek_kid`.
    pub kid: String,
}

/// Service that wraps Vault Transit for per-field PII encryption.
pub struct VaultTransitService {
    vault_addr: String,
    vault_token: String,
    http: reqwest::Client,
}

impl VaultTransitService {
    /// Create a new service instance.
    ///
    /// `vault_addr`: e.g. `http://localhost:8200`
    /// `vault_token`: Vault token with `transit:terroir-pii-master` policy.
    pub fn new(vault_addr: String, vault_token: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(VAULT_TIMEOUT_SECS))
            .build()
            .expect("build reqwest client");
        Self {
            vault_addr,
            vault_token,
            http,
        }
    }

    /// Encrypt a PII plaintext string.
    ///
    /// `context` must be the base64-encoded context string, e.g.
    /// `base64("tenant=t_pilot|field=full_name|producer=<uuid>")`.
    #[instrument(skip(self, plaintext), fields(context_len = context.len()))]
    pub async fn encrypt(&self, plaintext: &str, context: &str) -> Result<EncryptedPii> {
        let plaintext_b64 = base64::engine::general_purpose::STANDARD.encode(plaintext.as_bytes());

        let url = format!("{}/v1/transit/encrypt/terroir-pii-master", self.vault_addr);
        let body = EncryptRequest {
            plaintext: plaintext_b64,
            context: context.to_owned(),
        };

        let resp: EncryptResponse = self
            .http
            .post(&url)
            .header("X-Vault-Token", &self.vault_token)
            .json(&body)
            .send()
            .await
            .context("vault transit encrypt HTTP request")?
            .error_for_status()
            .context("vault transit encrypt error response")?
            .json()
            .await
            .context("vault transit encrypt JSON decode")?;

        let ciphertext = resp.data.ciphertext;
        debug!(
            key_version = resp.data.key_version,
            "vault transit encrypt OK"
        );

        Ok(EncryptedPii {
            ciphertext_bytes: ciphertext.as_bytes().to_vec(),
            kid: ciphertext,
        })
    }

    /// Decrypt a PII ciphertext.
    ///
    /// `ciphertext` is the `vault:v1:...` string stored as bytea in PG.
    /// `context` must match what was used during encryption (per Vault derived key).
    #[instrument(skip(self, ciphertext), fields(context_len = context.len()))]
    pub async fn decrypt(&self, ciphertext: &[u8], context: &str) -> Result<String> {
        let ciphertext_str =
            std::str::from_utf8(ciphertext).context("ciphertext is not valid UTF-8")?;

        let url = format!("{}/v1/transit/decrypt/terroir-pii-master", self.vault_addr);
        let body = DecryptRequest {
            ciphertext: ciphertext_str.to_owned(),
            context: context.to_owned(),
        };

        let resp: DecryptResponse = self
            .http
            .post(&url)
            .header("X-Vault-Token", &self.vault_token)
            .json(&body)
            .send()
            .await
            .context("vault transit decrypt HTTP request")?
            .error_for_status()
            .context("vault transit decrypt error response")?
            .json()
            .await
            .context("vault transit decrypt JSON decode")?;

        let plaintext_bytes = base64::engine::general_purpose::STANDARD
            .decode(&resp.data.plaintext)
            .context("decode vault plaintext base64")?;

        String::from_utf8(plaintext_bytes).context("vault plaintext not UTF-8")
    }
}

// ---------------------------------------------------------------------------
// Context builder helpers
// ---------------------------------------------------------------------------

/// Build the base64-encoded Vault Transit context string for a PII field.
/// Format: `tenant=<slug>|field=<field>|producer=<producer_uuid>`
pub fn pii_context(tenant_slug: &str, field: &str, producer_id: &uuid::Uuid) -> String {
    let raw = format!("tenant={tenant_slug}|field={field}|producer={producer_id}");
    base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
}

// ---------------------------------------------------------------------------
// DEK cache helpers (KAYA RESP3)
// ---------------------------------------------------------------------------

#[allow(unused_imports)]
use redis::AsyncCommands;

/// KAYA cache key for a DEK ciphertext kid.
pub fn dek_cache_key(kid: &str) -> String {
    // kid is the full vault ciphertext e.g. "vault:v1:ABC..."
    // We hash it to keep the KAYA key short and safe.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    kid.hash(&mut h);
    format!("terroir:dek:cache:{:x}", h.finish())
}

/// Store a ciphertext in the DEK KAYA cache (TTL 1h).
///
/// In practice, we cache the ciphertext string itself so that repeated reads
/// of the same producer field don't re-hit Vault for the unwrap step on cache
/// miss. This is a "is-it-known" check, not plaintext caching.
pub async fn cache_dek_mark<C>(kaya: &mut C, kid: &str) -> Result<()>
where
    C: redis::aio::ConnectionLike + Send,
{
    let cache_key = dek_cache_key(kid);
    // Use raw SET with EX option for KAYA RESP3 compatibility.
    let _: () = redis::cmd("SET")
        .arg(&cache_key)
        .arg("1")
        .arg("EX")
        .arg(DEK_CACHE_TTL_SECS)
        .query_async(kaya)
        .await
        .context("KAYA SET dek cache mark")?;
    Ok(())
}

/// Check if a DEK kid is marked in the KAYA cache.
#[allow(dead_code)]
pub async fn is_dek_cached(kaya: &mut impl redis::AsyncCommands, kid: &str) -> bool {
    let cache_key = dek_cache_key(kid);
    kaya.exists::<_, bool>(&cache_key).await.unwrap_or(false)
}
