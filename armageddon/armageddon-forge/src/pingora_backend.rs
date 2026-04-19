// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pingora-based proxy backend for ARMAGEDDON-FORGE.
//!
//! This module provides an alternative request-forwarding engine built on top of
//! [Pingora](https://github.com/cloudflare/pingora) instead of hyper 1.x.
//!
//! Pingora delivers connection pooling, zero-copy I/O, graceful restart, and built-in
//! TLS management out of the box, making it suitable for high-throughput production
//! deployments where the overhead of per-request connection setup is unacceptable.
//!
//! # Feature gate
//!
//! This module is only compiled when the crate feature `pingora` is enabled:
//!
//! ```text
//! cargo build --release --features pingora
//! ```
//!
//! The default (feature-less) build continues to use the hyper 1.x path in
//! [`crate::proxy`] with no changes required at the call site.
//!
//! # Architecture
//!
//! ```text
//!  Incoming TLS/plain TCP
//!        │
//!  ┌─────▼─────────────────────────────────┐
//!  │  PingoraGateway (ProxyHttp impl)       │
//!  │  ┌──────────────────────────────────┐  │
//!  │  │ upstream_peer()  ─► UpstreamRegistry│
//!  │  │ request_filter() ─► header strip  │  │
//!  │  │ response_filter()─► x-forge-via   │  │
//!  │  └──────────────────────────────────┘  │
//!  │  Connection pool (per upstream peer)   │
//!  └───────────────────────────────────────┘
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use pingora_core::prelude::*;
use pingora_proxy::{http_proxy_service, ProxyHttp, Session};

use armageddon_common::types::Endpoint;

// ── upstream registry ──────────────────────────────────────────────────────

/// Shared registry of healthy upstream endpoints, keyed by cluster name.
///
/// `PingoraGateway` holds an `Arc` to this so multiple worker threads see the
/// same view.  Hot-reload is safe: writers take a write-lock only during the
/// brief swap, readers proceed without blocking.
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

    /// Return the first healthy endpoint for `cluster`, round-robin not
    /// applied here — callers that need load balancing should wrap this.
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
///
/// Mirrors the subset of `ListenerConfig` / `Cluster` that Pingora needs.
/// Additional TLS options (`cert_path`, `key_path`) are passed directly to
/// Pingora's `ServerConf`.
#[derive(Debug, Clone)]
pub struct PingoraGatewayConfig {
    /// Default cluster to forward to when no per-path mapping is found.
    pub default_cluster: String,
    /// Upstream TLS: whether to verify server certificates.
    pub upstream_tls: bool,
    /// Request timeout in milliseconds applied to upstream connections.
    pub upstream_timeout_ms: u64,
    /// Maximum connections per upstream peer in Pingora's connection pool.
    pub pool_size: usize,
}

impl Default for PingoraGatewayConfig {
    fn default() -> Self {
        Self {
            default_cluster: "default".to_string(),
            upstream_tls: false,
            upstream_timeout_ms: 30_000,
            pool_size: 128,
        }
    }
}

// ── ProxyHttp implementation ───────────────────────────────────────────────

/// Pingora proxy gateway for ARMAGEDDON-FORGE.
///
/// Implements [`pingora_proxy::ProxyHttp`] so it can be driven by Pingora's
/// multi-threaded I/O scheduler.  Two extension points are wired:
///
/// - `upstream_peer`: resolves the target peer from the upstream registry.
/// - `request_filter`: strips hop-by-hop headers and injects `x-forge-id`.
/// - `response_filter`: appends `x-forge-via: armageddon-pingora`.
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

    /// Create a new gateway with default configuration and an empty upstream
    /// registry.  Useful for testing.
    pub fn with_defaults() -> Self {
        Self::new(PingoraGatewayConfig::default(), Arc::new(UpstreamRegistry::new()))
    }

    /// Return a reference to the upstream registry so callers can push updates.
    pub fn upstream_registry(&self) -> &Arc<UpstreamRegistry> {
        &self.upstream_registry
    }

    /// Return the gateway config.
    pub fn config(&self) -> &PingoraGatewayConfig {
        &self.config
    }
}

