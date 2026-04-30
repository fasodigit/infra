// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-admin — library root.
//
// Admin API loopback :9904 — feature flags, tenant onboarding (POST /admin/tenants),
// debug, healthcheck. Added by P0.C (multi-tenancy foundation).
//
// Conformité port-policy R-03 : loopback-only (bind 127.0.0.1).

#![forbid(unsafe_code)]

pub mod dto;
pub mod routes;
pub mod tenant_service;
pub mod tenant_template;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Port admin loopback (cf. INFRA/port-policy.yaml).
pub const HTTP_PORT: u16 = 9904;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }

    #[test]
    fn slug_validation_accepts_valid() {
        let req = dto::CreateTenantRequest {
            slug: "t_pilot".to_string(),
            legal_name: "Coopérative Pilote".to_string(),
            country_iso2: "BF".to_string(),
            region: Some("Boucle du Mouhoun".to_string()),
            primary_crop: "coton".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn slug_validation_rejects_uppercase() {
        let req = dto::CreateTenantRequest {
            slug: "T_PILOT".to_string(),
            legal_name: "Coopérative".to_string(),
            country_iso2: "BF".to_string(),
            region: None,
            primary_crop: "coton".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn slug_validation_rejects_too_short() {
        let req = dto::CreateTenantRequest {
            slug: "ab".to_string(),
            legal_name: "Coopérative".to_string(),
            country_iso2: "BF".to_string(),
            region: None,
            primary_crop: "coton".to_string(),
        };
        assert!(req.validate().is_err());
    }
}
