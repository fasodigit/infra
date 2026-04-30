// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — génération + storage OTP 8 chiffres.
//
// Décision utilisateur Q5 : OTP 8 chiffres (au lieu du 6 classique). Cela
// double l'entropie utile pour la hotline FCFA sans gêner la saisie au
// pavé téléphone (un USSD tient sur 11 chiffres / écran).
//
// Storage KAYA : STRING `terroir:ussd:otp:{msisdn}`. TTL 5 min appliqué
// côté client (KAYA P0 sans EXPIRE — voir `state.rs`).

use anyhow::Context;
use rand::Rng;

use crate::KAYA_PREFIX;
use crate::state::AppState;

/// Génère un OTP 8 chiffres (zero-padded).
pub fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    format!("{:08}", rng.gen_range(0..100_000_000u32))
}

/// Construit la clé KAYA OTP pour un MSISDN donné.
///
/// IMPORTANT : on évite le préfixe `terroir:ussd:simulator:*` ici parce
/// que cet OTP est consommé par le runtime `terroir-ussd` réel en P3+,
/// qui partagera la même key. Le simulator ne fait que produire / vérifier.
pub fn otp_key(msisdn: &str) -> String {
    format!("terroir:ussd:otp:{msisdn}")
}

/// Persiste un OTP en KAYA avec TTL.
pub async fn store_otp(state: &AppState, msisdn: &str, otp: &str) -> anyhow::Result<()> {
    state
        .put_ttl(&otp_key(msisdn), otp.to_string(), state.otp_ttl())
        .await
        .context("store_otp")
}

/// Vérifie un OTP soumis (exact match). Best-effort delete on success
/// pour empêcher replay.
pub async fn verify_otp(state: &AppState, msisdn: &str, submitted: &str) -> anyhow::Result<bool> {
    let key = otp_key(msisdn);
    let stored: Option<String> = state.get_ttl(&key).await?;
    let ok = stored.as_deref() == Some(submitted);
    if ok {
        let _ = state.kaya_del(&key).await;
    }
    Ok(ok)
}

/// Préfixe KAYA simulator (re-export pour confort).
pub const SIMULATOR_PREFIX: &str = KAYA_PREFIX;
