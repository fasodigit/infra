//! ARMAGEDDON: Security gateway replacing Envoy for the FASO DIGITALISATION project.
//!
//! Pentagon architecture: 5 security engines running in parallel on every request,
//! coordinated by NEXUS, fronted by FORGE (the HTTP/gRPC proxy).
//!
//! Request flow:
//! 1. TCP listener accepts connection
//! 2. FORGE parses HTTP request, builds RequestContext
//! 3. Pentagon pipeline: SENTINEL + ARBITER run in parallel, then AEGIS, conditional ORACLE, AI
//! 4. NEXUS aggregates all decisions into a FinalVerdict
//! 5. If allowed: FORGE proxies to upstream via round-robin load balancing
//! 6. If blocked: return 403 with structured error
//!
//! Usage:
//!   armageddon --config config/armageddon.yaml

mod pipeline;

use anyhow::Context;
use armageddon_common::context::RequestContext;
use armageddon_common::decision::Action;
use armageddon_common::types::{AuthMode, ConnectionInfo, HttpRequest, HttpVersion, Protocol};
use armageddon_forge::cors::CorsHandler;
use armageddon_forge::jwt::JwtValidator;
use armageddon_forge::router::Router;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

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

/// Shared state passed to each request handler.
struct GatewayState {
    pipeline: pipeline::Pentagon,
    forge: armageddon_forge::ForgeServer,
    veil: armageddon_veil::Veil,
    auth_mode: AuthMode,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI args
    let args = Args::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    tracing::info!(
        "ARMAGEDDON v{} starting (Pentagon security gateway)",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration
    let config_loader = armageddon_config::ConfigLoader::from_file(&args.config_path)
        .context("failed to load configuration")?;

    let config = config_loader.get();

    tracing::info!("configuration loaded from {}", args.config_path);

    // Build FORGE proxy server
    let cors_configs: Vec<(String, armageddon_common::types::CorsConfig)> = config
        .gateway
        .cors
        .iter()
        .map(|e| (e.platform.clone(), e.config.clone()))
        .collect();

    let forge = armageddon_forge::ForgeServer::new(
        config.gateway.listeners.clone(),
        config.gateway.routes.clone(),
        config.gateway.clusters.clone(),
        config.gateway.jwt.clone(),
        config.gateway.kratos.clone(),
        cors_configs,
        config.gateway.ext_authz.clone(),
    );

    // Build VEIL
    let veil = armageddon_veil::Veil::new(config.security.veil.clone());

    // Initialize the Pentagon pipeline
    let mut pipeline = pipeline::Pentagon::new(&config)?;
    pipeline.init().await?;

    tracing::info!("Pentagon pipeline initialized -- all engines ready");

    // Start health checks
    let _health_handles = forge.start_health_checks();

    // Build shared state
    let auth_mode = config.gateway.auth_mode.clone();
    let state = Arc::new(GatewayState {
        pipeline,
        forge,
        veil,
        auth_mode,
    });

    // Determine listen address from first listener config
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

    // Bind TCP listener
    let tcp_listener = TcpListener::bind(listen_addr)
        .await
        .context(format!("failed to bind to {}", listen_addr))?;

    tracing::info!("ARMAGEDDON listening on {}", listen_addr);
    tracing::info!("ARMAGEDDON is operational");

    // Set up graceful shutdown
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
                tracing::info!("shutdown signal received, draining...");
                break;
            }
        }
    }

    tracing::info!("ARMAGEDDON shutdown complete");
    Ok(())
}

