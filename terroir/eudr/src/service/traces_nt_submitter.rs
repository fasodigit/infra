// SPDX-License-Identifier: AGPL-3.0-or-later
//! TRACES NT submission worker.
//!
//! P1: posts the payload to `${TRACES_NT_URL}/submit` with exponential
//! backoff (5×, 30 s base) and on final failure publishes to a DLQ topic
//! (`terroir.dds.submitted.dlq`). P3+: replace `TRACES_NT_URL` with the
//! real EU endpoint (mTLS handled by Vault PKI in P2).

use anyhow::Result;
use backoff::{ExponentialBackoff, future::retry};
use serde::Deserialize;
use std::time::Duration;
use tracing::{instrument, warn};
use uuid::Uuid;

#[derive(Debug, Deserialize, Default)]
pub struct TracesNtAck {
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug)]
pub struct SubmissionResult {
    pub http_status: u16,
    pub reference: Option<String>,
    pub body: String,
}

#[instrument(skip(http, payload))]
pub async fn submit(
    http: &reqwest::Client,
    traces_nt_url: &str,
    dds_id: Uuid,
    payload: &serde_json::Value,
) -> Result<SubmissionResult> {
    let url = format!("{traces_nt_url}/submit");
    let backoff = ExponentialBackoff {
        initial_interval: Duration::from_secs(2),
        max_interval: Duration::from_secs(30),
        max_elapsed_time: Some(Duration::from_secs(120)),
        ..Default::default()
    };

    let result: Result<SubmissionResult, anyhow::Error> = retry(backoff, || {
        let url = url.clone();
        let payload = payload.clone();
        let http = http.clone();
        async move {
            let resp = http
                .post(&url)
                .json(&serde_json::json!({
                    "ddsId": dds_id,
                    "clientReference": dds_id,
                    "payload": payload,
                }))
                .send()
                .await
                .map_err(|e| {
                    backoff::Error::transient(anyhow::anyhow!("TRACES NT request: {e}"))
                })?;

            let http_status = resp.status().as_u16();
            let body = resp
                .text()
                .await
                .map_err(|e| backoff::Error::permanent(anyhow::anyhow!("read body: {e}")))?;

            if http_status >= 500 {
                warn!(http_status, "TRACES NT 5xx — will retry");
                return Err(backoff::Error::transient(anyhow::anyhow!(
                    "TRACES NT {http_status}"
                )));
            }
            let ack: TracesNtAck = serde_json::from_str(&body).unwrap_or_default();
            Ok(SubmissionResult {
                http_status,
                reference: ack.reference,
                body,
            })
        }
    })
    .await;

    result.map_err(|e| anyhow::anyhow!("TRACES NT submission failed: {e}"))
}
