// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — mock Africa's Talking (USSD + SMS)
//
// Africa's Talking (https://africastalking.com) — primary CI/SN, fallback
// BF/ML dans la matrice ADR-003.
//
// API surface AT réelle :
//   - USSD callback : POST text/plain body form-encoded :
//       sessionId, phoneNumber, networkCode, text  ("1*514412*Boukary")
//     → response text/plain commençant par "CON " (continue) ou "END "
//   - SMS send : POST application/x-www-form-urlencoded
//       username, to, message
//     → JSON { SMSMessageData: { Recipients: [{ messageId, status, ... }] } }
//
// On reproduit ce contrat (mais on accepte aussi JSON pour confort tests
// Playwright).

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::flows;
use crate::providers::{SmsRecord, record_sms};
use crate::state::AppState;

pub const PROVIDER: &str = "africastalking";

#[derive(Debug, Deserialize)]
pub struct UssdMenuReq {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "phoneNumber")]
    pub phone_number: String,
    #[serde(rename = "networkCode", default)]
    pub network_code: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct SmsSendForm {
    #[serde(default)]
    pub username: String,
    pub to: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SmsRecipient {
    #[serde(rename = "messageId")]
    pub message_id: String,
    pub status: &'static str,
    #[serde(rename = "phoneNumber")]
    pub phone_number: String,
    pub cost: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SmsMessageData {
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Recipients")]
    pub recipients: Vec<SmsRecipient>,
}

#[derive(Debug, Serialize)]
pub struct SmsSendRes {
    #[serde(rename = "SMSMessageData")]
    pub sms_message_data: SmsMessageData,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ussd/menu", post(ussd_menu))
        .route("/sms/send", post(sms_send))
}

/// AT accepte form-urlencoded ou JSON ; on dispatch sur Content-Type.
async fn ussd_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let ct = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let req: UssdMenuReq = if ct.starts_with("application/json") {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(err) => {
                return (StatusCode::BAD_REQUEST, format!("invalid JSON: {err}")).into_response();
            }
        }
    } else {
        // form-urlencoded (mode AT prod)
        match serde_urlencoded::from_bytes(&body) {
            Ok(v) => v,
            Err(err) => {
                return (StatusCode::BAD_REQUEST, format!("invalid form: {err}")).into_response();
            }
        }
    };

    let mut session =
        match flows::load_or_init_session(&state, PROVIDER, &req.session_id, &req.phone_number)
            .await
        {
            Ok(s) => s,
            Err(err) => {
                error!(target: "terroir-ussd-simulator", error = %err, "load session");
                return kaya_unavailable_text();
            }
        };

    let input = req.text.rsplit('*').next().unwrap_or("").to_string();

    let reply = match flows::step(&state, PROVIDER, &mut session, &input).await {
        Ok(r) => r,
        Err(err) => {
            error!(target: "terroir-ussd-simulator", error = %err, "flow step");
            return kaya_unavailable_text();
        }
    };

    if let Err(err) = flows::persist_session(&state, PROVIDER, &req.session_id, &session).await {
        error!(target: "terroir-ussd-simulator", error = %err, "persist session");
        return kaya_unavailable_text();
    }

    // Format AT : "CON ..." ou "END ..."
    let prefix = if reply.end { "END" } else { "CON" };
    let body = format!("{prefix} {}", reply.message);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
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
    let req: SmsSendForm = if ct.starts_with("application/json") {
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

    let sms = SmsRecord::new(PROVIDER, req.to.clone(), req.message);
    let id = sms.id.clone();
    if let Err(err) = record_sms(&state, &sms).await {
        error!(target: "terroir-ussd-simulator", error = %err, "record SMS");
        return kaya_unavailable_text();
    }
    let res = SmsSendRes {
        sms_message_data: SmsMessageData {
            message: "Sent to 1/1 Total Cost: USD 0".to_string(),
            recipients: vec![SmsRecipient {
                message_id: id,
                status: "Success",
                phone_number: req.to,
                cost: "USD 0.0000",
            }],
        },
    };
    (StatusCode::OK, Json(res)).into_response()
}

fn kaya_unavailable_text() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "ERROR KAYA_UNAVAILABLE",
    )
        .into_response()
}
