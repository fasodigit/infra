// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// ForgeServer is deprecated since v2.0.  This binary still boots it for
// runtime=hyper and runtime=shadow.  Remove after v3.0 cleanup.
#![allow(deprecated)]
//
//! ARMAGEDDON: Sovereign security gateway for the FASO DIGITALISATION project.
//!
//! Orchestration of all Vague 1 components:
//!
//! ```text
//!  ┌──────────────────────────────────────────────────────┐
//!  │                   ARMAGEDDON GATEWAY                 │
//!  │                                                      │
//!  │  HTTP/1+2 ─┐          ┌─ Pentagon security pipeline  │
//!  │            ├─ FORGE ──┤  (SENTINEL/ARBITER/ORACLE/   │
//!  │  HTTP/3 ───┘          │   AEGIS/AI/WASM)             │
//!  │  (QUIC)               │                              │
//!  │                       ├─ LB (7 algorithms)           │
//!  │                       ├─ Retry + Budget              │
//!  │                       ├─ Response Cache (KAYA)       │
//!  │                       └─ VEIL (header masking)       │
//!  │                                                      │
//!  │  MESH (SPIRE mTLS) ──── outbound mTLS connections    │
//!  │  xDS consumer ─────────  hot-reload clusters/routes  │
//!  │  Admin API (loopback) ── /admin/* management         │
//!  └──────────────────────────────────────────────────────┘
//! ```
//!
//! All optional components (QUIC, Mesh, xDS consumer, Cache, Admin) are
//! gracefully skipped when absent from the config file.
//!
//! Usage:
//!   armageddon --config config/armageddon.yaml

mod admin_providers;
mod pipeline;
#[cfg(feature = "numa")]
mod numa;

use anyhow::Context;
use armageddon_admin::{AdminConfig as AdminServerConfig, AdminServer, AdminState};
use armageddon_admin_api::AdminApi;
use armageddon_cache::{CachePolicy, InMemoryKv, ResponseCache};
use armageddon_common::context::RequestContext;
use armageddon_common::decision::Action;
use armageddon_common::types::{AuthMode, ConnectionInfo, HttpRequest, HttpResponse, HttpVersion, Protocol};
use armageddon_aegis::graphql_limits::{extract_gql_query, GqlLimitError, GraphQLLimiter};
use armageddon_config::gateway::{GatewayRuntime, ShadowSinkType};
use armageddon_forge::cors::CorsHandler;
use armageddon_forge::jwt::JwtValidator;
use armageddon_forge::kafka_producer::RedpandaProducer;
use armageddon_forge::pingora::{PingoraGateway, PingoraGatewayConfig, UpstreamRegistry};
use armageddon_forge::pingora::filters::router::{RouteTable, RouterFilter};
use armageddon_forge::pingora::filters::SharedFilter;
use armageddon_forge::pingora::server::build_server as pingora_build_server;
use armageddon_forge::pingora::shadow::ShadowSampler;
use armageddon_forge::pingora::shadow_sink::{
    MultiSink, NoopSink, RedpandaSink, ShadowDiffDispatcher, ShadowDiffSink, SinkMetrics,
    SqliteSink,
};
use armageddon_forge::router::Router;
use armageddon_forge::webhooks::GithubWebhookHandler;
use armageddon_lb::{
    LoadBalancer, LeastConnections, Maglev, PowerOfTwoChoices, Random, RingHash, RoundRobin,
    WeightedRoundRobin, Endpoint,
};
use armageddon_mesh::Mesh;
use armageddon_quic::{Http3Server, QuicListenerConfig, RequestHandler as QuicRequestHandler};
use armageddon_retry::{execute_with_retry, RetryBudget, RetryPolicy, RetryableRequest};
use armageddon_xds::{AdsClient, XdsCallback};
use armageddon_xds::proto::{
    cluster::Cluster as XdsCluster,
    endpoint::ClusterLoadAssignment as XdsEndpointAssignment,
    listener::Listener as XdsListener,
    route::RouteConfiguration as XdsRouteConfig,
    tls::Secret as XdsSecret,
};
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use prometheus::Registry;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// CLI arguments.
struct Args {
    config_path: String,
}

impl Args {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let config_path = if args.len() > 2 && args[1] == "--config" {
            args[2].clone()
        } else {
            "config/armageddon.yaml".to_string()
        };
        Self { config_path }
    }
}

// ---------------------------------------------------------------------------
// Shared gateway state
// ---------------------------------------------------------------------------

/// Shared state passed to each request handler.
struct GatewayState {
    pipeline: Arc<pipeline::Pentagon>,
    forge: Arc<armageddon_forge::ForgeServer>,
    veil: armageddon_veil::Veil,
    auth_mode: AuthMode,
    /// GitHub webhook handler (bypasses Pentagon pipeline).
    github_webhook: Option<Arc<GithubWebhookHandler>>,
    /// Load balancer (selected via config).
    lb: Arc<dyn LoadBalancer>,
    /// LB endpoint pool (derived from cluster config at startup).
    lb_endpoints: Vec<Arc<Endpoint>>,
    /// Retry policy.
    retry_policy: RetryPolicy,
    /// Retry budget (shared across all requests).
    retry_budget: Arc<RetryBudget>,
    /// Response cache backed by KAYA (or in-memory fallback).
    cache: Option<Arc<ResponseCache>>,
    /// mTLS mesh (present when SPIRE is configured and reachable).
    mesh: Option<Arc<Mesh>>,
    /// GraphQL depth/complexity/introspection limiter.
    /// `None` when disabled in config.
    gql_limiter: Option<Arc<GraphQLLimiter>>,
    /// Counter for requests denied before reaching upstream (labeled by reason).
    requests_denied: Arc<prometheus::IntCounterVec>,
    /// Shadow mode sampler.  Present only when `runtime = shadow` and shadow
    /// mode is enabled in config.  Used by the hyper request path to decide
    /// whether to mirror a request to the Pingora shadow backend and to
    /// dispatch diff events via the wired `DiffEventSender`.
    shadow_sampler: Option<Arc<ShadowSampler>>,
}

// ---------------------------------------------------------------------------
// HTTP/3 bridge: GatewayState implements QuicRequestHandler
// ---------------------------------------------------------------------------

/// Thin wrapper that bridges `QuicRequestHandler` to `GatewayState`.
///
/// HTTP/3 requests arrive as `HttpRequest` directly (no TCP peer addr at the
/// trait boundary); we synthesise a dummy peer address.
struct Http3Bridge {
    state: Arc<GatewayState>,
}

#[async_trait]
impl QuicRequestHandler for Http3Bridge {
    async fn handle(
        &self,
        req: HttpRequest,
    ) -> Result<HttpResponse, armageddon_quic::QuicError> {
        // HTTP/3 requests are forwarded through the same pipeline as HTTP/1.
        // We synthesise a peer address (QUIC layer does not surface it here).
        let dummy_peer: SocketAddr = "0.0.0.0:0".parse().expect("valid addr");
        let resp = handle_http_request(req, dummy_peer, Arc::clone(&self.state)).await;
        Ok(resp)
    }
}

// ---------------------------------------------------------------------------
// xDS callback — forwards cluster/endpoint updates into the LB pool
// ---------------------------------------------------------------------------

/// Minimal xDS callback: logs every resource update.
///
/// In production, wire this to a shared cluster registry so that new
/// endpoints from xDS hot-reload the LB ring without a restart.
struct LoggingXdsCallback;

#[async_trait]
impl XdsCallback for LoggingXdsCallback {
    async fn on_cluster_update(&self, c: XdsCluster) {
        tracing::info!(cluster = %c.name, "xDS cluster update");
    }
    async fn on_endpoint_update(&self, e: XdsEndpointAssignment) {
        tracing::info!(cluster = %e.cluster_name, "xDS endpoint update");
    }
    async fn on_listener_update(&self, l: XdsListener) {
        tracing::info!(listener = %l.name, "xDS listener update");
    }
    async fn on_route_update(&self, r: XdsRouteConfig) {
        tracing::info!(route = %r.name, "xDS route update");
    }
    async fn on_secret_update(&self, s: XdsSecret) {
        tracing::info!(secret = %s.name, "xDS secret update");
    }
}

