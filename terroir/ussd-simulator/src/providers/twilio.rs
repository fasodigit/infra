// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — mock Twilio (SMS only)
//
// Twilio (https://twilio.com) — pas d'USSD ouest-Africain ; gardé en
// fallback SMS OTP "urgence" tant qu'on n'a pas dérisqué Hub2/AT (cf.
// décision utilisateur Q7 + ADR-003 §Décision).
//
// API surface réelle Twilio (REST 2010-04-01) :
//   POST /2010-04-01/Accounts/{AccountSid}/Messages.json
//   Content-Type: application/x-www-form-urlencoded
//   Body: To, From, Body, …
//   → JSON { sid, status: "queued", to, from, body, … }
//
// On simplifie le path en `/twilio/sms/send` (compat tests Playwright)
// + on accepte aussi JSON pour confort.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::providers::{SmsRecord, record_sms};
use crate::state::AppState;

pub const PROVIDER: &str = "twilio";

#[derive(Debug, Deserialize)]
pub struct TwilioMessageReq {
    #[serde(rename = "To", alias = "to")]
    pub to: String,
    #[serde(rename = "From", alias = "from", default)]
    pub from: String,
    #[serde(rename = "Body", alias = "body")]
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct TwilioMessageRes {
    pub sid: String,
    pub status: &'static str,
    pub to: String,
    pub from: String,
    pub body: String,
    pub date_created: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/sms/send", post(sms_send))
}

async fn sms_send(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let ct = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let req: TwilioMessageReq = if ct.starts_with("application/json") {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(err) => {
                return (StatusCode::BAD_REQUEST, format!("invalid JSON: {err}")).into_response();
            }
        }
    } else {
        match serde_urlencoded::from_bytes(&body) {
            Ok(v) => v,
            Err(err) => {
                return (StatusCode::BAD_REQUEST, format!("invalid form: {err}")).into_response();
            }
        }
    };

    let sms = SmsRecord::new(PROVIDER, req.to.clone(), req.body.clone());
    let id = sms.id.clone();
    let sent_at = sms.sent_at.clone();

    // Persiste aussi dans la liste Twilio-spécifique (cf. ULTRAPLAN
    // demande `terroir:ussd:simulator:twilio:sms:by_to:{to_msisdn}`).
    let twilio_key = format!("{}:twilio:sms:by_to:{}", crate::KAYA_PREFIX, req.to);
    if let Err(err) = state
        .list_lpush_capped(
            &twilio_key,
            sms.clone(),
            crate::state::SMS_LIST_MAX,
            state.sms_ttl(),
        )
        .await
    {
        error!(target: "terroir-ussd-simulator", error = %err, "twilio key lpush");
        return kaya_unavailable();
    }

    if let Err(err) = record_sms(&state, &sms).await {
        error!(target: "terroir-ussd-simulator", error = %err, "record SMS");
        return kaya_unavailable();
    }

    let res = TwilioMessageRes {
        sid: id,
        status: "queued",
        to: req.to,
        from: req.from,
        body: req.body,
        date_created: sent_at,
    };
    (StatusCode::OK, Json(res)).into_response()
}

fn kaya_unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "code": 20500,
            "message": "Simulator backend (KAYA) is unreachable",
            "status": "error",
        })),
    )
        .into_response()
}
