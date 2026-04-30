// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-ussd — bibliothèque
//
// Gateway USSD/SMS (P3+). Pendant P0-P2, stack utilise terroir-ussd-simulator
// uniquement (cf. ADR-003). Décision providers réels (Hub2 / Africa's Talking
// / Twilio) reportée au gate G_ussd entrée P3 (cf. ULTRAPLAN §15).

#![forbid(unsafe_code)]

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub const HTTP_PORT: u16 = 8834;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }
}
