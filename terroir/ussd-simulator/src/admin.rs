// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — endpoints admin / test (loopback only)
//
// Cible primaire : suite Playwright `tests-e2e/19-terroir/`. Permet de
// récupérer le dernier OTP SMS pour un MSISDN (équivalent
// `MailpitClient.waitForOtp`), inspecter une session USSD, et wiper KAYA
// entre deux specs.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::flows::{self, SessionState};
use crate::providers::SmsRecord;
use crate::state::AppState;

/// Regex extraction OTP (8 chiffres) du body SMS (cf. décision Q5).
static OTP_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{8}\b").expect("static OTP regex compiles"));

#[derive(Debug, Deserialize)]
pub struct LastSmsQuery {
    pub msisdn: String,
}

#[derive(Debug, Serialize)]
pub struct LastSmsRes {
    pub provider: String,
    pub msisdn: String,
    pub body: String,
    pub sent_at: String,
    pub otp_extracted: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SmsHistoryQuery {
    pub msisdn: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    20
}

#[derive(Debug, Serialize)]
pub struct SmsHistoryRes {
    pub msisdn: String,
    pub count: usize,
    pub items: Vec<LastSmsRes>,
}

#[derive(Debug, Serialize)]
pub struct SessionRes {
    pub provider: String,
    pub session_id: String,
    pub state: SessionStateView,
}

#[derive(Debug, Serialize)]
pub struct SessionStateView {
    pub msisdn: String,
    pub level: u8,
    pub flow: Option<String>,
    pub last_input: String,
    pub nin: Option<String>,
    pub full_name: Option<String>,
    pub started_at: String,
}

impl From<SessionState> for SessionStateView {
    fn from(s: SessionState) -> Self {
        Self {
            msisdn: s.msisdn,
            level: s.level,
            flow: s.flow.map(|f| f.as_str().to_string()),
            last_input: s.last_input,
            nin: s.nin,
            full_name: s.full_name,
            started_at: s.started_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ClearRes {
    pub deleted: u64,
    pub prefix: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/last-sms", get(last_sms))
        .route("/sms-history", get(sms_history))
        .route("/sessions/{provider}/{session_id}", get(session_inspect))
        .route("/clear", post(clear))
}

async fn last_sms(State(state): State<AppState>, Query(q): Query<LastSmsQuery>) -> Response {
    let key = SmsRecord::history_key(&q.msisdn);
    match state.list_range::<SmsRecord>(&key, 1).await {
        Ok(items) => {
            let Some(rec) = items.into_iter().next() else {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": "no SMS recorded for this MSISDN",
                        "msisdn": q.msisdn,
                    })),
                )
                    .into_response();
            };
            let res = LastSmsRes {
                provider: rec.provider,
                msisdn: rec.msisdn,
                otp_extracted: extract_otp(&rec.body),
                body: rec.body,
                sent_at: rec.sent_at,
            };
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(err) => {
            error!(target: "terroir-ussd-simulator", error = %err, "last_sms");
            kaya_unavailable()
        }
    }
}

async fn sms_history(State(state): State<AppState>, Query(q): Query<SmsHistoryQuery>) -> Response {
    let key = SmsRecord::history_key(&q.msisdn);
    let limit = q.limit.clamp(1, crate::state::SMS_LIST_MAX as i64) as usize;
    match state.list_range::<SmsRecord>(&key, limit).await {
        Ok(items) => {
            let out: Vec<LastSmsRes> = items
                .into_iter()
                .map(|rec| LastSmsRes {
                    provider: rec.provider,
                    otp_extracted: extract_otp(&rec.body),
                    msisdn: rec.msisdn,
                    body: rec.body,
                    sent_at: rec.sent_at,
                })
                .collect();
            let res = SmsHistoryRes {
                msisdn: q.msisdn,
                count: out.len(),
                items: out,
            };
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(err) => {
            error!(target: "terroir-ussd-simulator", error = %err, "sms_history");
            kaya_unavailable()
        }
    }
}

async fn session_inspect(
    State(state): State<AppState>,
    Path((provider, session_id)): Path<(String, String)>,
) -> Response {
    let key = flows::session_key(&provider, &session_id);
    match state.get_ttl::<SessionState>(&key).await {
        Ok(Some(session)) => {
            let res = SessionRes {
                provider,
                session_id,
                state: session.into(),
            };
            (StatusCode::OK, Json(res)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "session not found",
                "provider": provider,
                "session_id": session_id,
            })),
        )
            .into_response(),
        Err(err) => {
            error!(target: "terroir-ussd-simulator", error = %err, "session_inspect");
            kaya_unavailable()
        }
    }
}

async fn clear(State(state): State<AppState>) -> Response {
    let prefix = format!("{}:", crate::KAYA_PREFIX);
    match state.wipe_all().await {
        Ok(deleted) => (
            StatusCode::OK,
            Json(ClearRes {
                deleted,
                prefix: prefix.clone(),
            }),
        )
            .into_response(),
        Err(err) => {
            error!(target: "terroir-ussd-simulator", error = %err, "clear");
            kaya_unavailable()
        }
    }
}

fn extract_otp(body: &str) -> Option<String> {
    OTP_REGEX.find(body).map(|m| m.as_str().to_string())
}

fn kaya_unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": "KAYA backend unavailable",
            "code": "KAYA_UNAVAILABLE",
        })),
    )
        .into_response()
}
