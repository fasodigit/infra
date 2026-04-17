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

pub mod circuit_breaker;
pub mod cors;
pub mod grpc_web;
pub mod health;
pub mod health_grpc;
pub mod health_tcp;
pub mod jwt;
pub mod kafka_producer;
pub mod proxy;
pub mod router;
pub mod tcp_proxy;
pub mod websocket;
pub mod webhooks;

pub use health::{EjectionPolicy, HealthCheckType, ProbeResult};

use armageddon_common::types::{Cluster, CorsConfig, JwtConfig, KratosConfig, Route};
use armageddon_config::gateway::{ExtAuthzConfig, ListenerConfig};
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
    /// Round-robin counters per cluster
    rr_counters: Arc<DashMap<String, RoundRobinCounter>>,
    clusters: Arc<Vec<Cluster>>,
}

use dashmap::DashMap;

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
        let rr_counters = Arc::new(DashMap::new());
        for cluster in &clusters {
            rr_counters.insert(cluster.name.clone(), RoundRobinCounter::new());
        }

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

    /// Start health check background tasks. Returns join handles.
    pub fn start_health_checks(&self) -> Vec<tokio::task::JoinHandle<()>> {
        self.health_manager.start()
    }

    /// Select a healthy upstream endpoint for a cluster using round-robin.
    pub fn select_upstream(
        &self,
        cluster_name: &str,
    ) -> Option<armageddon_common::types::Endpoint> {
        // Check circuit breaker
        if let Some(breaker) = self.circuit_breakers.get(cluster_name) {
            if !breaker.allow_request() {
                tracing::warn!("circuit breaker open for cluster '{}'", cluster_name);
                return None;
            }
        }

        // Get healthy endpoints
        let endpoints = self.health_manager.healthy_endpoints(cluster_name);
        if endpoints.is_empty() {
            return None;
        }

        // Round-robin selection
        let counter = self
            .rr_counters
            .entry(cluster_name.to_string())
            .or_insert_with(RoundRobinCounter::new);

        let idx = proxy::select_endpoint_round_robin(&endpoints, &counter)?;
        Some(endpoints[idx].clone())
    }

    /// Report that the listener addresses are ready.
    pub fn listener_info(&self) -> Vec<String> {
        self.listeners
            .iter()
            .map(|l| format!("{}:{} ({:?})", l.address, l.port, l.protocol))
            .collect()
    }
}
