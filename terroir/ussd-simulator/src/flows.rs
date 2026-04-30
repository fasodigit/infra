// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — flows métier mock (state machines USSD)
//
// On modélise les sessions USSD avec une "state machine" naïve : le numéro
// de niveau (`level`) et le dernier input (`last_input`) sont stockés en
// KAYA STRING (JSON sérialisé sous TtlEnvelope) ; chaque appel d'un router
// provider re-rentre dans `step()` qui décide la prochaine réponse +
// transition. Cf. `state.rs` pour l'explication du modèle string-only.
//
// Flows implémentés (P0.6 ULTRAPLAN §4) :
//   1. `producer-signup`    — *XXX# → menu → NIN → Nom → OTP 8 chiffres → END
//   2. `payment-confirmation` — confirmation paiement mobile money
//
// Pour l'event Redpanda (P3 réel) on stub avec un `tracing::info!` —
// remplaçable plus tard par la lib KAYA producer quand elle existera.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::otp;
use crate::providers::{SmsRecord, record_sms};
use crate::state::AppState;

/// Identifiant haut niveau d'un flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowKind {
    ProducerSignup,
    PaymentConfirmation,
}

impl FlowKind {
    pub fn from_root_choice(choice: &str) -> Option<Self> {
        match choice.trim() {
            "1" => Some(Self::ProducerSignup),
            "2" => Some(Self::PaymentConfirmation),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProducerSignup => "producer-signup",
            Self::PaymentConfirmation => "payment-confirmation",
        }
    }
}

/// Réponse normalisée d'une étape de flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowReply {
    pub message: String,
    pub end: bool,
    pub next_level: u8,
}

impl FlowReply {
    fn con(message: impl Into<String>, next_level: u8) -> Self {
        Self {
            message: message.into(),
            end: false,
            next_level,
        }
    }
    fn end(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            end: true,
            next_level: 0,
        }
    }
}

/// Signature d'une session USSD — sérialisée JSON dans KAYA.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub msisdn: String,
    pub level: u8,
    pub flow: Option<FlowKind>,
    pub last_input: String,
    pub nin: Option<String>,
    pub full_name: Option<String>,
    pub started_at: String,
}

