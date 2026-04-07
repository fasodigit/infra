//! Core trait that all Pentagon security engines must implement.

use crate::context::RequestContext;
use crate::decision::Decision;
use crate::error::Result;
use async_trait::async_trait;

/// Every Pentagon security engine implements this trait.
///
/// Engines run in parallel on each request. Each returns a [`Decision`] which
/// is collected by NEXUS for correlation and final scoring.
#[async_trait]
pub trait SecurityEngine: Send + Sync + 'static {
    /// Engine identifier (e.g. "SENTINEL", "ARBITER").
    fn name(&self) -> &'static str;

    /// Initialize the engine (load rules, connect to external services, etc.).
    async fn init(&mut self) -> Result<()>;

    /// Inspect a request and return a decision.
    async fn inspect(&self, ctx: &RequestContext) -> Result<Decision>;

    /// Gracefully shut down the engine.
    async fn shutdown(&self) -> Result<()>;

    /// Return true if the engine is healthy and ready to serve.
    fn is_ready(&self) -> bool;
}