// ---------------------------------------------------------------------------
// Runtime bootstrap
// ---------------------------------------------------------------------------

/// Entry point.
///
/// When the `numa` feature is enabled **and** the machine is multi-socket, a
/// NUMA-pinned Tokio runtime is built (one worker per NUMA node, each
/// thread bound to its node's CPU set via `sched_setaffinity`).
///
/// On single-NUMA machines, non-Linux platforms, or when the feature is off,
/// the standard `multi_thread` runtime is used transparently.
fn main() -> anyhow::Result<()> {
    #[cfg(feature = "numa")]
    let runtime = {
        let nodes: Vec<usize> = numa::detect_topology()
            .map(|t| t.nodes.iter().map(|n| n.id).collect())
            .unwrap_or_default();
        numa::spawn_numa_pinned_runtime(nodes)
    };

    #[cfg(not(feature = "numa"))]
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime must build");

    runtime.block_on(async_main())
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async fn async_main() -> anyhow::Result<()> {
    // -- 1. Parse CLI args
    let args = Args::parse();

    // -- 2. Load config first (so we know whether to wire the OTel bridge).
    //    We can't initialize tracing before this because the bridge layer
    //    must be registered with the subscriber alongside the fmt layer.
    let config_loader = armageddon_config::ConfigLoader::from_file(&args.config_path)
        .context("failed to load configuration")?;
    let config = config_loader.get();

    // -- 3. Initialize tracing — wire the tracing-opentelemetry bridge layer
    //    when traces_endpoint is set so that #[tracing::instrument] spans on
    //    the Pingora hot-path actually reach Jaeger/Tempo.
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().json();

    if let Some(endpoint) = config.observability.traces_endpoint.as_deref() {
        let otel_cfg = armageddon_config::OtelConfig {
            endpoint: endpoint.to_string(),
            service_name: "armageddon".to_string(),
            sampling_rate: 1.0,
            resource_attributes: Default::default(),
        };
        match armageddon_oracle::otel_propagation::build_tracer_provider(&otel_cfg) {
            Ok(provider) => {
                use opentelemetry::trace::TracerProvider as _;
                let tracer = provider.tracer("armageddon");
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .with(otel_layer)
                    .init();
                tracing::info!(
                    endpoint = %endpoint,
                    "OTel tracer + tracing bridge registered — armageddon spans will export"
                );
            }
            Err(e) => {
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .init();
                tracing::warn!(
                    endpoint = %endpoint, err = %e,
                    "OTel tracer init failed — continuing without span export"
                );
            }
        }
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }

    tracing::info!(
        "ARMAGEDDON v{} starting (Pentagon + Vague-1 components)",
        env!("CARGO_PKG_VERSION")
    );
    tracing::info!(path = %args.config_path, "configuration loaded");

    // -- 4. Prometheus registry
    let registry = Registry::new();

    // -- 5. Build FORGE
    let cors_configs: Vec<(String, armageddon_common::types::CorsConfig)> = config
        .gateway
        .cors
        .iter()
        .map(|e| (e.platform.clone(), e.config.clone()))
        .collect();

    let forge = Arc::new(armageddon_forge::ForgeServer::new_with_rate_limit(
        config.gateway.listeners.clone(),
        config.gateway.routes.clone(),
        config.gateway.clusters.clone(),
        config.gateway.jwt.clone(),
        config.gateway.kratos.clone(),
        cors_configs,
        config.gateway.ext_authz.clone(),
        config.gateway.rate_limit.as_ref(),
        &registry,
    ));

    // Counter for requests denied by the rate limiter.
    // Label: reason="rate_limit"
    let rl_denied_counter = prometheus::register_int_counter_vec_with_registry!(
        prometheus::opts!(
            "armageddon_forge_requests_denied_total",
            "Total requests denied before reaching upstream"
        ),
        &["reason"],
        registry
    )
    .unwrap_or_else(|_| {
        // If already registered (e.g. in tests), use the default registry fallback.
        prometheus::IntCounterVec::new(
            prometheus::opts!(
                "armageddon_forge_requests_denied_total_fallback",
                "Total requests denied (fallback)"
            ),
            &["reason"],
        )
        .expect("counter must build")
    });
    let rl_denied_counter = Arc::new(rl_denied_counter);

    // -- 6. Build VEIL
    let veil = armageddon_veil::Veil::new(config.security.veil.clone());

    // -- 7. Initialize Pentagon pipeline
    let pentagon = {
        let mut p = pipeline::Pentagon::new(&config)?;
        p.init().await?;
        Arc::new(p)
    };
    tracing::info!("Pentagon pipeline initialized — all 5 engines ready");

    // -- 8. Start ForgeServer health checks
    let _health_handles = forge.start_health_checks();

    // -- 9. GitHub webhook handler (optional)
    let github_webhook = build_github_webhook_handler(&config).await;

    // -- 10. mTLS Mesh (optional)
    let mesh: Option<Arc<Mesh>> = if let Some(mesh_cfg) = config.gateway.mesh.clone() {
        tracing::info!(socket = %mesh_cfg.socket_path, "initialising SPIRE mTLS mesh");
        let ca_bundle = mesh_cfg
            .ca_bundle_pem
            .as_deref()
            .unwrap_or("")
            .as_bytes()
            .to_vec();
        match Mesh::new(
            Path::new(&mesh_cfg.socket_path),
            ca_bundle,
            mesh_cfg.peer_id.clone(),
        )
        .await
        {
            Ok(m) => {
                tracing::info!(peer = %mesh_cfg.peer_id, "mTLS mesh initialised");
                Some(m)
            }
            Err(e) => {
                tracing::warn!(err = %e, "mTLS mesh init failed — running without SPIRE (fallback to plain TLS)");
                None
            }
        }
    } else {
        tracing::info!("mTLS mesh not configured — skipping");
        None
    };

    // -- 11. xDS ADS consumer (optional)
    let xds_client: Option<AdsClient> = if let Some(xds_cfg) = config.gateway.xds_consumer.clone() {
        tracing::info!(endpoint = %xds_cfg.endpoint, node = %xds_cfg.node_id, "connecting to xDS controller");
        match AdsClient::connect(&xds_cfg.endpoint, xds_cfg.node_id.clone()).await {
            Ok(c) => {
                tracing::info!(endpoint = %xds_cfg.endpoint, "xDS ADS channel established");
                Some(c)
            }
            Err(e) => {
                tracing::warn!(err = %e, "xDS connection failed — running with static config (will retry in background)");
                None
            }
        }
    } else {
        tracing::info!("xDS consumer not configured — using static cluster config only");
        None
    };

    // -- 12. Load balancer
    let lb_endpoints = build_lb_endpoints(&config);
    let lb: Arc<dyn LoadBalancer> = build_load_balancer(&config, &lb_endpoints);
    tracing::info!(algorithm = %config.gateway.lb.algorithm, endpoints = lb_endpoints.len(), "load balancer ready");

    // -- 13. Retry policy + budget
    let retry_policy = build_retry_policy(&config);
    let retry_budget = Arc::new(RetryBudget::new(
        config.gateway.retry.budget_percent / 100.0,
        config.gateway.retry.min_concurrency,
    ));

    // -- 14. Response cache (optional — in-memory fallback if KAYA unavailable)
    let cache: Option<Arc<ResponseCache>> = if let Some(cache_cfg) = config.gateway.cache.clone() {
        if cache_cfg.enabled {
            tracing::info!(prefix = %cache_cfg.kaya_prefix, ttl = cache_cfg.default_ttl_secs, "response cache enabled (in-memory fallback)");
            let kv = Arc::new(InMemoryKv::new());
            let policy = CachePolicy {
                default_ttl: Duration::from_secs(cache_cfg.default_ttl_secs),
                max_body_size: cache_cfg.max_body_size,
                ..CachePolicy::default()
            };
            match ResponseCache::new(kv, policy, &registry) {
                Ok(c) => Some(Arc::new(c)),
                Err(e) => {
                    tracing::warn!(err = %e, "response cache metrics registration failed — cache disabled");
                    None
                }
            }
        } else {
            tracing::info!("response cache disabled in config");
            None
        }
    } else {
        tracing::info!("response cache not configured — skipping");
        None
    };

    // -- 15. Auth mode
    let auth_mode = config.gateway.auth_mode.clone();

    // -- 15b. GraphQL limiter (built from security config)
    let gql_limiter: Option<Arc<GraphQLLimiter>> = {
        let cfg = &config.security.graphql_limits;
        if cfg.enabled {
            let limiter = GraphQLLimiter {
                max_depth: cfg.max_depth,
                max_complexity: cfg.max_complexity,
                max_aliases: cfg.max_aliases,
                max_directives: cfg.max_directives,
                introspection_enabled: cfg.introspection_enabled,
            };
            tracing::info!(
                max_depth = cfg.max_depth,
                max_complexity = cfg.max_complexity,
                introspection = cfg.introspection_enabled,
                "GraphQL limiter enabled",
            );
            Some(Arc::new(limiter))
        } else {
            tracing::info!("GraphQL limiter disabled in config");
            None
        }
    };

    // -- 15c. Determine HTTP/1 listen address (needed by shadow sampler port config).
    // Note: the full TCP listener is bound later at step 22b; here we only
    // derive the SocketAddr for port arithmetic in shadow mode.
    let listen_addr_early: SocketAddr = if let Some(listener) = config.gateway.listeners.first() {
        SocketAddr::new(
            listener
                .address
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            listener.port,
        )
    } else {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8443)
    };

    // -- 15d. Shadow mode dispatcher + sampler (runtime=shadow only)
    //
    // The dispatcher owns the bounded mpsc channel and spawns the background
    // drain task that flushes diff events to the configured sink backend.
    // On graceful shutdown, dropping the dispatcher closes the channel which
    // signals the drain task to exit after flushing remaining in-flight events.
    //
    // The `ShadowSampler` is cheap-clone (inner state is Arc/Atomic) and is
    // shared across all request handlers via `GatewayState::shadow_sampler`.
    let shadow_sampler: Option<Arc<ShadowSampler>> =
        if config.gateway.runtime == GatewayRuntime::Shadow
            && config.gateway.shadow_mode.enabled
        {
            let sink_cfg = &config.gateway.shadow_mode.sink;

            // Build the concrete sink backend from config.
            let sink: Arc<dyn ShadowDiffSink> = match sink_cfg.sink_type {
                ShadowSinkType::Redpanda => {
                    let producer = RedpandaProducer::new_logging(); // stub; real broker via rdkafka feature
                    Arc::new(RedpandaSink::new(producer, sink_cfg.redpanda.topic.clone()))
                }
                ShadowSinkType::Sqlite => {
                    match SqliteSink::open(
                        sink_cfg.sqlite.path.clone(),
                        sink_cfg.sqlite.max_rows,
                    ) {
                        Ok(s) => Arc::new(s),
                        Err(e) => {
                            tracing::warn!(
                                path = %sink_cfg.sqlite.path,
                                error = %e,
                                "SQLite shadow sink open failed — falling back to noop"
                            );
                            Arc::new(NoopSink)
                        }
                    }
                }
                ShadowSinkType::Multi => {
                    let producer = RedpandaProducer::new_logging();
                    let redpanda: Arc<dyn ShadowDiffSink> =
                        Arc::new(RedpandaSink::new(producer, sink_cfg.redpanda.topic.clone()));
                    let sqlite: Arc<dyn ShadowDiffSink> =
                        match SqliteSink::open(
                            sink_cfg.sqlite.path.clone(),
                            sink_cfg.sqlite.max_rows,
                        ) {
                            Ok(s) => Arc::new(s),
                            Err(e) => {
                                tracing::warn!(
                                    path = %sink_cfg.sqlite.path,
                                    error = %e,
                                    "SQLite shadow sink open failed in multi-sink"
                                );
                                Arc::new(NoopSink)
                            }
                        };
                    Arc::new(MultiSink::new(vec![redpanda, sqlite]))
                }
                ShadowSinkType::Noop => Arc::new(NoopSink),
            };

            // Register sink metrics on the shared Prometheus registry.
            let sink_metrics = match SinkMetrics::new(&registry) {
                Ok(m) => {
                    tracing::info!("shadow sink metrics registered");
                    Some(Arc::new(m))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "shadow sink metrics registration failed");
                    None
                }
            };

            // Spawn the background drain task.
            let dispatcher = ShadowDiffDispatcher::start(
                sink,
                sink_cfg.channel_capacity,
                sink_metrics,
            );
            let sender = dispatcher.sender();

            // Build the sampler from the shadow_mode config.
            let shadow_rate_percent =
                (config.gateway.shadow_mode.sample_rate * 100.0).round() as u32;
            let shadow_config = armageddon_forge::pingora::shadow::ShadowModeConfig {
                enabled: true,
                hyper_port: listen_addr_early.port(),
                pingora_port: listen_addr_early.port() + 1,
                sample_rate_percent: shadow_rate_percent,
                shadow_timeout_ms: 5_000,
            };

            let sampler = ShadowSampler::new(&shadow_config).with_sink(sender);

            tracing::info!(
                sample_rate_percent = shadow_rate_percent,
                sink_type = ?config.gateway.shadow_mode.sink.sink_type,
                "shadow mode dispatcher + sampler ready"
            );

            // Keep the dispatcher alive for the process lifetime by leaking it
            // into a Box.  The drain task exits when the last DiffEventSender
            // (owned by the sampler) is dropped at process shutdown.
            // We deliberately hold the dispatcher in a Box so its Drop runs
            // only when the owning Box is dropped — currently we keep it alive
            // until process exit by mem::forget, since graceful shutdown of the
            // Pingora thread calls std::process::exit before Rust destructors run.
            //
            // For the hyper path (shadow mode), the drain task is joined
            // implicitly when the tokio runtime shuts down.
            std::mem::forget(dispatcher);

            Some(sampler)
        } else {
            if config.gateway.runtime == GatewayRuntime::Shadow {
                tracing::info!("shadow mode runtime active but shadow_mode.enabled=false — sampler not created");
            }
            None
        };

    // -- 16. Shared gateway state
    let state = Arc::new(GatewayState {
        pipeline: Arc::clone(&pentagon),
        forge: Arc::clone(&forge),
        veil,
        auth_mode,
        github_webhook,
        lb,
        lb_endpoints,
        retry_policy,
        retry_budget,
        cache,
        mesh: mesh.clone(),
        gql_limiter,
        requests_denied: rl_denied_counter,
        shadow_sampler,
    });

    // -- 17. Shutdown broadcast channel
    let (shutdown_tx, _) = broadcast::channel::<()>(16);

    // -- 18. Spawn Mesh SVID rotation task (optional)
    if let Some(m) = mesh.clone() {
        let rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            Arc::clone(&m).run(rx).await;
        });
        tracing::info!("mTLS SVID rotation task spawned");
    }

    // -- 19. Spawn xDS ADS consumer task (optional)
    if let Some(client) = xds_client {
        tokio::spawn(async move {
            let cb = Arc::new(LoggingXdsCallback);
            if let Err(e) = client.run(cb).await {
                tracing::error!(err = %e, "xDS ADS consumer exited with error");
            }
        });
        tracing::info!("xDS ADS consumer task spawned");
    }

    // -- 20. Spawn Admin API (optional)
    if let Some(admin_cfg) = config.gateway.admin.clone() {
        if admin_cfg.enabled {
            let bind_addr: IpAddr = admin_cfg
                .bind_addr
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
            let server_cfg = AdminServerConfig {
                bind_addr,
                port: admin_cfg.port,
                admin_token: admin_cfg.admin_token.clone(),
            };
            let admin_state = AdminState::new(
                config.gateway.clone(),
                args.config_path.clone(),
            );
            let server = AdminServer::new(server_cfg, admin_state);
            let rx = shutdown_tx.subscribe();
            tokio::spawn(async move {
                if let Err(e) = server.run(rx).await {
                    tracing::error!(err = %e, "admin server exited with error");
                }
            });
            tracing::info!(port = admin_cfg.port, "admin API task spawned");
        }
    }

    // -- 20b. Spawn Envoy-style Admin API (loopback :9099 by default).
    if let Some(admin_api_cfg) = config.gateway.admin_api.clone() {
        if admin_api_cfg.enabled {
            let stats = Arc::new(admin_providers::ForgePrometheusStatsProvider::from_registry(
                Arc::new(registry.clone()),
            ));
            let clusters_provider = Arc::new(admin_providers::RuntimeClusterProvider::new(
                Arc::clone(&forge),
                Arc::clone(&config),
            ));
            let config_dumper = Arc::new(admin_providers::GatewayConfigDumper::new(
                Arc::clone(&config),
            ));
            let runtime_provider = Arc::new(
                admin_providers::StaticRuntimeProvider::from_config(config.as_ref()),
            );
            let health_provider = Arc::new(admin_providers::PentagonHealthProvider::new(
                Arc::clone(&pentagon),
            ));

            match AdminApi::build(
                admin_api_cfg,
                stats,
                clusters_provider,
                config_dumper,
                runtime_provider,
                health_provider,
            ) {
                Ok(api) => {
                    let bind = api.bind_addr();
                    tracing::info!("Admin API listening on {bind}");
                    let rx = shutdown_tx.subscribe();
                    tokio::spawn(async move {
                        if let Err(e) = api.run(rx).await {
                            tracing::error!(err = %e, "admin-api exited with error");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(err = %e, "admin-api disabled (build failed)");
                }
            }
        } else {
            tracing::info!("admin-api disabled in config");
        }
    }

    // -- 21. Determine HTTP/1 listen address from first listener config
    let listen_addr = if let Some(listener) = config.gateway.listeners.first() {
        SocketAddr::new(
            listener
                .address
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            listener.port,
        )
    } else {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8443)
    };

    // -- 22. Runtime dispatch: Pingora | Shadow | Hyper (legacy)
    //
    // Pingora's `Server::run_forever()` never returns (it calls
    // `std::process::exit`), so it must run on a dedicated OS thread.
    // In shadow mode we boot both backends simultaneously: Pingora shadows
    // all traffic routed through the hyper path via `ShadowSampler`.
    // In pure `pingora` mode the hyper accept loop below is skipped.

    let pingora_thread: Option<std::thread::JoinHandle<()>> = match config.gateway.runtime {
        GatewayRuntime::Pingora | GatewayRuntime::Shadow => {
            let is_shadow = config.gateway.runtime == GatewayRuntime::Shadow;
            let runtime_label = if is_shadow { "shadow" } else { "pingora" };

            // Derive the listen address for Pingora.
            // Shadow mode: Pingora listens on :8081 so hyper keeps :8080.
            // Pure pingora mode: Pingora listens on the primary address.
            let pingora_listen = if is_shadow {
                let shadow_port = listen_addr.port() + 1;
                format!("{}:{}", listen_addr.ip(), shadow_port)
            } else {
                listen_addr.to_string()
            };

            tracing::info!(
                runtime = runtime_label,
                addr = %pingora_listen,
                "booting PingoraGateway",
            );

            // -- Prometheus metrics for Pingora subsystems, registered on the
            // shared `registry` so they appear in `/stats`.
            // Pass `_pingora_metrics` to `spawn_xds_watcher` / `spawn_svid_rotation_bridge`
            // when those are wired into the Pingora startup path.
            let _pingora_metrics = match armageddon_forge::pingora::metrics::PingoraMetrics::new(&registry) {
                Ok(m) => {
                    tracing::info!("PingoraMetrics registered on shared Prometheus registry");
                    Some(Arc::new(m))
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "PingoraMetrics registration failed — Pingora metrics will be unavailable"
                    );
                    None
                }
            };

            // Build upstream registry from cluster config.
            let upstream_reg = Arc::new(UpstreamRegistry::new());
            for cluster in config.gateway.clusters.iter() {
                let eps: Vec<armageddon_common::types::Endpoint> = cluster
                    .endpoints
                    .iter()
                    .map(|e| armageddon_common::types::Endpoint {
                        address: e.address.clone(),
                        port: e.port,
                        weight: e.weight,
                        healthy: true,
                    })
                    .collect();
                upstream_reg.update_cluster(&cluster.name, eps);
            }

            // ── Build RouterFilter from yaml gateway.routes ──
            //
            // Without this, the static `routes:` table from the yaml is never
            // applied and `upstream_peer` falls back to the brittle
            // path-prefix-as-cluster heuristic which does not match the
            // declared cluster names.  The first route whose cluster matches
            // the catch-all `prefix: /` (typically `default-backend`) is used
            // as the gateway's default cluster fallback.
            let mut prefix_routes: Vec<(String, String)> = Vec::new();
            let mut exact_routes: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut regex_routes: Vec<(regex::Regex, String)> = Vec::new();
            let mut yaml_default_cluster: Option<String> = None;

            for route in config.gateway.routes.iter() {
                if let Some(p) = &route.match_rule.prefix {
                    if p == "/" || p.is_empty() {
                        // Treat the catch-all prefix as the default cluster.
                        yaml_default_cluster.get_or_insert_with(|| route.cluster.clone());
                    } else {
                        prefix_routes.push((p.clone(), route.cluster.clone()));
                    }
                }
                if let Some(p) = &route.match_rule.path {
                    exact_routes.insert(p.clone(), route.cluster.clone());
                }
                if let Some(re) = &route.match_rule.regex {
                    match regex::Regex::new(re) {
                        Ok(r) => regex_routes.push((r, route.cluster.clone())),
                        Err(e) => tracing::warn!(
                            route = %route.name, regex = %re, err = %e,
                            "skipping route — invalid regex"
                        ),
                    }
                }
            }

            // Default cluster: yaml catch-all → first cluster name → "default".
            let default_cluster = yaml_default_cluster
                .or_else(|| config.gateway.clusters.first().map(|c| c.name.clone()))
                .unwrap_or_else(|| "default".to_string());

            let route_table = RouteTable::new(
                exact_routes,
                prefix_routes,
                regex_routes,
                default_cluster.clone(),
            );
            let router_filter: SharedFilter = std::sync::Arc::new(RouterFilter::new(route_table));
            tracing::info!(
                default_cluster = %default_cluster,
                routes = config.gateway.routes.len(),
                "router filter built from yaml routes"
            );

            // ── Build WafFilter from yaml gateway.waf ──
            //
            // The WAF runs FIRST in the chain (before the router) so that
            // injection / SSRF / scanner traffic never reaches OPA, JWT, or
            // upstream selection. Skipped silently when absent or disabled.
            let mut filters: Vec<SharedFilter> = Vec::with_capacity(2);
            if let Some(waf_cfg) = config.gateway.waf.as_ref() {
                if waf_cfg.enabled {
                    // ── Coraza-on-Pingora preference (feature-gated) ──
                    //
                    // When the `coraza-wasm` Cargo feature is enabled AND
                    // `gateway.waf.wasm_module` points at a `.wasm` file,
                    // we prefer the Coraza filter over the regex one.
                    // Module-load failure with `fail_closed_on_load_error=false`
                    // (the regex WAF default) silently falls back to regex.
                    #[allow(unused_mut)]
                    let mut waf_registered = false;

                    #[cfg(feature = "coraza-wasm")]
                    if let Some(wasm_path) = waf_cfg.wasm_module.as_deref() {
                        // Default `coraza.conf` path: sibling of the
                        // wasm module.  Operators can override by
                        // placing the file there or leaving it absent
                        // (Coraza falls back to its built-in defaults).
                        let coraza_conf_path = std::path::PathBuf::from(wasm_path)
                            .parent()
                            .map(|p| p.join("coraza.conf"))
                            .filter(|p| p.exists());

                        let coraza_cfg = armageddon_forge::pingora::filters::waf_coraza::WafCorazaConfig {
                            enabled: true,
                            wasm_module_path: std::path::PathBuf::from(wasm_path),
                            coraza_conf_path,
                            block_status: waf_cfg.block_status,
                            learning_mode: waf_cfg.learning_mode,
                            // Conservative default: fail-open and keep
                            // serving on a bad .wasm so the regex WAF
                            // takes over.  Operators tighten via config.
                            fail_closed_on_load_error: false,
                            max_body_inspect_bytes: 256 * 1024,
                        };
                        match armageddon_forge::pingora::filters::waf_coraza::WafCorazaFilter::new(
                            coraza_cfg,
                            &registry,
                        ) {
                            Ok(filter) => {
                                tracing::info!(
                                    wasm_module = %wasm_path,
                                    block_status = waf_cfg.block_status,
                                    "WAF filter registered (Coraza proxy-wasm v0.2.1)"
                                );
                                let waf_filter: SharedFilter = std::sync::Arc::new(filter);
                                filters.push(waf_filter);
                                waf_registered = true;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    err = %e,
                                    wasm_module = %wasm_path,
                                    "Coraza WAF load failed — falling back to regex WAF"
                                );
                            }
                        }
                    }

                    if !waf_registered {
                        let runtime_cfg =
                            armageddon_forge::pingora::filters::waf::WafFilterConfig {
                                enabled: true,
                                paranoia_level: waf_cfg.paranoia_level,
                                learning_mode: waf_cfg.learning_mode,
                                block_status: waf_cfg.block_status,
                                // Body inspection cap (256 KiB default).
                                // Larger bodies are inspected up to the cap with
                                // body_buffer_overflow=true on the request ctx.
                                max_body_inspect_bytes: 256 * 1024,
                            };
                        match armageddon_forge::pingora::filters::waf::WafFilter::new(
                            runtime_cfg,
                            &registry,
                        ) {
                            Ok(filter) => {
                                tracing::info!(
                                    paranoia_level = waf_cfg.paranoia_level,
                                    learning_mode = waf_cfg.learning_mode,
                                    block_status = waf_cfg.block_status,
                                    "WAF filter registered (regex engine, native Rust)"
                                );
                                let waf_filter: SharedFilter = std::sync::Arc::new(filter);
                                filters.push(waf_filter);
                            }
                            Err(e) => {
                                tracing::error!(err = %e, "WAF filter init failed — continuing without WAF");
                            }
                        }
                    }
                } else {
                    tracing::info!("WAF configured but disabled — zero overhead");
                }
            } else {
                tracing::info!("WAF not configured — skipping");
            }
            filters.push(router_filter);

            let gw_cfg = PingoraGatewayConfig {
                default_cluster,
                upstream_tls: false,
                upstream_timeout_ms: 30_000,
                pool_size: 128,
                filters,
                compression: None,
            };
            let gateway = PingoraGateway::new(gw_cfg, Arc::clone(&upstream_reg));

            match pingora_build_server(gateway, &pingora_listen) {
                Ok(server) => {
                    let handle = std::thread::Builder::new()
                        .name("pingora-main".to_string())
                        .spawn(move || {
                            tracing::info!("PingoraGateway thread starting");
                            server.run_forever(); // never returns
                        })
                        .expect("failed to spawn Pingora OS thread");
                    Some(handle)
                }
                Err(e) => {
                    if is_shadow {
                        tracing::warn!(
                            err = %e,
                            "Pingora shadow server failed to build — \
                             continuing in hyper-only mode"
                        );
                        None
                    } else {
                        // Pure pingora mode: build failure is fatal.
                        return Err(e.context("PingoraGateway build failed"));
                    }
                }
            }
        }
        GatewayRuntime::Hyper => {
            #[allow(deprecated)]
            tracing::warn!(
                "ARMAGEDDON is running with the legacy hyper backend \
                 (runtime=hyper). This backend is deprecated since v2.0 \
                 and will be removed in v3.0. Migrate to runtime=pingora \
                 via the shadow validation window described in CUTOVER.md."
            );
            None
        }
    };

    // In pure Pingora mode the hyper accept loop below is vestigial —
    // Pingora's thread has taken over the primary listen address.
    // We still bind (on the same port) so the TCP listener creation code
    // compiles cleanly; `run_forever()` on the Pingora side means the
    // process will exit from that thread before the accept loop is
    // meaningfully hot. In a future v3.0 refactor the hyper path is removed.
    let skip_hyper_accept_loop = config.gateway.runtime == GatewayRuntime::Pingora
        && pingora_thread.is_some();

    // -- 22b. Bind HTTP/1+2 TCP listener (hyper)
    //
    // Pre-v2.1 fix: when runtime=pingora the hyper bind would race-claim
    // the port and Pingora's own bind on the same port would loop forever
    // ("0.0.0.0:8080 is in use, will try again"). The hyper listener is
    // only needed when the hyper accept loop is actually consumed
    // (runtime=hyper or runtime=shadow). In pure pingora mode we skip the
    // hyper bind entirely so Pingora can claim the port cleanly.
    let tcp_listener: Option<TcpListener> = if skip_hyper_accept_loop {
        tracing::info!(
            addr = %listen_addr,
            "HTTP/1 listener skipped — pure Pingora runtime owns the port"
        );
        None
    } else {
        let l = TcpListener::bind(listen_addr)
            .await
            .context(format!("failed to bind HTTP/1 listener on {}", listen_addr))?;
        tracing::info!(addr = %listen_addr, "HTTP/1 listener bound");
        Some(l)
    };

    // -- 23. Bind HTTP/3 QUIC listener (optional)
    let http3_handle: Option<tokio::task::JoinHandle<anyhow::Result<()>>> =
        if let Some(quic_cfg) = config.gateway.quic.clone() {
            let quic_listener_cfg = QuicListenerConfig {
                address: quic_cfg.address.clone(),
                port: quic_cfg.port,
                cert_path: quic_cfg.cert_path.clone(),
                key_path: quic_cfg.key_path.clone(),
                max_concurrent_streams: quic_cfg.max_concurrent_streams,
            };
            let rx = shutdown_tx.subscribe();
            let bridge = Arc::new(Http3Bridge {
                state: Arc::clone(&state),
            });
            match Http3Server::new(quic_listener_cfg).await {
                Ok(server) => {
                    tracing::info!(port = quic_cfg.port, "HTTP/3 QUIC listener bound");
                    let handle = tokio::spawn(async move {
                        server
                            .run(bridge, rx)
                            .await
                            .map_err(|e| anyhow::anyhow!("HTTP/3 server error: {}", e))
                    });
                    Some(handle)
                }
                Err(e) => {
                    tracing::warn!(err = %e, "HTTP/3 QUIC bind failed — continuing without HTTP/3");
                    None
                }
            }
        } else {
            tracing::info!("HTTP/3 (QUIC) not configured — skipping");
            None
        };

    tracing::info!(
        runtime = ?config.gateway.runtime,
        "ARMAGEDDON is operational",
    );

    // -- 24. HTTP/1 accept loop
    //
    // When runtime=pingora, Pingora owns the primary port. The hyper accept
    // loop is started only for runtime=hyper and runtime=shadow (where hyper
    // is the primary path and Pingora shadows on port+1).
    if skip_hyper_accept_loop {
        // Pure Pingora mode: block the async task until Ctrl-C so graceful
        // shutdown propagates. Pingora's thread exits independently.
        tracing::info!("hyper accept loop skipped — Pingora is primary runtime");
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for shutdown signal");
        tracing::info!("shutdown signal received");
        let _ = shutdown_tx.send(());
        if let Some(h) = http3_handle {
            let _ = tokio::time::timeout(Duration::from_secs(30), h).await;
        }
        tracing::info!("ARMAGEDDON shutdown complete");
        return Ok(());
    }

    // At this point skip_hyper_accept_loop is false → tcp_listener is Some.
    let tcp_listener = tcp_listener
        .expect("hyper accept loop active but tcp_listener is None — invariant violated");

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            accept_result = tcp_listener.accept() => {
                match accept_result {
                    Ok((stream, peer_addr)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            let service = service_fn(move |req: Request<Incoming>| {
                                let state = Arc::clone(&state);
                                async move {
                                    handle_request(req, peer_addr, state).await
                                }
                            });

                            if let Err(e) = http1::Builder::new()
                                .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                                .await
                            {
                                tracing::debug!("connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("accept error: {}", e);
                    }
                }
            }
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received — draining connections");
                break;
            }
        }
    }

    // -- 25. Graceful shutdown (30 s timeout)
    let _ = shutdown_tx.send(());
    if let Some(h) = http3_handle {
        let _ = tokio::time::timeout(Duration::from_secs(30), h).await;
    }

    tracing::info!("ARMAGEDDON shutdown complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

/// Build an LB endpoint pool from the cluster config.
///
/// Flattens all cluster endpoints into a single pool for the shared LB.
/// In production, per-cluster pools should be maintained; this covers the
/// common single-cluster deployment.
fn build_lb_endpoints(config: &armageddon_config::ArmageddonConfig) -> Vec<Arc<Endpoint>> {
    config
        .gateway
        .clusters
        .iter()
        .flat_map(|c| {
            c.endpoints.iter().map(|e| {
                Arc::new(Endpoint::new(
                    format!("{}:{}", e.address, e.port),
                    format!("{}:{}", e.address, e.port),
                    e.weight,
                ))
            })
        })
        .collect()
}

/// Instantiate the correct LB algorithm from the config string.
fn build_load_balancer(
    config: &armageddon_config::ArmageddonConfig,
    endpoints: &[Arc<Endpoint>],
) -> Arc<dyn LoadBalancer> {
    let algo = config.gateway.lb.algorithm.as_str();
    let eps_vec: Vec<Arc<Endpoint>> = endpoints.to_vec();
    match algo {
        "least_conn" => Arc::new(LeastConnections::new()),
        "p2c" => Arc::new(PowerOfTwoChoices::new()),
        "ring_hash" => Arc::new(RingHash::new(eps_vec)),
        "maglev" => Arc::new(Maglev::new(eps_vec)),
        "weighted" => Arc::new(WeightedRoundRobin::new(eps_vec)),
        "random" => Arc::new(Random::new()),
        _ => Arc::new(RoundRobin::new()),
    }
}

/// Build a retry policy from config.
fn build_retry_policy(config: &armageddon_config::ArmageddonConfig) -> RetryPolicy {
    let rc = &config.gateway.retry;
    RetryPolicy {
        max_retries: rc.max_retries,
        per_try_timeout: Duration::from_millis(rc.per_try_timeout_ms),
        overall_timeout: Duration::from_millis(rc.overall_timeout_ms),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// HTTP request handling (HTTP/1 entrypoint + shared logic)
// ---------------------------------------------------------------------------

/// Handle a single HTTP/1 request through the full ARMAGEDDON pipeline.
async fn handle_request(
    req: Request<Incoming>,
    peer_addr: SocketAddr,
    state: Arc<GatewayState>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let (parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => Bytes::new(),
    };

    let method = parts.method.to_string();
    let uri = parts.uri.to_string();
    let path = parts.uri.path().to_string();
    let query = parts.uri.query().map(|q| q.to_string());

    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            headers.insert(name.as_str().to_lowercase(), v.to_string());
        }
    }

    let http_req = HttpRequest {
        method: method.clone(),
        uri,
        path,
        query,
        headers,
        body: if body_bytes.is_empty() {
            None
        } else {
            Some(body_bytes.to_vec())
        },
        version: HttpVersion::Http11,
    };

    let resp = handle_http_request(http_req, peer_addr, state).await;
    build_hyper_response(resp)
}

/// Static body returned by the internal `/armageddon/healthz` endpoint.
/// Kept as a `&'static str` so the hot path is zero-allocation.
const ARMAGEDDON_HEALTHZ_BODY: &str = r#"{"status":"ok","version":"1.1.0"}"#;

/// Core request handler shared by HTTP/1 (hyper) and HTTP/3 (QUIC) paths.
async fn handle_http_request(
    http_req: HttpRequest,
    peer_addr: SocketAddr,
    state: Arc<GatewayState>,
) -> HttpResponse {
    let method = http_req.method.clone();
    let path = http_req.path.clone();
    let headers = http_req.headers.clone();

    // --- Internal /armageddon/healthz (bypass WAF, Pentagon, proxy) ---
    // Matched BEFORE CORS/Aho-Corasick WAF/load balancer for p95 < 50 ms
    // on OVH Scale A7 2026. Zero-allocation: returns a static string.
    if path == "/armageddon/healthz" {
        let mut h = HashMap::new();
        h.insert("content-type".to_string(), "application/json".to_string());
        h.insert("cache-control".to_string(), "no-store".to_string());
        return HttpResponse {
            status: 200,
            headers: h,
            body: Some(ARMAGEDDON_HEALTHZ_BODY.as_bytes().to_vec()),
        };
    }

    // --- CORS preflight ---
    if CorsHandler::is_preflight(&method, &headers) {
        if let Some(origin) = headers.get("origin").cloned() {
            if let Some(cors_headers) = state.forge.cors_handler().build_headers_for_origin(&origin) {
                return HttpResponse {
                    status: 204,
                    headers: cors_headers.into_iter().collect(),
                    body: None,
                };
            }
        }
        return error_response(403, "cors_rejected", "CORS origin not allowed");
    }

    // --- GitHub webhook fast path ---
    if method == "POST" && path == "/webhooks/github" {
        let body_bytes = http_req.body.as_deref().map(Bytes::copy_from_slice).unwrap_or_default();
        return handle_github_webhook_inner(&state, &headers, &body_bytes, &peer_addr).await;
    }

    // --- Cache lookup (GET only, before Pentagon) ---
    let cache_hit = if method == "GET" {
        if let Some(cache) = &state.cache {
            match cache.get(&http_req).await {
                Ok(Some(cached)) => {
                    tracing::debug!(path = %path, "cache hit");
                    return HttpResponse {
                        status: cached.status,
                        headers: cached.headers.into_iter().collect(),
                        body: Some(cached.body.to_vec()),
                    };
                }
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!(err = %e, "cache get error — bypassing cache");
                    false
                }
            }
        } else {
            false
        }
    } else {
        false
    };
    let _ = cache_hit; // may be extended later

    // --- Detect protocol ---
    let protocol = if Router::is_grpc(&headers) {
        Protocol::Grpc
    } else if Router::is_graphql(&path) {
        Protocol::GraphQL
    } else {
        Protocol::Http
    };

    // --- GraphQL depth/complexity limiter ---
    // Applied before auth and the Pentagon pipeline so malformed or oversized
    // GraphQL documents are rejected cheaply at the edge.
    if protocol == Protocol::GraphQL {
        if let Some(limiter) = &state.gql_limiter {
            let content_type = headers.get("content-type").map(|s| s.as_str());
            let body_slice = http_req.body.as_deref().unwrap_or(&[]);
            if let Some(query_str) = extract_gql_query(content_type, body_slice) {
                if let Err(e) = limiter.validate_query(&query_str) {
                    let (status, code, msg) = match &e {
                        GqlLimitError::IntrospectionDisabled => (
                            403u16,
                            "gql_introspection_disabled",
                            e.to_string(),
                        ),
                        GqlLimitError::DepthExceeded { .. }
                        | GqlLimitError::ComplexityExceeded { .. }
                        | GqlLimitError::AliasesExceeded { .. }
                        | GqlLimitError::DirectivesExceeded { .. } => (
                            400,
                            "gql_limit_exceeded",
                            e.to_string(),
                        ),
                        GqlLimitError::Parse(_)
                        | GqlLimitError::UnknownFragment(_)
                        | GqlLimitError::CyclicFragments(_) => (400, "gql_parse_error", e.to_string()),
                    };
                    tracing::warn!(
                        path = %path,
                        error = %e,
                        "GraphQL request rejected by limiter",
                    );
                    return error_response(status, code, &msg);
                }
            }
        }
    }

    let conn_info = ConnectionInfo {
        client_ip: peer_addr.ip(),
        client_port: peer_addr.port(),
        server_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        server_port: 0,
        tls: None,
        ja3_fingerprint: None,
        ja4_fingerprint: None,
    };

    let mut ctx = RequestContext::new(http_req.clone(), conn_info, protocol);

    // --- Route matching ---
    let matched_route = state.forge.router().match_route(&method, &path, &headers);

    let (cluster_name, timeout_ms, auth_skip) = match matched_route {
        Some(route) => {
            ctx.matched_route = Some(route.name.clone());
            ctx.target_cluster = Some(route.cluster.clone());
            (route.cluster.clone(), route.timeout_ms, route.auth_skip)
        }
        None => {
            tracing::debug!("no route matched for {} {}", method, path);
            return error_response(
                404,
                "not_found",
                &format!("No route for {} {}", method, path),
            );
        }
    };

    // --- Authentication ---
    if !auth_skip {
        if let Err(reason) = authenticate(&state, &headers, &mut ctx).await {
            tracing::warn!(request_id = %ctx.request_id, path = %path, "auth failed: {}", reason);
            return error_response(401, "unauthorized", "Authentication required");
        }
    }

    // --- Pentagon security pipeline ---
    let verdict = match state.pipeline.inspect(&ctx).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Pentagon pipeline error: {}", e);
            return error_response(503, "security_pipeline_error", "Security inspection failed");
        }
    };

    // --- Act on verdict ---
    match verdict.action {
        Action::Block => {
            tracing::warn!(
                request_id = %ctx.request_id,
                score = verdict.score,
                "BLOCKED: {}",
                verdict.reason,
            );
            return error_response(403, "blocked", "Request blocked by ARMAGEDDON security gateway");
        }
        Action::Challenge => {
            tracing::info!(
                request_id = %ctx.request_id,
                score = verdict.score,
                "CHALLENGE: {}",
                verdict.reason,
            );
            return error_response(429, "challenge_required", "Verification required");
        }
        Action::Throttle => {
            tracing::info!(request_id = %ctx.request_id, "THROTTLE: {}", verdict.reason);
        }
        Action::LogOnly => {
            tracing::debug!(request_id = %ctx.request_id, "LOG: {}", verdict.reason);
        }
        Action::Forward => {}
    }

    // --- Rate limit check (before upstream) ---
    // Point d'intégration: armageddon/src/main.rs, juste avant select_endpoint.
    // Le filter est None quand rate_limit absent ou disabled dans la config.
    if let Some(rl_filter) = state.forge.rate_limit_filter() {
        use armageddon_ratelimit::RateLimitDecision;
        match rl_filter.check(&http_req).await {
            RateLimitDecision::Allow => {
                // Continue normally.
            }
            RateLimitDecision::Deny { retry_after_secs } => {
                state.requests_denied.with_label_values(&["rate_limit"]).inc();
                tracing::warn!(
                    request_id = %ctx.request_id,
                    path = %path,
                    retry_after = retry_after_secs,
                    "rate limit exceeded — returning 429",
                );
                let body = serde_json::json!({
                    "error": "rate_limit_exceeded",
                    "retry_after": retry_after_secs,
                })
                .to_string()
                .into_bytes();
                let mut h = std::collections::HashMap::new();
                h.insert("content-type".to_string(), "application/json".to_string());
                h.insert("retry-after".to_string(), retry_after_secs.to_string());
                return HttpResponse { status: 429, headers: h, body: Some(body) };
            }
            RateLimitDecision::Shadow { retry_after_secs } => {
                // Shadow mode: over-limit but forward anyway (dry-run).
                tracing::warn!(
                    request_id = %ctx.request_id,
                    path = %path,
                    retry_after = retry_after_secs,
                    "rate limit shadow: would deny, forwarding anyway",
                );
            }
        }
    }

    // --- Proxy to upstream via LB ---
    let endpoint = match select_endpoint(&state, &cluster_name) {
        Some(ep) => ep,
        None => {
            tracing::error!("no healthy upstream for cluster '{}'", cluster_name);
            return error_response(
                503,
                "no_upstream",
                &format!("No healthy upstream for cluster '{}'", cluster_name),
            );
        }
    };

    // Record request start on circuit breaker
    if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
        breaker.on_request_start();
    }

    // Forward — inject identity headers
    let mut header_pairs: Vec<(String, String)> = headers.into_iter().collect();
    armageddon_veil::Veil::inject_identity_headers(&mut header_pairs, &ctx);

    // Mesh active → flag upstream for mTLS-aware backends.
    if state.mesh.is_some() {
        header_pairs.push(("x-faso-mesh".to_string(), "active".to_string()));
    }

    let body_option = http_req
        .body
        .as_ref()
        .map(|b| Bytes::copy_from_slice(b));

    // Wrap upstream call in the retry loop (budget + backoff + jitter).
    let fwd_req = ForwardReq {
        endpoint: endpoint.clone(),
        method: method.clone(),
        path: path.clone(),
        headers: header_pairs.clone(),
        body: body_option.clone(),
        timeout_ms,
    };
    let proxy_result = execute_with_retry(
        &state.retry_policy,
        &state.retry_budget,
        fwd_req,
        |r| async move {
            armageddon_forge::proxy::forward_request(
                &r.endpoint,
                &r.method,
                &r.path,
                &r.headers,
                r.body.clone(),
                r.timeout_ms,
            )
            .await
        },
    )
    .await
    .map_err(|retry_err| {
        tracing::warn!(
            request_id = %ctx.request_id,
            cluster = %cluster_name,
            error = %retry_err,
            "retry loop terminated",
        );
        armageddon_common::error::ArmageddonError::UpstreamConnection(retry_err.to_string())
    });

    // Record circuit breaker result
    if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
        breaker.on_request_end();
    }

    match proxy_result {
        Ok(proxy_resp) => {
            if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
                breaker.record_success();
            }

            let mut response_headers_vec = proxy_resp.headers.clone();
            state.veil.process_response_headers(&mut response_headers_vec);
            let response_headers: HashMap<String, String> =
                response_headers_vec.into_iter().collect();

            // Cache PUT for successful GET responses
            if method == "GET" && proxy_resp.status == 200 {
                if let Some(cache) = &state.cache {
                    let upstream_resp = armageddon_common::types::HttpResponse {
                        status: proxy_resp.status,
                        headers: response_headers.clone(),
                        body: Some(proxy_resp.body.to_vec()),
                    };
                    if let Err(e) = cache
                        .put(&http_req, &upstream_resp, Duration::from_secs(60))
                        .await
                    {
                        tracing::debug!(err = %e, "cache put skipped");
                    }
                }
            }

            HttpResponse {
                status: proxy_resp.status,
                headers: response_headers,
                body: Some(proxy_resp.body.to_vec()),
            }
        }
        Err(e) => {
            if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
                breaker.record_failure();
            }
            tracing::error!(
                request_id = %ctx.request_id,
                cluster = %cluster_name,
                upstream = %endpoint.address,
                "upstream error: {}",
                e,
            );
            error_response(502, "upstream_error", "Bad gateway")
        }
    }
}

