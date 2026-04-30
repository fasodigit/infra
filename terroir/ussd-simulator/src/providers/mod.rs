// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — adapters mock des 3 providers (Hub2/AT/Twilio)
//
// Chaque provider expose un router Axum qui :
//   - imite la *request shape* de l'API publique (compat tests bas niveau)
//   - imite la *response shape* (compat clients réels en mode mock)
//   - persiste l'état session/SMS en KAYA pour observation
//   - délègue la logique business (menus, OTP) à `flows::*`
//
// La "vérité" du flow est dans `flows::producer_signup` etc. — les
// providers ne font que muxer vers ces flows en passant l'état session.

pub mod africastalking;
pub mod hub2;
pub mod twilio;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::state::{AppState, SMS_LIST_MAX};

/// Représentation canonique d'un SMS qui transite par le simulator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsRecord {
    pub provider: String,
    pub msisdn: String,
    pub body: String,
    pub sent_at: String,
    pub id: String,
}

impl SmsRecord {
    pub fn new(provider: &str, msisdn: String, body: String) -> Self {
        Self {
            provider: provider.to_string(),
            msisdn,
            body,
            sent_at: Utc::now().to_rfc3339(),
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Clé KAYA STRING pour l'historique SMS d'un MSISDN. Préfixe commun
    /// aux 3 providers — ainsi `/admin/last-sms` retourne le dernier SMS
    /// peu importe sa provenance.
    pub fn history_key(msisdn: &str) -> String {
        format!("{}:sms:by_msisdn:{}", crate::KAYA_PREFIX, msisdn)
    }
}

/// Persiste un SMS en KAYA (LPUSH-like client-side, cap 50, TTL 24h).
pub async fn record_sms(state: &AppState, sms: &SmsRecord) -> anyhow::Result<()> {
    state
        .list_lpush_capped(
            &SmsRecord::history_key(&sms.msisdn),
            sms.clone(),
            SMS_LIST_MAX,
            state.sms_ttl(),
        )
        .await
}
