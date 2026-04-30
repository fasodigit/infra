// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-eudr — bibliothèque
//
// Validation parcelles vs Hansen Global Forest Change v1.11 (mirror MinIO)
// et JRC Tropical Moist Forest (mirror MinIO). Génère les DDS (Due Diligence
// Statement) signées via Vault PKI EORI exportateur, soumises à TRACES NT.
//
// HTTP :8831, gRPC :8731 (cf. INFRA/port-policy.yaml).

#![forbid(unsafe_code)]

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub const HTTP_PORT: u16 = 8831;
pub const GRPC_PORT: u16 = 8731;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }
}