// ---------------------------------------------------------------------------
// Retryable upstream request (wraps forward_request args for execute_with_retry)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ForwardReq {
    endpoint: armageddon_common::types::Endpoint,
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Bytes>,
    timeout_ms: u64,
}

impl RetryableRequest for ForwardReq {
    type Response = armageddon_forge::proxy::ProxyResponse;
    type Error = armageddon_common::error::ArmageddonError;

    fn clone_for_retry(&self) -> Self {
        self.clone()
    }

    fn is_retryable_error(e: &Self::Error) -> bool {
        use armageddon_common::error::ArmageddonError;
        matches!(
            e,
            ArmageddonError::UpstreamTimeout(_) | ArmageddonError::UpstreamConnection(_)
        )
    }

    fn retryable_status(resp: &Self::Response) -> Option<u16> {
        // 5xx and 429 are treated as upstream-signalled retry-worthy.
        if resp.status == 429 || (500..=599).contains(&resp.status) {
            Some(resp.status)
        } else {
            None
        }
    }

    fn retry_after(resp: &Self::Response) -> Option<Duration> {
        resp.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
            .and_then(|(_, v)| RetryPolicy::parse_retry_after(v))
    }
}

// ---------------------------------------------------------------------------
// LB endpoint selection (adapts ForgeServer endpoint type to LB endpoint type)
// ---------------------------------------------------------------------------

