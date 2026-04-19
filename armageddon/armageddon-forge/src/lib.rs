// SPDX-License-Identifier: AGPL-3.0-or-later
//! armageddon-forge: Core HTTP/gRPC proxy engine replacing Envoy.
//!
//! FORGE is the heart of the gateway. Built on hyper 1.x + tokio, it handles:
//! - HTTP/1.1, HTTP/2, gRPC proxying
//! - JWT ES384 validation (JWKS from auth-ms, cached 300s)
//! - CORS per-platform origin configuration
//! - GraphQL routing (/api/graphql -> DGS Gateway)
//! - gRPC routing (content-type: application/grpc)
//! - Health checks (HTTP periodic per upstream)
//! - Circuit breakers per cluster upstream
//! - Round-robin load balancing across healthy endpoints

// -- optional feature-gated backends --

/// Pingora-based proxy backend (compiled only with `--features pingora`).
#[cfg(feature = "pingora")]
pub mod pingora_backend;

/// io_uring-based backend (Linux only, compiled with `--features io_uring`).
#[cfg(all(target_os = "linux", feature = "io_uring"))]
pub mod io_uring_backend;

// -- core modules --

pub mod circuit_breaker;
pub mod compression;
pub mod cors;
pub mod feature_flags;
pub mod grpc_web;
pub mod health;
pub mod health_grpc;
pub mod health_tcp;
pub mod jwt;
pub mod kafka_producer;
pub mod otel_middleware;
pub mod proxy;
pub mod router;
pub mod splice;
pub mod tcp_proxy;
pub mod traffic_split;
pub mod upstream_pool;
pub mod websocket;
pub mod webhooks;

pub use health::{EjectionPolicy, HealthCheckType, ProbeResult};

// RateLimitFilter is now wired into the pipeline.
// See `rate_limit_filter()` accessor and `build_rate_limit_filter()` helper.

use armageddon_common::types::{Cluster, CorsConfig, JwtConfig, KratosConfig, RateLimitConfig, RateLimitFallback, RateLimitMode, Route};
use armageddon_config::gateway::{ExtAuthzConfig, ListenerConfig};
use armageddon_ratelimit::filter::{FallbackPolicy, RateLimitFilter};
use armageddon_ratelimit::global::{GlobalRateLimiter, GlobalRuleConfig, MockRateLimitBackend};
use armageddon_ratelimit::LocalTokenBucket;
use dashmap::DashMap;
use proxy::RoundRobinCounter;
use std::sync::Arc;

/// The FORGE proxy server.
pub struct ForgeServer {
    listeners: Vec<ListenerConfig>,
    router: Arc<router::Router>,
    jwt_validator: Arc<jwt::JwtValidator>,
    kratos_validator: Arc<jwt::KratosSessionValidator>,
    cors_handler: Arc<cors::CorsHandler>,
    health_manager: Arc<health::HealthManager>,
    circuit_breakers: Arc<circuit_breaker::CircuitBreakerManager>,
    /// Round-robin counters per cluster.
    rr_counters: Arc<DashMap<String, RoundRobinCounter>>,
    clusters: Arc<Vec<Cluster>>,
    /// Persistent H2 connection pool — shared across all request handlers.
    pub upstream_pool: Arc<upstream_pool::UpstreamPool>,
    /// Rate limit filter — `None` when rate limiting is disabled in config.
    rate_limit_filter: Option<Arc<RateLimitFilter>>,
}

impl ForgeServer {
    /// Create a new FORGE proxy server.
    pub fn new(
        listeners: Vec<ListenerConfig>,
        routes: Vec<Route>,
        clusters: Vec<Cluster>,
        jwt_config: JwtConfig,
        kratos_config: KratosConfig,
        cors_configs: Vec<(String, CorsConfig)>,
        _ext_authz_config: ExtAuthzConfig,
    ) -> Self {
        Self::new_with_rate_limit(
            listeners,
            routes,
            clusters,
            jwt_config,
            kratos_config,
            cors_configs,
            _ext_authz_config,
            None,
            &prometheus::Registry::new(),
        )
    }

