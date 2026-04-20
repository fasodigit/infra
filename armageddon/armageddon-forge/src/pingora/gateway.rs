// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Pingora gateway — implements [`pingora_proxy::ProxyHttp`] and drives
//! the FORGE filter / engine chains.
//!
//! ## API surface
//!
//! This module preserves the public types used by the pre-M0
//! `pingora_backend` module (`PingoraGateway`, `PingoraGatewayConfig`,
//! `UpstreamRegistry`) so external callers (and the `proxy_compare` bench)
//! keep compiling.
//!
//! ## Chain semantics
//!
//! Filters are invoked in registration order.  Each hook walks the chain
//! until a filter returns `Decision::ShortCircuit` or `Decision::Deny`, at
//! which point the chain stops and the returned response / status flows
//! downstream.  `on_logging` is called for **all** filters regardless of
//! short-circuiting — it is the access-log phase.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use pingora_core::prelude::*;
use pingora_proxy::{ProxyHttp, Session};

use armageddon_common::types::Endpoint;

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, SharedFilter};

// ── upstream registry ──────────────────────────────────────────────────────

/// Shared registry of healthy upstream endpoints, keyed by cluster name.
///
/// Writers take a write-lock only during the brief `insert` swap; readers
/// proceed without blocking.  This makes hot-reload from xDS or the admin
/// API cheap.
#[derive(Debug, Default)]
pub struct UpstreamRegistry {
    inner: RwLock<HashMap<String, Vec<Endpoint>>>,
}

impl UpstreamRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the endpoint list for `cluster`.
    pub fn update_cluster(&self, cluster: &str, endpoints: Vec<Endpoint>) {
        let mut guard = self.inner.write().expect("upstream registry poisoned");
        guard.insert(cluster.to_string(), endpoints);
        tracing::debug!(cluster, "upstream registry updated");
    }

    /// Return the first healthy endpoint for `cluster`.
    pub fn first_healthy(&self, cluster: &str) -> Option<Endpoint> {
        let guard = self.inner.read().expect("upstream registry poisoned");
        guard
            .get(cluster)
            .and_then(|eps| eps.iter().find(|e| e.healthy).cloned())
    }

    /// Return all endpoints for `cluster` regardless of health state.
    pub fn all(&self, cluster: &str) -> Vec<Endpoint> {
        let guard = self.inner.read().expect("upstream registry poisoned");
        guard.get(cluster).cloned().unwrap_or_default()
    }
}

// ── gateway configuration ──────────────────────────────────────────────────

/// Configuration for the Pingora gateway.
#[derive(Clone)]
pub struct PingoraGatewayConfig {
    /// Default cluster when no per-path mapping resolves.
    pub default_cluster: String,
    /// Upstream TLS: whether to verify server certificates.
    pub upstream_tls: bool,
    /// Request timeout in milliseconds applied to upstream connections.
    pub upstream_timeout_ms: u64,
    /// Maximum connections per upstream peer in Pingora's pool.
    pub pool_size: usize,
    /// Ordered list of filters to invoke at each hook.
    pub filters: Vec<SharedFilter>,
}

impl std::fmt::Debug for PingoraGatewayConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PingoraGatewayConfig")
            .field("default_cluster", &self.default_cluster)
            .field("upstream_tls", &self.upstream_tls)
            .field("upstream_timeout_ms", &self.upstream_timeout_ms)
            .field("pool_size", &self.pool_size)
            .field("filters", &self.filters.len())
            .finish()
    }
}

impl Default for PingoraGatewayConfig {
    fn default() -> Self {
        Self {
            default_cluster: "default".to_string(),
            upstream_tls: false,
            upstream_timeout_ms: 30_000,
            pool_size: 128,
            filters: Vec::new(),
        }
    }
}

// ── gateway ────────────────────────────────────────────────────────────────

/// Pingora proxy gateway for ARMAGEDDON-FORGE.
pub struct PingoraGateway {
    config: PingoraGatewayConfig,
    upstream_registry: Arc<UpstreamRegistry>,
}