/// Select an upstream endpoint via the configured LB algorithm.
///
/// Falls back to `ForgeServer::select_upstream` which uses the Forge-internal
/// round-robin when the LB pool is empty.
fn select_endpoint(
    state: &GatewayState,
    cluster_name: &str,
) -> Option<armageddon_common::types::Endpoint> {
    // Prefer the LB pool if populated
    if !state.lb_endpoints.is_empty() {
        if let Some(ep) = state.lb.select(&state.lb_endpoints, None) {
            return Some(armageddon_common::types::Endpoint {
                address: ep.address.split(':').next().unwrap_or(&ep.address).to_string(),
                port: ep.address
                    .split(':')
                    .nth(1)
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080),
                weight: ep.weight,
                healthy: ep.is_healthy(),
            });
        }
    }
    // Fallback to Forge's built-in selection
    state.forge.select_upstream(cluster_name)
}

// ---------------------------------------------------------------------------
// Authentication helper
// ---------------------------------------------------------------------------

async fn authenticate(
    state: &GatewayState,
    headers: &HashMap<String, String>,
    ctx: &mut RequestContext,
) -> Result<(), String> {
    match state.auth_mode {
        AuthMode::Jwt => authenticate_jwt(state, headers, ctx).await,
        AuthMode::Session => authenticate_session(state, headers, ctx).await,
        AuthMode::Dual => {
            // Try JWT first, fall back to session
            if authenticate_jwt(state, headers, ctx).await.is_ok() {
                Ok(())
            } else {
                authenticate_session(state, headers, ctx).await
            }
        }
    }
}