    /// Create a new FORGE proxy server with optional rate limiting.
    ///
    /// Pass `rate_limit_cfg = Some(cfg)` to enable rate limiting from the
    /// gateway config.  Pass `None` to skip it entirely (zero overhead).
    ///
    /// The `registry` is the Prometheus registry used for RL metrics.
    pub fn new_with_rate_limit(
        listeners: Vec<ListenerConfig>,
        routes: Vec<Route>,
        clusters: Vec<Cluster>,
        jwt_config: JwtConfig,
        kratos_config: KratosConfig,
        cors_configs: Vec<(String, CorsConfig)>,
        _ext_authz_config: ExtAuthzConfig,
        rate_limit_cfg: Option<&RateLimitConfig>,
        registry: &prometheus::Registry,
    ) -> Self {
        let rr_counters = Arc::new(DashMap::new());
        for cluster in &clusters {
            rr_counters.insert(cluster.name.clone(), RoundRobinCounter::new());
        }

        let rate_limit_filter = rate_limit_cfg
            .filter(|cfg| cfg.enabled)
            .and_then(|cfg| {
                match build_rate_limit_filter(cfg, registry) {
                    Ok(f) => Some(Arc::new(f)),
                    Err(e) => {
                        tracing::warn!(err = %e, "rate limit metrics registration failed — filter disabled");
                        None
                    }
                }
            });

        Self {
            listeners,
            router: Arc::new(router::Router::new(routes)),
            jwt_validator: Arc::new(jwt::JwtValidator::new(jwt_config)),
            kratos_validator: Arc::new(jwt::KratosSessionValidator::new(kratos_config)),
            cors_handler: Arc::new(cors::CorsHandler::new(cors_configs)),
            health_manager: Arc::new(health::HealthManager::new(clusters.clone())),
            circuit_breakers: Arc::new(circuit_breaker::CircuitBreakerManager::new(
                clusters.clone(),
            )),
            rr_counters,
            clusters: Arc::new(clusters),
            upstream_pool: Arc::new(upstream_pool::UpstreamPool::new(
                upstream_pool::PoolConfig::default(),
            )),
            rate_limit_filter,
        }
    }

    /// Get a reference to the router.
    pub fn router(&self) -> &Arc<router::Router> {
        &self.router
    }

    /// Get a reference to the JWT validator.
    pub fn jwt_validator(&self) -> &Arc<jwt::JwtValidator> {
        &self.jwt_validator
    }

    /// Get a reference to the Kratos session validator.
    pub fn kratos_validator(&self) -> &Arc<jwt::KratosSessionValidator> {
        &self.kratos_validator
    }

    /// Get a reference to the CORS handler.
    pub fn cors_handler(&self) -> &Arc<cors::CorsHandler> {
        &self.cors_handler
    }

    /// Get a reference to the health manager.
    pub fn health_manager(&self) -> &Arc<health::HealthManager> {
        &self.health_manager
    }

    /// Get a reference to the circuit breaker manager.
    pub fn circuit_breakers(&self) -> &Arc<circuit_breaker::CircuitBreakerManager> {
        &self.circuit_breakers
    }

    /// Get a reference to the clusters.
    pub fn clusters(&self) -> &Arc<Vec<Cluster>> {
        &self.clusters
    }

    /// Get a reference to the round-robin counters.
    pub fn rr_counters(&self) -> &Arc<DashMap<String, RoundRobinCounter>> {
        &self.rr_counters
    }

    /// Find a cluster by name.
    pub fn find_cluster(&self, name: &str) -> Option<&Cluster> {
        self.clusters.iter().find(|c| c.name == name)
    }

    /// Return the rate limit filter, or `None` if disabled.
    pub fn rate_limit_filter(&self) -> Option<&Arc<RateLimitFilter>> {
        self.rate_limit_filter.as_ref()
    }

    /// Start health check background tasks. Returns join handles.
    pub fn start_health_checks(&self) -> Vec<tokio::task::JoinHandle<()>> {
        self.health_manager.start()
    }

    /// Select a healthy upstream endpoint for a cluster using round-robin.
    pub fn select_upstream(
        &self,
        cluster_name: &str,
    ) -> Option<armageddon_common::types::Endpoint> {
        // Check circuit breaker.
        if let Some(breaker) = self.circuit_breakers.get(cluster_name) {
            if !breaker.allow_request() {
                tracing::warn!("circuit breaker open for cluster '{}'", cluster_name);
                return None;
            }
        }

        // Get healthy endpoints.
        let endpoints = self.health_manager.healthy_endpoints(cluster_name);
        if endpoints.is_empty() {
            return None;
        }

        // Round-robin selection.
        let counter = self
            .rr_counters
            .entry(cluster_name.to_string())
            .or_insert_with(RoundRobinCounter::new);

        let idx = proxy::select_endpoint_round_robin(&endpoints, &counter)?;
        Some(endpoints[idx].clone())
    }

