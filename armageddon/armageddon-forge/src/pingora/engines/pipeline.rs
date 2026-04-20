// SPDX-License-Identifier: AGPL-3.0-or-later
//! Security-engine pipeline evaluation.
//!
//! **M0 scaffolding**: [`evaluate`] is a no-op returning
//! [`Decision::Continue`].  M3 #104 fills in the real fan-out / score
//! aggregation across SENTINEL, ARBITER, ORACLE, AEGIS, AI.

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::Decision;

/// Evaluate the security-engine pipeline for the current request.
///
/// The caller (gateway) invokes this at the end of the filter phase and
/// aborts the request if the returned decision is not `Continue`.
///
/// **M0**: always returns `Decision::Continue`.  No engines are wired.
pub fn evaluate(_ctx: &mut RequestCtx) -> Decision {
    Decision::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_is_noop_in_m0() {
        let mut ctx = RequestCtx::new();
        assert!(matches!(evaluate(&mut ctx), Decision::Continue));
        assert_eq!(ctx.waf_score, 0.0);
        assert_eq!(ctx.ai_score, 0.0);
    }
}
