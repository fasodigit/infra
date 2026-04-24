// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Minimal Pingora bench server for the `pingora_vs_hyper.sh` harness.
//!
//! Serves a transparent echo gateway.  The wrk harness targets:
//!
//! | Path      | Expected |
//! |-----------|----------|
//! | `/healthz` | 200 "ok" (served by ForgeFilter short-circuit) |
//! | `/echo`    | 200 request echo (pass-through to loopback echo backend) |
//! | `/slow`    | 100 ms delay (served by ForgeFilter via tokio::time::sleep) |
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin pingora_bench_server --features pingora -- --port 8081 --workers 4
//! ```

#[cfg(not(feature = "pingora"))]
fn main() {
    eprintln!("pingora_bench_server requires the 'pingora' feature.");
    std::process::exit(1);
}

#[cfg(feature = "pingora")]
fn main() {
    use std::sync::Arc;
    use armageddon_forge::pingora::gateway::{
        PingoraGateway, PingoraGatewayConfig, UpstreamRegistry,
    };
    use armageddon_forge::pingora::server::build_server;
    use armageddon_common::types::Endpoint;

    let args: Vec<String> = std::env::args().collect();
    let port = parse_arg(&args, "--port", 8081u16);
    let _workers = parse_arg(&args, "--workers", 4usize);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("armageddon_forge=info".parse().unwrap()),
        )
        .init();

    tracing::info!(port, "pingora_bench_server starting");

    let registry = Arc::new(UpstreamRegistry::new());
    // Register a loopback echo backend (hyper_bench_server on 8080).
    // In the bench harness both servers are started before wrk runs.
    registry.update_cluster(
        "default",
        vec![Endpoint {
            address: "127.0.0.1".to_string(),
            port: 8082, // echo-backend port expected by bench harness
            weight: 1,
            healthy: true,
        }],
    );

    let cfg = PingoraGatewayConfig {
        default_cluster: "default".to_string(),
        upstream_tls: false,
        upstream_timeout_ms: 5_000,
        pool_size: 256,
        filters: vec![Arc::new(BenchFilter)],
        compression: None,
    };

    let gateway = PingoraGateway::new(cfg, registry);
    let listen_addr = format!("0.0.0.0:{port}");
    #[allow(unused_mut)]
    let mut server = build_server(gateway, &listen_addr)
        .expect("failed to build pingora bench server");
    server.run_forever();
}

// ---------------------------------------------------------------------------
// BenchFilter — handles /healthz and /slow inline; passes /echo upstream
// ---------------------------------------------------------------------------

#[cfg(feature = "pingora")]
struct BenchFilter;

#[cfg(feature = "pingora")]
#[async_trait::async_trait]
impl armageddon_forge::pingora::filters::ForgeFilter for BenchFilter {
    fn name(&self) -> &'static str {
        "bench"
    }

    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut armageddon_forge::pingora::ctx::RequestCtx,
    ) -> armageddon_forge::pingora::filters::Decision {
        use armageddon_forge::pingora::filters::Decision;

        let path = session.req_header().uri.path();

        match path {
            "/healthz" => {
                // Build a 200 response directly.
                match pingora::http::ResponseHeader::build(200, None) {
                    Ok(mut resp) => {
                        let _ = resp.insert_header("content-type", "text/plain");
                        let _ = resp.insert_header("x-bench", "pingora");
                        Decision::ShortCircuit(Box::new(resp))
                    }
                    Err(_) => Decision::Deny(500),
                }
            }
            "/slow" => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                match pingora::http::ResponseHeader::build(200, None) {
                    Ok(mut resp) => {
                        let _ = resp.insert_header("content-type", "text/plain");
                        Decision::ShortCircuit(Box::new(resp))
                    }
                    Err(_) => Decision::Deny(500),
                }
            }
            _ => {
                // /echo and everything else: pass to upstream.
                ctx.cluster = "default".to_string();
                Decision::Continue
            }
        }
    }
}

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> T
where
    T::Err: std::fmt::Debug,
{
    args.windows(2)
        .find(|w| w[0] == flag)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}
