// SPDX-License-Identifier: AGPL-3.0-or-later
//! Router filter — resolves the downstream cluster from request path /
//! headers.
//!
//! **M0 scaffolding**: this is a no-op stub.  The real implementation
//! (port of `src/router.rs`) lands in gate M1 #95.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Router filter — populates `RequestCtx::cluster`.
///
/// Currently a no-op; leaves `ctx.cluster` empty so that the upstream
/// selector falls back to `PingoraGatewayConfig::default_cluster`.
#[derive(Debug, Default)]
pub struct RouterFilter;

impl RouterFilter {
    /// Create a new stub router filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for RouterFilter {
    fn name(&self) -> &'static str {
        "router"
    }

    async fn on_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #95: resolve cluster from `session.req_header().uri.path()` and
        // the static route table; populate `ctx.cluster`.
        Decision::Continue
    }
}
