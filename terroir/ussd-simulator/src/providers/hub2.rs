// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — mock Hub2 (USSD + SMS)
//
// Hub2 (https://hub2.io) — couverture francophone UEMOA, primary BF/ML/SN
// dans la matrice de routage ADR-003.
//
// API surface mockée :
//   POST /hub2/ussd/push  body { msisdn, session_id, text, level }
//                         → { sessionId, status: "OK", message }
//   POST /hub2/sms/send   body { msisdn, message }
//                         → { id, status: "QUEUED" }
//
// Note : le flow `text` réel Hub2 concatène les inputs comme AT
// (`"1*514412*Boukary"`). Pour la version P0 on prend le **dernier** segment
// après le dernier `*` comme entrée courante (heuristique safe pour un
// mock — la vraie intégration Hub2 P3 se substituera).

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::flows;
use crate::providers::{SmsRecord, record_sms};
use crate::state::AppState;

pub const PROVIDER: &str = "hub2";

#[derive(Debug, Deserialize)]
pub struct UssdPushReq {
    pub msisdn: String,
    pub session_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default = "default_level")]
    pub level: u8,
}

fn default_level() -> u8 {
    1
}

#[derive(Debug, Serialize)]
pub struct UssdPushRes {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub status: &'static str,
    pub message: String,
    pub end: bool,
    pub level: u8,
}

#[derive(Debug, Deserialize)]
pub struct SmsSendReq {
    pub msisdn: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SmsSendRes {
    pub id: String,
    pub status: &'static str,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ussd/push", post(ussd_push))
        .route("/sms/send", post(sms_send))
}

async fn ussd_push(
    State(state): State<AppState>,
    Json(req): Json<UssdPushReq>,
) -> impl IntoResponse {
    // Hub2 réel : level fourni par client, on l'utilise comme info, mais
    // on s'appuie en priorité sur la session KAYA pour la state machine.
    let mut session =
        match flows::load_or_init_session(&state, PROVIDER, &req.session_id, &req.msisdn).await {
            Ok(s) => s,
            Err(err) => {
                error!(target: "terroir-ussd-simulator", error = %err, "load session");
                return kaya_unavailable();
            }
        };

    // Hub2 envoie l'historique d'inputs concaténé par `*` ; on récupère
    // le dernier segment.
    let input = req.text.rsplit('*').next().unwrap_or("").to_string();

    let reply = match flows::step(&state, PROVIDER, &mut session, &input).await {
        Ok(r) => r,
        Err(err) => {
            error!(target: "terroir-ussd-simulator", error = %err, "flow step");
            return kaya_unavailable();
        }
    };

    if let Err(err) = flows::persist_session(&state, PROVIDER, &req.session_id, &session).await {
        error!(target: "terroir-ussd-simulator", error = %err, "persist session");
        return kaya_unavailable();
    }

    let res = UssdPushRes {
        session_id: req.session_id,
        status: "OK",
        message: reply.message,
        end: reply.end,
        level: if reply.end { 0 } else { session.level },
    };
    (StatusCode::OK, Json(res)).into_response()
}

async fn sms_send(State(state): State<AppState>, Json(req): Json<SmsSendReq>) -> impl IntoResponse {
    let sms = SmsRecord::new(PROVIDER, req.msisdn.clone(), req.message);
    let id = sms.id.clone();
    if let Err(err) = record_sms(&state, &sms).await {
        error!(target: "terroir-ussd-simulator", error = %err, "record SMS");
        return kaya_unavailable();
    }
    (
        StatusCode::OK,
        Json(SmsSendRes {
            id,
            status: "QUEUED",
        }),
    )
        .into_response()
}

fn kaya_unavailable() -> axum::response::Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "status": "ERROR",
            "code": "KAYA_UNAVAILABLE",
            "message": "Simulator backend (KAYA) is unreachable",
        })),
    )
        .into_response()
}