impl SessionState {
    pub fn new(msisdn: String) -> Self {
        Self {
            msisdn,
            level: 1,
            flow: None,
            last_input: String::new(),
            nin: None,
            full_name: None,
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Construit la clé KAYA STRING pour une session d'un provider donné.
pub fn session_key(provider: &str, session_id: &str) -> String {
    format!("{}:{}:session:{}", crate::KAYA_PREFIX, provider, session_id)
}

/// Charge ou initialise une session USSD.
pub async fn load_or_init_session(
    state: &AppState,
    provider: &str,
    session_id: &str,
    msisdn: &str,
) -> anyhow::Result<SessionState> {
    let key = session_key(provider, session_id);
    if let Some(s) = state.get_ttl::<SessionState>(&key).await? {
        return Ok(s);
    }
    Ok(SessionState::new(msisdn.to_string()))
}

/// Persiste une session après transition.
pub async fn persist_session(
    state: &AppState,
    provider: &str,
    session_id: &str,
    session: &SessionState,
) -> anyhow::Result<()> {
    let key = session_key(provider, session_id);
    state
        .put_ttl(&key, session.clone(), state.session_ttl())
        .await
        .context("persist session")
}

/// Étape de la state-machine.
pub async fn step(
    state: &AppState,
    provider: &'static str,
    session: &mut SessionState,
    input: &str,
) -> anyhow::Result<FlowReply> {
    let trimmed = input.trim();
    session.last_input = trimmed.to_string();

    if session.flow.is_none() {
        if trimmed.is_empty() {
            session.level = 1;
            return Ok(FlowReply::con(
                "Bienvenue cooperative TERROIR\n\
                1. S'inscrire (producteur)\n\
                2. Confirmer paiement\n\
                3. Mon solde",
                1,
            ));
        }
        match FlowKind::from_root_choice(trimmed) {
            Some(FlowKind::ProducerSignup) => {
                session.flow = Some(FlowKind::ProducerSignup);
                session.level = 2;
                return Ok(FlowReply::con("Entrez votre numero CNIB (NIN)", 2));
            }
            Some(FlowKind::PaymentConfirmation) => {
                session.flow = Some(FlowKind::PaymentConfirmation);
                session.level = 2;
                return Ok(FlowReply::con(
                    "Entrez le code de transaction Mobile Money",
                    2,
                ));
            }
            None if trimmed == "3" => {
                return Ok(FlowReply::end("Solde indisponible (mock)"));
            }
            _ => {
                return Ok(FlowReply::end("Choix invalide"));
            }
        }
    }

    match session.flow.expect("flow set above") {
        FlowKind::ProducerSignup => step_producer_signup(state, provider, session).await,
        FlowKind::PaymentConfirmation => step_payment_confirmation(state, provider, session).await,
    }
}

async fn step_producer_signup(
    state: &AppState,
    provider: &'static str,
    session: &mut SessionState,
) -> anyhow::Result<FlowReply> {
    match session.level {
        2 => {
            let nin = session.last_input.clone();
            if nin.len() < 6 {
                return Ok(FlowReply::end("NIN invalide"));
            }
            session.nin = Some(nin);
            session.level = 3;
            Ok(FlowReply::con("Entrez votre nom complet", 3))
        }
        3 => {
            let name = session.last_input.clone();
            if name.is_empty() {
                return Ok(FlowReply::end("Nom invalide"));
            }
            session.full_name = Some(name);

            let code = otp::generate_otp();
            otp::store_otp(state, &session.msisdn, &code)
                .await
                .context("persist OTP")?;
            let sms_body = format!(
                "TERROIR: votre code de validation est {code}. \
                 Valable 5 minutes."
            );
            let sms = SmsRecord::new(provider, session.msisdn.clone(), sms_body);
            record_sms(state, &sms).await.context("record SMS OTP")?;

            info!(
                target: "terroir-ussd-simulator",
                event = "terroir.ussd.otp.sent",
                provider = provider,
                msisdn = %session.msisdn,
                otp_len = code.len(),
                "OTP emitted"
            );

            session.level = 4;
            Ok(FlowReply::con(
                "Code OTP envoye par SMS. Saisissez-le pour valider.",
                4,
            ))
        }
        4 => {
            let submitted = session.last_input.clone();
            let ok = otp::verify_otp(state, &session.msisdn, &submitted).await?;
            if ok {
                info!(
                    target: "terroir-ussd-simulator",
                    event = "terroir.ussd.otp.verified",
                    provider = provider,
                    msisdn = %session.msisdn,
                    "OTP verified"
                );
                Ok(FlowReply::end(
                    "Inscription validee. Bienvenue dans la cooperative.",
                ))
            } else {
                Ok(FlowReply::end("Code OTP invalide ou expire"))
            }
        }
        _ => Ok(FlowReply::end("Etat de session inconnu")),
    }
}

async fn step_payment_confirmation(
    state: &AppState,
    provider: &'static str,
    session: &mut SessionState,
) -> anyhow::Result<FlowReply> {
    match session.level {
        2 => {
            let tx = session.last_input.clone();
            if tx.is_empty() {
                return Ok(FlowReply::end("Code transaction invalide"));
            }
            let code = otp::generate_otp();
            otp::store_otp(state, &session.msisdn, &code).await?;
            let body = format!(
                "TERROIR: code de confirmation paiement {code} \
                 (transaction {tx}). Valable 5 minutes."
            );
            let sms = SmsRecord::new(provider, session.msisdn.clone(), body);
            record_sms(state, &sms).await?;
            info!(
                target: "terroir-ussd-simulator",
                event = "terroir.ussd.otp.sent",
                provider = provider,
                msisdn = %session.msisdn,
                flow = "payment-confirmation",
                "OTP emitted"
            );
            session.level = 3;
            Ok(FlowReply::con("Saisissez le code de confirmation", 3))
        }
        3 => {
            let submitted = session.last_input.clone();
            let ok = otp::verify_otp(state, &session.msisdn, &submitted).await?;
            if ok {
                info!(
                    target: "terroir-ussd-simulator",
                    event = "terroir.ussd.otp.verified",
                    provider = provider,
                    msisdn = %session.msisdn,
                    flow = "payment-confirmation",
                    "OTP verified"
                );
                Ok(FlowReply::end("Paiement confirme. Merci."))
            } else {
                Ok(FlowReply::end("Code invalide ou expire"))
            }
        }
        _ => Ok(FlowReply::end("Etat de session inconnu")),
    }
}