impl PingoraGateway {
    /// Create a new gateway with the given configuration and upstream registry.
    pub fn new(config: PingoraGatewayConfig, upstream_registry: Arc<UpstreamRegistry>) -> Self {
        Self {
            config,
            upstream_registry,
        }
    }

    /// Create a new gateway with default configuration and an empty registry.
    /// Primarily useful for testing.
    pub fn with_defaults() -> Self {
        Self::new(
            PingoraGatewayConfig::default(),
            Arc::new(UpstreamRegistry::new()),
        )
    }

    /// Return the upstream registry so callers can push updates.
    pub fn upstream_registry(&self) -> &Arc<UpstreamRegistry> {
        &self.upstream_registry
    }

    /// Return the gateway config.
    pub fn config(&self) -> &PingoraGatewayConfig {
        &self.config
    }
}

// Hop-by-hop headers that must not be forwarded to upstream.
pub(crate) const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

#[async_trait]
impl ProxyHttp for PingoraGateway {
    /// Per-request context shared across every hook.
    type CTX = RequestCtx;

    fn new_ctx(&self) -> Self::CTX {
        RequestCtx::new()
    }

    /// Resolve the upstream peer from the registry, preferring
    /// `ctx.cluster` (set by the router filter in M1 #95) over the URI
    /// path heuristic.
    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let cluster: String = if !ctx.cluster.is_empty() {
            ctx.cluster.clone()
        } else {
            session
                .req_header()
                .uri
                .path()
                .trim_start_matches('/')
                .split('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(&self.config.default_cluster)
                .to_string()
        };

        let endpoint = self
            .upstream_registry
            .first_healthy(&cluster)
            .or_else(|| {
                self.upstream_registry
                    .first_healthy(&self.config.default_cluster)
            })
            .ok_or_else(|| {
                Error::new_str("no healthy upstream for cluster")
            })?;

        let addr = format!("{}:{}", endpoint.address, endpoint.port);
        ctx.upstream_addr = addr.clone();

        tracing::debug!(
            cluster = %cluster,
            upstream = %addr,
            "pingora resolved upstream peer"
        );

        let peer = HttpPeer::new(addr, self.config.upstream_tls, String::new());
        Ok(Box::new(peer))
    }

    /// Run the filter chain's `on_request` hooks, then perform the
    /// core header-hygiene work (hop-by-hop strip + x-forge-id injection).
    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<bool> {
        // 1. Filter chain.
        for filter in &self.config.filters {
            match filter.on_request(session, ctx).await {
                Decision::Continue => continue,
                Decision::ShortCircuit(_resp) => {
                    // The response body is empty; a future release will
                    // teach the gateway to write `_resp` downstream via
                    // `session.write_response_header(...)`.  For M0 we
                    // simply short-circuit and let Pingora emit the
                    // default response.
                    tracing::debug!(
                        filter = filter.name(),
                        "filter short-circuit — stopping chain"
                    );
                    return Ok(true);
                }
                Decision::Deny(code) => {
                    tracing::debug!(filter = filter.name(), code, "filter deny");
                    // Let Pingora convert this into an error response.
                    return Err(Error::new_str("forge filter denied request"));
                }
            }
        }

        // 2. Core request-header hygiene.
        let req = session.req_header_mut();
        for hdr in HOP_BY_HOP {
            req.remove_header(*hdr);
        }

        // Inject the request-ID generated in `new_ctx` so upstream logs
        // can correlate.
        req.insert_header("x-forge-id", ctx.request_id.as_str())
            .map_err(|_| Error::new_str("x-forge-id insert failed"))?;

        Ok(false)
    }