async fn authenticate_jwt(
    state: &GatewayState,
    headers: &HashMap<String, String>,
    ctx: &mut RequestContext,
) -> Result<(), String> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| "missing Authorization header".to_string())?;
    let token = JwtValidator::extract_bearer(auth_header)
        .ok_or_else(|| "invalid Authorization header (expected Bearer token)".to_string())?;
    let claims = state
        .forge
        .jwt_validator()
        .validate(token)
        .await
        .map_err(|e| e.to_string())?;
    ctx.user_id = claims.get("sub").and_then(|v| v.as_str()).map(|s| s.to_string());
    ctx.tenant_id = claims.get("tenant_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    ctx.user_roles = claims
        .get("roles")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    ctx.jwt_claims = Some(claims);
    Ok(())
}

async fn authenticate_session(
    state: &GatewayState,
    headers: &HashMap<String, String>,
    ctx: &mut RequestContext,
) -> Result<(), String> {
    let cookie = headers
        .get("cookie")
        .ok_or_else(|| "missing session cookie".to_string())?;
    let session = state
        .forge
        .kratos_validator()
        .validate_session(cookie)
        .await
        .map_err(|e| e.to_string())?;
    ctx.user_id = Some(session.user_id);
    ctx.tenant_id = session.tenant_id;
    ctx.user_roles = session.roles;
    Ok(())
}

