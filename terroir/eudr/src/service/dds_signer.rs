// SPDX-License-Identifier: AGPL-3.0-or-later
//! DDS signer — calls Vault PKI EORI exporter role and computes a stable
//! signature fingerprint over the DDS payload.
//!
//! Vault path: `pki-terroir/issue/eori-exporter` (P0.B). The HTTP body
//! contains `common_name=<eori>.exporter.terroir.bf` and we receive a
//! certificate + private key in the JSON response. We then build a
//! fingerprint as `SHA-256(payload_sha256 || cert_sha256)` to record an
//! auditable bond between the DDS payload and the issuing certificate.
//!
//! For P1 we keep a deliberately simple "fingerprint" instead of a full
//! CMS / X.509 detached signature — this gives us a verifiable hash chain
//! while leaving room for the production-grade CMS impl in P2.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::instrument;

#[derive(Debug, Serialize)]
struct PkiIssueRequest {
    common_name: String,
    ttl: String,
}

#[derive(Debug, Deserialize)]
struct PkiIssueData {
    certificate: String,
    #[allow(dead_code)]
    #[serde(default)]
    private_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PkiIssueResponse {
    data: PkiIssueData,
}

pub struct SignedArtifacts {
    pub fingerprint: String,
    pub cert_pem: String,
}

/// Issue a fresh EORI cert via Vault PKI and produce a payload fingerprint.
#[instrument(skip(http, vault_token, payload_sha256))]
pub async fn sign(
    http: &reqwest::Client,
    vault_addr: &str,
    vault_token: &str,
    pki_role: &str,
    operator_eori: &str,
    payload_sha256: &str,
) -> Result<SignedArtifacts> {
    let url = format!("{vault_addr}/v1/{pki_role}");
    let body = PkiIssueRequest {
        common_name: format!("{operator_eori}.exporter.terroir.bf"),
        ttl: "8760h".into(), // 1 year, aligned with EU TRACES NT cycle
    };

    let resp_value: serde_json::Value = http
        .post(&url)
        .header("X-Vault-Token", vault_token)
        .json(&json!({"common_name": body.common_name, "ttl": body.ttl}))
        .send()
        .await
        .context("Vault PKI HTTP request")?
        .error_for_status()
        .context("Vault PKI returned an error status")?
        .json()
        .await
        .context("Vault PKI JSON decode")?;

    let parsed: PkiIssueResponse =
        serde_json::from_value(resp_value).context("parse Vault PKI response")?;

    let cert_pem = parsed.data.certificate;

    let mut hasher = Sha256::new();
    hasher.update(payload_sha256.as_bytes());
    hasher.update(b"|");
    hasher.update(cert_pem.as_bytes());
    let fingerprint = hex::encode(hasher.finalize());

    Ok(SignedArtifacts {
        fingerprint,
        cert_pem,
    })
}