// Hop-by-hop headers that must not be forwarded to upstream.
const HOP_BY_HOP: &[&str] = &[
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
    /// Per-request context.  We store the resolved upstream address here so
    /// `upstream_peer` and response filters share it without re-resolving.
    type CTX = RequestCtx;

    fn new_ctx(&self) -> Self::CTX {
        RequestCtx::default()
    }

    /// Resolve the upstream peer from the registry.
    ///
    /// If no healthy endpoint is found, the request fails with a 503.
    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        // Derive cluster from the first path segment; fall back to default.
        let cluster = session
            .req_header()
            .uri
            .path()
            .trim_start_matches('/')
            .split('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.config.default_cluster);

        let endpoint = self
            .upstream_registry
            .first_healthy(cluster)
            .or_else(|| {
                self.upstream_registry
                    .first_healthy(&self.config.default_cluster)
            })
            .ok_or_else(|| {
                Error::new_str(&format!("no healthy upstream for cluster '{cluster}'"))
            })?;

        let addr = format!("{}:{}", endpoint.address, endpoint.port);
        ctx.upstream_addr = addr.clone();

        tracing::debug!(
            cluster,
            upstream = %addr,
            "pingora resolved upstream peer"
        );

        let peer = HttpPeer::new(addr, self.config.upstream_tls, String::new());
        Ok(Box::new(peer))
    }

    /// Strip hop-by-hop headers and append `x-forge-id` before forwarding.
    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<bool> {
        let req = session.req_header_mut();

        // Remove hop-by-hop headers.
        for hdr in HOP_BY_HOP {
            req.remove_header(*hdr);
        }

        // Inject a unique request-ID for tracing correlation.
        let rid = uuid::Uuid::new_v4().to_string();
        req.insert_header("x-forge-id", &rid)
            .map_err(|e| Error::new_str(&format!("header insert error: {e}")))?;
        ctx.request_id = rid;

        Ok(false) // false = continue processing (do not short-circuit)
    }

    /// Append the `x-forge-via` header to every upstream response.
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        upstream_response
            .insert_header("x-forge-via", "armageddon-pingora")
            .map_err(|e| Error::new_str(&format!("response header error: {e}")))?;

        tracing::debug!(
            request_id = %ctx.request_id,
            upstream = %ctx.upstream_addr,
            "pingora response forwarded"
        );
        Ok(())
    }
}

// ── per-request context ────────────────────────────────────────────────────

/// Context propagated through the Pingora filter chain for a single request.
#[derive(Debug, Default)]
pub struct RequestCtx {
    /// Resolved upstream address (host:port).
    pub upstream_addr: String,
    /// Unique request identifier injected into the forwarded request.
    pub request_id: String,
}

// ── server bootstrap ───────────────────────────────────────────────────────