// ---------------------------------------------------------------------------
// GitHub webhook helper
// ---------------------------------------------------------------------------

async fn build_github_webhook_handler(
    config: &armageddon_config::ArmageddonConfig,
) -> Option<Arc<GithubWebhookHandler>> {
    if !config.gateway.webhooks.github.enabled {
        tracing::info!("GitHub webhook handler disabled in configuration");
        return None;
    }
    let wh_cfg = &config.gateway.webhooks.github;
    let secret = match std::env::var(&wh_cfg.secret_env) {
        Ok(s) if !s.is_empty() => s,
        _ => {
            tracing::warn!(
                "GitHub webhook handler disabled: env var '{}' not set or empty",
                wh_cfg.secret_env
            );
            return None;
        }
    };
    let producer = Arc::new(RedpandaProducer::new_logging());
    let kaya_url = format!(
        "redis://{}:{}/{}",
        config.kaya.host, config.kaya.port, config.kaya.db,
    );
    let kaya_client = match redis::Client::open(kaya_url.as_str()) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("GitHub webhook handler disabled: KAYA client build failed: {}", e);
            return None;
        }
    };
    let handler = GithubWebhookHandler::new(
        secret.into_bytes(),
        producer,
        kaya_client,
        wh_cfg.topic.clone(),
        wh_cfg.rate_limit_per_ip_per_min,
    );
    tracing::info!(topic = %wh_cfg.topic, "GitHub webhook handler enabled");
    Some(Arc::new(handler))
}

