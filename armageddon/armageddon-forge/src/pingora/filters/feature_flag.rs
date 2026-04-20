// SPDX-License-Identifier: AGPL-3.0-or-later
//! Feature-flag filter — injects active flags from GrowthBook into the
//! request context.
//!
//! **M0 scaffolding**: this is a no-op stub.  The real implementation
//! (port of `src/feature_flag_filter.rs`) lands in gate M1 #98.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Feature-flag filter — populates `RequestCtx::feature_flags`.
#[derive(Debug, Default)]
pub struct FeatureFlagFilter;

impl FeatureFlagFilter {
    /// Create a new stub feature-flag filter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ForgeFilter for FeatureFlagFilter {
    fn name(&self) -> &'static str {
        "feature_flag"
    }

    async fn on_request(
        &self,
        _session: &mut pingora_proxy::Session,
        _ctx: &mut RequestCtx,
    ) -> Decision {
        // M1 #98: query GrowthBook SSE cache, push active flag names into
        // ctx.feature_flags, and (optionally) set per-request header overrides.
        Decision::Continue
    }
}
