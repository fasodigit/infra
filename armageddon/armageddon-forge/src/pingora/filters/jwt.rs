// SPDX-License-Identifier: AGPL-3.0-or-later
//! JWT filter — ES384 validation + claims extraction.
//!
//! **M0 scaffolding**: this is a no-op stub.  The real implementation
//! (port of `src/jwt.rs`) lands in gate M1 #97.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// JWT filter — validates the Bearer token, populates `user_id`,
/// `tenant_id`, `roles`.
#[derive(Debug, Default)]
pub struct JwtFilter;

impl JwtFilter {
    /// Create a new stub JWT filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for JwtFilter {
    fn name(&self) -> &'static str {
        "jwt"
    }

    async fn on_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #97: extract `Authorization: Bearer …`, validate against JWKS
        // cached in KAYA (via the runtime bridge), populate ctx user/tenant.
        // On failure: return `Decision::Deny(401)`.
        Decision::Continue
    }
}
