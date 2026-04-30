// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd-simulator — bibliothèque
//
// Mock loopback des 3 providers (Hub2, Africa's Talking, Twilio) avec
// state KAYA `terroir:ussd:simulator:*` et endpoint `/admin/last-sms`
// pour Playwright (capture OTP comme Mailpit).
//
// Implémentation P0.F (livrable P0.6 — cf. ULTRAPLAN §4 et ADR-003).
// Loopback only — bind 127.0.0.1:1080. Mock indispensable au flow
// Playwright `terroir-ussd-simulator-roundtrip.spec.ts` (P0.I).

#![forbid(unsafe_code)]

pub mod admin;
pub mod flows;
pub mod otp;
pub mod providers;
pub mod state;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Port loopback par défaut (binding 127.0.0.1, hors port-policy "allocations"
/// car simulateur dev/test). Choisi dans la plage dev-tools (1000-1099).
pub const HTTP_PORT: u16 = 1080;

/// Préfixe KAYA commun à toutes les keys du simulator.
pub const KAYA_PREFIX: &str = "terroir:ussd:simulator";

/// Variable d'environnement pour la connection KAYA (RESP3 sur :6380).
pub const KAYA_URL_ENV: &str = "TERROIR_KAYA_URL";
pub const KAYA_URL_DEFAULT: &str = "redis://127.0.0.1:6380/0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }
}