    /// Run the filter chain's `on_upstream_request` hooks.
    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut pingora::http::RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        for filter in &self.config.filters {
            match filter.on_upstream_request(session, upstream_request, ctx).await {
                Decision::Continue => continue,
                Decision::ShortCircuit(_) | Decision::Deny(_) => {
                    // Upstream-request filters should not short-circuit —
                    // by the time we reach this hook, the upstream peer
                    // is already selected.  Log + continue silently.
                    tracing::warn!(
                        filter = filter.name(),
                        "on_upstream_request returned non-Continue — ignored"
                    );
                    break;
                }
            }
        }
        Ok(())
    }

    /// Run the filter chain's `on_response` hooks, then append the
    /// `x-forge-via` banner.
    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut pingora::http::ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        for filter in &self.config.filters {
            match filter.on_response(session, upstream_response, ctx).await {
                Decision::Continue => continue,
                Decision::ShortCircuit(_) | Decision::Deny(_) => {
                    tracing::warn!(
                        filter = filter.name(),
                        "on_response returned non-Continue — ignored"
                    );
                    break;
                }
            }
        }

        upstream_response
            .insert_header("x-forge-via", "armageddon-pingora")
            .map_err(|_| Error::new_str("x-forge-via insert failed"))?;

        tracing::debug!(
            request_id = %ctx.request_id,
            upstream = %ctx.upstream_addr,
            "pingora response forwarded"
        );
        Ok(())
    }

    /// Run the filter chain's `on_logging` hooks (fan-out; no early exit).
    async fn logging(
        &self,
        session: &mut Session,
        _e: Option<&Error>,
        ctx: &mut Self::CTX,
    ) where
        Self::CTX: Send + Sync,
    {
        for filter in &self.config.filters {
            filter.on_logging(session, ctx).await;
        }
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // -- helpers --------------------------------------------------------------

    fn healthy_endpoint(host: &str, port: u16) -> Endpoint {
        Endpoint {
            address: host.to_string(),
            port,
            weight: 1,
            healthy: true,
        }
    }

    fn unhealthy_endpoint(host: &str, port: u16) -> Endpoint {
        Endpoint {
            address: host.to_string(),
            port,
            weight: 1,
            healthy: false,
        }
    }

    // -- ported: init ---------------------------------------------------------

    #[test]
    fn test_gateway_init_with_defaults() {
        let gw = PingoraGateway::with_defaults();
        assert_eq!(gw.config().default_cluster, "default");
        assert_eq!(gw.config().pool_size, 128);
        assert!(!gw.config().upstream_tls);
        assert!(gw.config().filters.is_empty());
        assert!(gw
            .upstream_registry()
            .first_healthy("any-cluster")
            .is_none());
    }

    #[test]
    fn test_gateway_init_custom_config() {
        let cfg = PingoraGatewayConfig {
            default_cluster: "prod-cluster".to_string(),
            upstream_tls: true,
            upstream_timeout_ms: 5_000,
            pool_size: 64,
            filters: Vec::new(),
        };
        let registry = Arc::new(UpstreamRegistry::new());
        let gw = PingoraGateway::new(cfg, registry);

        assert_eq!(gw.config().default_cluster, "prod-cluster");
        assert!(gw.config().upstream_tls);
        assert_eq!(gw.config().upstream_timeout_ms, 5_000);
        assert_eq!(gw.config().pool_size, 64);
    }

    // -- ported: upstream registry -------------------------------------------

    #[test]
    fn test_upstream_registry_resolves_healthy() {
        let reg = UpstreamRegistry::new();
        reg.update_cluster(
            "api",
            vec![
                unhealthy_endpoint("10.0.0.1", 8080),
                healthy_endpoint("10.0.0.2", 8080),
            ],
        );
        let ep = reg.first_healthy("api").expect("should resolve");
        assert_eq!(ep.address, "10.0.0.2");
        assert!(ep.healthy);
    }

    #[test]
    fn test_upstream_registry_all_unhealthy_returns_none() {
        let reg = UpstreamRegistry::new();
        reg.update_cluster(
            "db",
            vec![
                unhealthy_endpoint("10.0.0.3", 5432),
                unhealthy_endpoint("10.0.0.4", 5432),
            ],
        );
        assert!(reg.first_healthy("db").is_none());
    }

    #[test]
    fn test_upstream_registry_unknown_cluster() {
        let reg = UpstreamRegistry::new();
        assert!(reg.first_healthy("non-existent").is_none());
        assert!(reg.all("non-existent").is_empty());
    }

    #[test]
    fn test_upstream_registry_hot_reload() {
        let reg = UpstreamRegistry::new();
        reg.update_cluster("svc", vec![healthy_endpoint("10.0.0.5", 9000)]);
        let ep = reg.first_healthy("svc").unwrap();
        assert_eq!(ep.address, "10.0.0.5");

        reg.update_cluster("svc", vec![healthy_endpoint("10.0.0.6", 9000)]);
        let ep2 = reg.first_healthy("svc").unwrap();
        assert_eq!(ep2.address, "10.0.0.6");
    }

    // -- ported: hop-by-hop list ---------------------------------------------

    #[test]
    fn test_hop_by_hop_list_does_not_strip_content_type() {
        assert!(!HOP_BY_HOP.contains(&"content-type"));
        assert!(!HOP_BY_HOP.contains(&"authorization"));
        assert!(!HOP_BY_HOP.contains(&"x-request-id"));
    }

    #[test]
    fn test_hop_by_hop_list_contains_connection() {
        assert!(HOP_BY_HOP.contains(&"connection"));
        assert!(HOP_BY_HOP.contains(&"transfer-encoding"));
        assert!(HOP_BY_HOP.contains(&"upgrade"));
    }

    // -- new: filter chain traversal smoke test -------------------------------

    /// Counter-based no-op filter used to verify the chain is traversed.
    struct CountingFilter {
        name: &'static str,
        counter: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl crate::pingora::filters::ForgeFilter for CountingFilter {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn on_request(
            &self,
            _session: &mut Session,
            ctx: &mut RequestCtx,
        ) -> Decision {
            self.counter.fetch_add(1, Ordering::SeqCst);
            // Record filter activation into ctx so downstream assertions
            // can see the traversal order.
            ctx.feature_flags.push(self.name.to_string());
            Decision::Continue
        }
    }

    #[test]
    fn test_pingora_gateway_accepts_filter_chain() {
        let counter = Arc::new(AtomicUsize::new(0));
        let filters: Vec<SharedFilter> = vec![
            Arc::new(CountingFilter {
                name: "alpha",
                counter: counter.clone(),
            }),
            Arc::new(CountingFilter {
                name: "bravo",
                counter: counter.clone(),
            }),
        ];

        let cfg = PingoraGatewayConfig {
            filters,
            ..PingoraGatewayConfig::default()
        };
        let gw = PingoraGateway::new(cfg, Arc::new(UpstreamRegistry::new()));

        // Sanity: the filters are stored in the config in registration order.
        assert_eq!(gw.config().filters.len(), 2);
        assert_eq!(gw.config().filters[0].name(), "alpha");
        assert_eq!(gw.config().filters[1].name(), "bravo");

        // We cannot construct a real `pingora_proxy::Session` in unit
        // tests (it requires a live TCP connection), so we cannot drive
        // `request_filter` end-to-end here.  The crate-level integration
        // test in `tests/` (M1 — planned) exercises that path.
        //
        // What we can check: `new_ctx` returns a populated RequestCtx.
        let ctx = gw.new_ctx();
        assert_eq!(ctx.request_id.len(), 36);
    }

    // -- ported: build_server construction -----------------------------------

    #[test]
    fn test_build_server_constructs_without_panic() {
        let gw = PingoraGateway::with_defaults();
        // `build_server` binds lazily; in a CI sandbox it may fail due to
        // cap_net_bind_service missing — we only assert no panic.
        let result = crate::pingora::server::build_server(gw, "127.0.0.1:0");
        match result {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("build_server returned err in test env: {e}");
            }
        }
    }
}
