// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-buyer — bibliothèque
//
// Portail acheteurs invitation-only (P3) : catalog public, contract signing
// avec escrow + Vault PKI signature, DDS download signé.

#![forbid(unsafe_code)]

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub const HTTP_PORT: u16 = 8835;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }
}
