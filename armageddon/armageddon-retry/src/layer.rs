// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Tower [`Layer`] and [`Service`] wrappers that compose retry + timeout
//! behaviour into any tower middleware stack used within ARMAGEDDON-forge.
//!
//! Usage:
//! ```rust,ignore
//! let svc = ServiceBuilder::new()
//!     .layer(RetryLayer::new(policy, budget))
//!     .service(upstream_service);
//! ```

use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use crate::budget::RetryBudget;
use crate::policy::RetryPolicy;

// -- RetryLayer --

/// Tower [`Layer`] that wraps a service with retry + budget enforcement.
#[derive(Clone)]
pub struct RetryLayer {
    policy: Arc<RetryPolicy>,
    budget: Arc<RetryBudget>,
}

impl RetryLayer {
    /// Create a new `RetryLayer` from a policy and budget.
    pub fn new(policy: RetryPolicy, budget: RetryBudget) -> Self {
        Self {
            policy: Arc::new(policy),
            budget: Arc::new(budget),
        }
    }
}

impl<S> Layer<S> for RetryLayer {
    type Service = RetryService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RetryService {
            inner,
            policy: Arc::clone(&self.policy),
            budget: Arc::clone(&self.budget),
        }
    }
}

// -- RetryService --

/// Tower [`Service`] that applies retry + budget logic around the inner service.
///
/// This is intentionally kept as a thin wrapper — the heavy retry loop lives in
/// `execute_with_retry` (see `lib.rs`).  The `Service` impl delegates to the
/// inner service and exposes the retry policy for downstream middleware to
/// inspect via `.policy()`.
#[derive(Clone)]
pub struct RetryService<S> {
    inner: S,
    policy: Arc<RetryPolicy>,
    budget: Arc<RetryBudget>,
}

impl<S> RetryService<S> {
    /// Expose the shared retry policy for inspection.
    pub fn policy(&self) -> &RetryPolicy {
        &self.policy
    }

    /// Expose the shared retry budget for inspection.
    pub fn budget(&self) -> &RetryBudget {
        &self.budget
    }
}

impl<S, Req> Service<Req> for RetryService<S>
where
    S: Service<Req>,
    S::Future: Send + 'static,
    Req: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    /// Dispatch the request to the inner service.
    ///
    /// NOTE: Full retry looping with backoff / budget occurs in
    /// [`execute_with_retry`](crate::execute_with_retry).  This `call`
    /// implementation forwards a single attempt so that `RetryService` can
    /// compose cleanly with other tower layers while higher-level code drives
    /// the retry loop explicitly.
    fn call(&mut self, req: Req) -> Self::Future {
        self.inner.call(req)
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use std::future::{ready, Ready};
    use tower::ServiceExt;

    // Minimal no-op service for layer composition tests.
    #[derive(Clone)]
    struct EchoService;

    impl Service<u32> for EchoService {
        type Response = u32;
        type Error = Infallible;
        type Future = Ready<Result<u32, Infallible>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: u32) -> Self::Future {
            ready(Ok(req))
        }
    }

    #[tokio::test]
    async fn layer_wraps_service_transparently() {
        let layer = RetryLayer::new(RetryPolicy::default(), RetryBudget::default());
        let mut svc = layer.layer(EchoService);
        let resp = svc.ready().await.unwrap().call(42u32).await.unwrap();
        assert_eq!(resp, 42);
    }

    #[test]
    fn layer_exposes_policy_and_budget() {
        let policy = RetryPolicy::default();
        let budget = RetryBudget::new(0.20, 5);
        let layer = RetryLayer::new(policy, budget);
        let svc = layer.layer(EchoService);
        assert_eq!(svc.policy().max_retries, 2);
        assert_eq!(svc.budget().min_retry_concurrency, 5);
    }
}
