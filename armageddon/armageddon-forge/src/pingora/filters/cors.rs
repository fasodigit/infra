// SPDX-License-Identifier: AGPL-3.0-or-later
//! CORS filter — handles pre-flight and injects the origin-allow headers.
//!
//! **M0 scaffolding**: this is a no-op stub.  The real implementation
//! (port of `src/cors.rs`) lands in gate M1 #96.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// CORS filter — per-platform origin policy enforcement.
#[derive(Debug, Default)]
pub struct CorsFilter;

impl CorsFilter {
    /// Create a new stub CORS filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for CorsFilter {
    fn name(&self) -> &'static str {
        "cors"
    }

    async fn on_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #96: short-circuit OPTIONS pre-flight with a CORS ACK response.
        Decision::Continue
    }

    async fn on_response(
        &self,
        _session: &mut pingora_proxy::Session,
        _res: &mut pingora::http::ResponseHeader,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #96: inject Access-Control-Allow-Origin on the downstream response.
        Decision::Continue
    }
}