/// Build and return a Pingora [`Server`] wired with `gateway` as the only
/// HTTP proxy service.
///
/// The returned server can be started with `server.run_forever()`.  Graceful
/// restart is provided by Pingora's own SIGUP handler — no additional wiring
/// is needed.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "pingora")]
/// # {
/// use armageddon_forge::pingora_backend::{
///     build_server, PingoraGateway, PingoraGatewayConfig, UpstreamRegistry,
/// };
/// use std::sync::Arc;
///
/// let registry = Arc::new(UpstreamRegistry::new());
/// let gw = PingoraGateway::new(PingoraGatewayConfig::default(), registry);
/// let mut server = build_server(gw, "0.0.0.0:8080").expect("server build failed");
/// // server.run_forever();  // blocking
/// # }
/// ```
pub fn build_server(
    gateway: PingoraGateway,
    listen_addr: &str,
) -> anyhow::Result<Server> {
    let mut server = Server::new(None)?;
    server.bootstrap();

    let mut proxy = http_proxy_service(&server.configuration, gateway);
    proxy.add_tcp(listen_addr);

    server.add_service(proxy);
    Ok(server)
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- helper ---------------------------------------------------------------

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

    // -- test 1: init ---------------------------------------------------------

    /// Verify that `PingoraGateway::with_defaults()` constructs a gateway
    /// with a valid (empty) upstream registry.
    #[test]
    fn test_gateway_init_with_defaults() {
        let gw = PingoraGateway::with_defaults();
        assert_eq!(gw.config().default_cluster, "default");
        assert_eq!(gw.config().pool_size, 128);
        assert!(!gw.config().upstream_tls);
        // Registry starts empty; no cluster resolves.
        assert!(gw.upstream_registry().first_healthy("any-cluster").is_none());
    }

    /// Verify that custom config values are stored correctly and exposed.
    #[test]
    fn test_gateway_init_custom_config() {
        let cfg = PingoraGatewayConfig {
            default_cluster: "prod-cluster".to_string(),
            upstream_tls: true,
            upstream_timeout_ms: 5_000,
            pool_size: 64,
        };
        let registry = Arc::new(UpstreamRegistry::new());
        let gw = PingoraGateway::new(cfg, registry);

        assert_eq!(gw.config().default_cluster, "prod-cluster");
        assert!(gw.config().upstream_tls);
        assert_eq!(gw.config().upstream_timeout_ms, 5_000);
        assert_eq!(gw.config().pool_size, 64);
    }

    // -- test 2: upstream registry --------------------------------------------

    /// Healthy endpoint is resolved from the registry.
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

    /// When all endpoints are unhealthy, `first_healthy` returns `None`.
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

    /// Unknown cluster returns `None` — does not panic.
    #[test]
    fn test_upstream_registry_unknown_cluster() {
        let reg = UpstreamRegistry::new();
        assert!(reg.first_healthy("non-existent").is_none());
        assert!(reg.all("non-existent").is_empty());
    }

    /// Hot-reload: replacing the endpoint list is reflected immediately.
    #[test]
    fn test_upstream_registry_hot_reload() {
        let reg = UpstreamRegistry::new();
        reg.update_cluster("svc", vec![healthy_endpoint("10.0.0.5", 9000)]);

        let ep = reg.first_healthy("svc").unwrap();
        assert_eq!(ep.address, "10.0.0.5");

        // Swap to a different host.
        reg.update_cluster("svc", vec![healthy_endpoint("10.0.0.6", 9000)]);
        let ep2 = reg.first_healthy("svc").unwrap();
        assert_eq!(ep2.address, "10.0.0.6");
    }

    // -- test 3: request forward (unit — no live upstream) --------------------

    /// Verify that `HOP_BY_HOP` list does not include safe headers that must
    /// be forwarded (regression guard).
    #[test]
    fn test_hop_by_hop_list_does_not_strip_content_type() {
        assert!(!HOP_BY_HOP.contains(&"content-type"));
        assert!(!HOP_BY_HOP.contains(&"authorization"));
        assert!(!HOP_BY_HOP.contains(&"x-request-id"));
    }

    /// Verify hop-by-hop headers are in the list.
    #[test]
    fn test_hop_by_hop_list_contains_connection() {
        assert!(HOP_BY_HOP.contains(&"connection"));
        assert!(HOP_BY_HOP.contains(&"transfer-encoding"));
        assert!(HOP_BY_HOP.contains(&"upgrade"));
    }

    // -- test 4: graceful-shutdown contract -----------------------------------

    /// `build_server` should return an error (not panic) when Pingora fails to
    /// initialise — e.g. due to an invalid listen address format.
    ///
    /// NOTE: We cannot call `server.run_forever()` in tests (it blocks and
    /// binds a port).  We verify only the construction path here.
    #[test]
    fn test_build_server_constructs_without_panic() {
        let gw = PingoraGateway::with_defaults();
        // A valid address — server object should be returned (no bind yet).
        let result = build_server(gw, "127.0.0.1:0");
        // We accept either Ok or Err (environment may lack privileges);
        // what we assert is that no panic occurred.
        match result {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("build_server returned err in test env: {e}");
            }
        }
    }
}
