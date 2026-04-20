// SPDX-License-Identifier: AGPL-3.0-or-later
//! VEIL filter — response header hygiene (hide internal stack, strip
//! upstream version banners, add strict-security headers).
//!
//! **M0 scaffolding**: this is a no-op stub.  The real implementation lives
//! in the `armageddon-veil` crate and will be wrapped by this filter in
//! gate M1 #100.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// VEIL filter — response header hygiene.
#[derive(Debug, Default)]
pub struct VeilFilter;

impl VeilFilter {
    /// Create a new stub VEIL filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for VeilFilter {
    fn name(&self) -> &'static str {
        "veil"
    }

    async fn on_response(
        &self,
        _session: &mut pingora_proxy::Session,
        _res: &mut pingora::http::ResponseHeader,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #100: strip Server / X-Powered-By, add Strict-Transport-Security,
        // X-Content-Type-Options, etc.
        Decision::Continue
    }
}