/// Handle a single HTTP request through the full ARMAGEDDON pipeline.
async fn handle_request(
    req: Request<Incoming>,
    peer_addr: SocketAddr,
    state: Arc<GatewayState>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // Extract request parts
    let (parts, body) = req.into_parts();

    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => Bytes::new(),
    };

    let method = parts.method.to_string();
    let uri = parts.uri.to_string();
    let path = parts.uri.path().to_string();
    let query = parts.uri.query().map(|q| q.to_string());

    // Build header map
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            headers.insert(name.as_str().to_lowercase(), v.to_string());
        }
    }

    // Detect protocol
    let protocol = if Router::is_grpc(&headers) {
        Protocol::Grpc
    } else if Router::is_graphql(&path) {
        Protocol::GraphQL
    } else {
        Protocol::Http
    };

    // --- CORS preflight handling ---
    if CorsHandler::is_preflight(&method, &headers) {
        if let Some(origin) = headers.get("origin").cloned() {
            if let Some(cors_headers) = state.forge.cors_handler().build_headers_for_origin(&origin) {
                let mut response = Response::builder().status(204);
                for (name, value) in &cors_headers {
                    response = response.header(name.as_str(), value.as_str());
                }
                return Ok(response
                    .body(Full::new(Bytes::new()))
                    .unwrap_or_else(|_| {
                        Response::new(Full::new(Bytes::new()))
                    }));
            }
        }
        // Preflight with no matching origin: return 403
        return Ok(Response::builder()
            .status(403)
            .body(Full::new(Bytes::from("CORS origin not allowed")))
            .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Forbidden")))));
    }

    // Build RequestContext for the Pentagon pipeline
    let http_req = HttpRequest {
        method: method.clone(),
        uri: uri.clone(),
        path: path.clone(),
        query,
        headers: headers.clone(),
        body: if body_bytes.is_empty() {
            None
        } else {
            Some(body_bytes.to_vec())
        },
        version: HttpVersion::Http11,
    };

    let conn_info = ConnectionInfo {
        client_ip: peer_addr.ip(),
        client_port: peer_addr.port(),
        server_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        server_port: 0,
        tls: None,
        ja3_fingerprint: None,
    };

    let mut ctx = RequestContext::new(http_req, conn_info, protocol);

    // --- Route matching ---
    let matched_route = state.forge.router().match_route(&method, &path, &headers);

    let (cluster_name, timeout_ms, auth_skip) = match matched_route {
        Some(route) => {
            ctx.matched_route = Some(route.name.clone());
            ctx.target_cluster = Some(route.cluster.clone());
            (route.cluster.clone(), route.timeout_ms, route.auth_skip)
        }
        None => {
            // No route matched: return 404
            tracing::debug!("no route matched for {} {}", method, path);
            return Ok(Response::builder()
                .status(404)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "not_found",
                        "message": format!("No route for {} {}", method, path),
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Not Found")))));
        }
    };

    // --- Authentication ---
    if !auth_skip {
        let auth_result = match state.auth_mode {
            AuthMode::Jwt => {
                // Extract Bearer token from Authorization header
                if let Some(auth_header) = headers.get("authorization") {
                    if let Some(token) = JwtValidator::extract_bearer(auth_header) {
                        match state.forge.jwt_validator().validate(token).await {
                            Ok(claims) => {
                                ctx.user_id = claims
                                    .get("sub")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                ctx.tenant_id = claims
                                    .get("tenant_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                ctx.user_roles = claims
                                    .get("roles")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                ctx.jwt_claims = Some(claims);
                                Ok(())
                            }
                            Err(e) => Err(format!("{}", e)),
                        }
                    } else {
                        Err("invalid Authorization header (expected Bearer token)".to_string())
                    }
                } else {
                    Err("missing Authorization header".to_string())
                }
            }
            AuthMode::Session => {
                // Extract session cookie
                if let Some(cookie_header) = headers.get("cookie") {
                    match state
                        .forge
                        .kratos_validator()
                        .validate_session(cookie_header)
                        .await
                    {
                        Ok(session) => {
                            ctx.user_id = Some(session.user_id);
                            ctx.tenant_id = session.tenant_id;
                            ctx.user_roles = session.roles;
                            Ok(())
                        }
                        Err(e) => Err(format!("{}", e)),
                    }
                } else {
                    Err("missing session cookie".to_string())
                }
            }
            AuthMode::Dual => {
                // Try JWT first, fallback to session
                let jwt_result = if let Some(auth_header) = headers.get("authorization") {
                    if let Some(token) = JwtValidator::extract_bearer(auth_header) {
                        match state.forge.jwt_validator().validate(token).await {
                            Ok(claims) => {
                                ctx.user_id = claims
                                    .get("sub")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                ctx.tenant_id = claims
                                    .get("tenant_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                ctx.user_roles = claims
                                    .get("roles")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                ctx.jwt_claims = Some(claims);
                                Some(Ok(()))
                            }
                            Err(_) => None, // JWT failed, try session
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(result) = jwt_result {
                    result
                } else if let Some(cookie_header) = headers.get("cookie") {
                    match state
                        .forge
                        .kratos_validator()
                        .validate_session(cookie_header)
                        .await
                    {
                        Ok(session) => {
                            ctx.user_id = Some(session.user_id);
                            ctx.tenant_id = session.tenant_id;
                            ctx.user_roles = session.roles;
                            Ok(())
                        }
                        Err(e) => Err(format!("{}", e)),
                    }
                } else {
                    Err("no valid authentication credentials found".to_string())
                }
            }
        };

        if let Err(reason) = auth_result {
            tracing::warn!(
                request_id = %ctx.request_id,
                path = %path,
                "auth failed: {}",
                reason,
            );
            return Ok(Response::builder()
                .status(401)
                .header("content-type", "application/json")
                .header("x-armageddon-request-id", ctx.request_id.to_string())
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "unauthorized",
                        "message": "Authentication required",
                        "request_id": ctx.request_id.to_string(),
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Unauthorized")))));
        }
    }

    // --- Pentagon security pipeline ---
    let verdict = match state.pipeline.inspect(&ctx).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Pentagon pipeline error: {}", e);
            // Fail-closed: block on pipeline error
            return Ok(Response::builder()
                .status(503)
                .header("content-type", "application/json")
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "security_pipeline_error",
                        "message": "Security inspection failed",
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| {
                    Response::new(Full::new(Bytes::from("Service Unavailable")))
                }));
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
            return Ok(Response::builder()
                .status(403)
                .header("content-type", "application/json")
                .header("x-armageddon-request-id", ctx.request_id.to_string())
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "blocked",
                        "message": "Request blocked by ARMAGEDDON security gateway",
                        "request_id": ctx.request_id.to_string(),
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Forbidden")))));
        }
        Action::Challenge => {
            tracing::info!(
                request_id = %ctx.request_id,
                score = verdict.score,
                "CHALLENGE: {}",
                verdict.reason,
            );
            return Ok(Response::builder()
                .status(429)
                .header("content-type", "application/json")
                .header("x-armageddon-request-id", ctx.request_id.to_string())
                .header("retry-after", "30")
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "challenge_required",
                        "message": "Verification required",
                        "request_id": ctx.request_id.to_string(),
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Too Many Requests")))));
        }
        Action::Throttle => {
            tracing::info!(
                request_id = %ctx.request_id,
                "THROTTLE: {}",
                verdict.reason,
            );
            // Continue to proxy but log the throttle decision
        }
        Action::LogOnly => {
            tracing::debug!(
                request_id = %ctx.request_id,
                "LOG: {}",
                verdict.reason,
            );
            // Continue to proxy
        }
        Action::Forward => {
            // All clear
        }
    }

    // --- Proxy to upstream ---
    let endpoint = match state.forge.select_upstream(&cluster_name) {
        Some(ep) => ep,
        None => {
            tracing::error!("no healthy upstream for cluster '{}'", cluster_name);
            return Ok(Response::builder()
                .status(503)
                .header("content-type", "application/json")
                .header("x-armageddon-request-id", ctx.request_id.to_string())
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "no_upstream",
                        "message": format!("No healthy upstream for cluster '{}'", cluster_name),
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| {
                    Response::new(Full::new(Bytes::from("Service Unavailable")))
                }));
        }
    };

    // Record request start on circuit breaker
    if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
        breaker.on_request_start();
    }

    // Forward the request -- inject identity headers before proxying
    let mut header_pairs: Vec<(String, String)> = headers.into_iter().collect();
    armageddon_veil::Veil::inject_identity_headers(&mut header_pairs, &ctx);

    let body_option = if body_bytes.is_empty() {
        None
    } else {
        Some(body_bytes)
    };

    let proxy_result = armageddon_forge::proxy::forward_request(
        &endpoint,
        &method,
        &path,
        &header_pairs,
        body_option,
        timeout_ms,
    )
    .await;

    // Record result on circuit breaker
    if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
        breaker.on_request_end();
    }

    match proxy_result {
        Ok(proxy_resp) => {
            // Record success
            if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
                breaker.record_success();
            }

            // Build response
            let mut builder = Response::builder().status(proxy_resp.status);

            // Apply upstream response headers
            let mut response_headers = proxy_resp.headers;

            // Apply VEIL: remove sensitive headers, inject security headers
            state.veil.process_response_headers(&mut response_headers);

            for (name, value) in &response_headers {
                builder = builder.header(name.as_str(), value.as_str());
            }

            // Add ARMAGEDDON headers
            builder = builder.header("x-armageddon-request-id", ctx.request_id.to_string());

            // Add CORS headers if origin was present
            if let Some(origin) = header_pairs.iter().find(|(k, _)| k == "origin").map(|(_, v)| v.clone()) {
                if let Some(cors_headers) = state.forge.cors_handler().build_headers_for_origin(&origin) {
                    for (name, value) in &cors_headers {
                        builder = builder.header(name.as_str(), value.as_str());
                    }
                }
            }

            Ok(builder
                .body(Full::new(proxy_resp.body))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Internal Error")))))
        }
        Err(e) => {
            // Record failure
            if let Some(breaker) = state.forge.circuit_breakers().get(&cluster_name) {
                breaker.record_failure();
            }

            tracing::error!(
                request_id = %ctx.request_id,
                cluster = %cluster_name,
                upstream = %format!("{}:{}", endpoint.address, endpoint.port),
                "upstream error: {}",
                e,
            );

            Ok(Response::builder()
                .status(502)
                .header("content-type", "application/json")
                .header("x-armageddon-request-id", ctx.request_id.to_string())
                .body(Full::new(Bytes::from(
                    serde_json::json!({
                        "error": "upstream_error",
                        "message": "Bad gateway",
                        "gateway": "ARMAGEDDON"
                    })
                    .to_string(),
                )))
                .unwrap_or_else(|_| Response::new(Full::new(Bytes::from("Bad Gateway")))))
        }
    }
}