async fn handle_github_webhook_inner(
    state: &Arc<GatewayState>,
    headers: &HashMap<String, String>,
    body_bytes: &Bytes,
    peer_addr: &SocketAddr,
) -> HttpResponse {
    let handler = match &state.github_webhook {
        Some(h) => h.clone(),
        None => {
            tracing::warn!("GitHub webhook request received but handler is disabled");
            return error_response(503, "webhook_disabled", "Webhook handler not configured");
        }
    };

    let http_req = HttpRequest {
        method: "POST".to_string(),
        uri: "/webhooks/github".to_string(),
        path: "/webhooks/github".to_string(),
        query: None,
        headers: headers.clone(),
        body: if body_bytes.is_empty() { None } else { Some(body_bytes.to_vec()) },
        version: HttpVersion::Http11,
    };
    let source_ip = peer_addr.ip().to_string();

    match handler.handle(&http_req, &source_ip).await {
        Ok(wh_resp) => {
            tracing::info!(source_ip = %source_ip, status = wh_resp.status, "GitHub webhook handled");
            let mut h = HashMap::new();
            h.insert("content-type".to_string(), "application/json".to_string());
            HttpResponse {
                status: wh_resp.status,
                headers: h,
                body: Some(wh_resp.body.into_bytes()),
            }
        }
        Err(wh_err) => {
            use armageddon_forge::webhooks::WebhookError;
            let (status, error_key) = match &wh_err {
                WebhookError::BodyTooLarge { .. } => (413u16, "payload_too_large"),
                WebhookError::MissingHeader(_) => (400, "missing_header"),
                WebhookError::InvalidSignature => (401, "invalid_signature"),
                WebhookError::UnsupportedEvent(_) => (400, "unsupported_event"),
                WebhookError::InvalidJson(_) => (400, "invalid_json"),
                WebhookError::DuplicateDelivery(_) => (200, "duplicate"),
                WebhookError::RateLimitExceeded { .. } => (429, "rate_limit_exceeded"),
                WebhookError::Kaya(_) | WebhookError::Redpanda(_) => (500, "internal_error"),
            };
            error_response(status, error_key, &wh_err.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

/// Build a JSON error `HttpResponse`.
fn error_response(status: u16, error_key: &str, message: &str) -> HttpResponse {
    let body = serde_json::json!({
        "error": error_key,
        "message": message,
        "gateway": "ARMAGEDDON"
    })
    .to_string()
    .into_bytes();

    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());
    HttpResponse {
        status,
        headers,
        body: Some(body),
    }
}

/// Convert an `HttpResponse` (internal type) to a `hyper::Response<Full<Bytes>>`.
fn build_hyper_response(
    resp: HttpResponse,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let mut builder = Response::builder().status(resp.status);
    for (name, value) in &resp.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    let body = resp.body.map(Bytes::from).unwrap_or_default();
    Ok(builder
        .body(Full::new(body))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Internal Error")))))
}