    /// Forward a request to a healthy upstream using the persistent H2 pool.
    pub async fn forward_via_pool(
        &self,
        endpoint: &armageddon_common::types::Endpoint,
        method: &str,
        path: &str,
        headers: &[(String, String)],
        body: Option<bytes::Bytes>,
        timeout_ms: u64,
    ) -> armageddon_common::error::Result<proxy::ProxyResponse> {
        use std::net::SocketAddr;

        let addr: SocketAddr = format!("{}:{}", endpoint.address, endpoint.port)
            .parse()
            .map_err(|e| {
                armageddon_common::error::ArmageddonError::Internal(format!(
                    "invalid upstream address: {}",
                    e
                ))
            })?;

        let conn = self.upstream_pool.get_or_create(addr).await.map_err(|e| {
            armageddon_common::error::ArmageddonError::UpstreamConnection(e.to_string())
        })?;

        let upstream_uri = format!("http://{}:{}{}", endpoint.address, endpoint.port, path);
        let mut builder = hyper::Request::builder()
            .method(method)
            .uri(&upstream_uri);

        for (name, value) in headers {
            let lower = name.to_lowercase();
            if matches!(
                lower.as_str(),
                "connection"
                    | "keep-alive"
                    | "transfer-encoding"
                    | "te"
                    | "trailer"
                    | "upgrade"
                    | "proxy-authorization"
                    | "proxy-authenticate"
            ) {
                continue;
            }
            if lower == "host" {
                continue;
            }
            builder = builder.header(name.as_str(), value.as_str());
        }
        builder = builder.header(
            "host",
            format!("{}:{}", endpoint.address, endpoint.port),
        );

        let req_body = body.unwrap_or_default();
        let request = builder
            .body(http_body_util::Full::new(req_body))
            .map_err(|e| {
                armageddon_common::error::ArmageddonError::Internal(format!(
                    "failed to build request: {}",
                    e
                ))
            })?;

        let (parts, body_bytes): (http::response::Parts, bytes::Bytes) =
            tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                conn.send(request),
            )
            .await
            .map_err(|_| {
                armageddon_common::error::ArmageddonError::UpstreamTimeout(timeout_ms)
            })?
            .map_err(|e: upstream_pool::PoolError| {
                armageddon_common::error::ArmageddonError::UpstreamConnection(e.to_string())
            })?;

        let resp_headers: Vec<(String, String)> = parts
            .headers
            .iter()
            .map(|(k, v): (&http::header::HeaderName, &http::header::HeaderValue)| {
                (k.as_str().to_string(), v.to_str().unwrap_or("").to_string())
            })
            .collect();

        Ok(proxy::ProxyResponse {
            status: parts.status.as_u16(),
            headers: resp_headers,
            body: body_bytes,
        })
    }

    /// Report that the listener addresses are ready.
    pub fn listener_info(&self) -> Vec<String> {
        self.listeners
            .iter()
            .map(|l| format!("{}:{} ({:?})", l.address, l.port, l.protocol))
            .collect()
    }
}

// ── Rate limit filter builder ──────────────────────────────────────────────────

/// Construct a `RateLimitFilter` from a `RateLimitConfig`.
///
/// Uses `MockRateLimitBackend` for global/hybrid modes (production should swap
/// to `KayaRateLimitBackend` once a KAYA connection is available at build time).
fn build_rate_limit_filter(
    cfg: &RateLimitConfig,
    registry: &prometheus::Registry,
) -> Result<RateLimitFilter, prometheus::Error> {
    let fallback = match cfg.fallback {
        RateLimitFallback::FailOpen => FallbackPolicy::FailOpen,
        RateLimitFallback::FailClosed => FallbackPolicy::FailClosed,
    };

    let bucket = Arc::new(LocalTokenBucket::new());
    for rule in &cfg.rules {
        let burst = rule.burst.unwrap_or(rule.requests_per_window);
        bucket.add_rule(&rule.descriptor, rule.requests_per_window, burst);
    }

    match cfg.mode {
        RateLimitMode::Local => {
            RateLimitFilter::new_local(bucket, registry)
        }
        RateLimitMode::Global => {
            let backend = Arc::new(MockRateLimitBackend::new()) as Arc<dyn armageddon_ratelimit::RateLimitBackend>;
            let mut limiter = GlobalRateLimiter::new(backend);
            for rule in &cfg.rules {
                limiter.add_rule(&rule.descriptor, GlobalRuleConfig {
                    requests_per_window: rule.requests_per_window,
                    window_secs: rule.window_secs,
                });
            }
            RateLimitFilter::new_global(Arc::new(limiter), fallback, registry)
        }
        RateLimitMode::Hybrid => {
            let backend = Arc::new(MockRateLimitBackend::new()) as Arc<dyn armageddon_ratelimit::RateLimitBackend>;
            let mut limiter = GlobalRateLimiter::new(backend);
            for rule in &cfg.rules {
                limiter.add_rule(&rule.descriptor, GlobalRuleConfig {
                    requests_per_window: rule.requests_per_window,
                    window_secs: rule.window_secs,
                });
            }
            RateLimitFilter::new_hybrid(bucket, Arc::new(limiter), fallback, cfg.shadow, registry)
        }
    }
}
