// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Minimal hyper bench server for the `pingora_vs_hyper.sh` harness.
//!
//! Serves three endpoints:
//!
//! | Path       | Response |
//! |------------|----------|
//! | `/healthz` | `200 OK` "ok" |
//! | `/echo`    | `200 OK` `{"method":"…","path":"…"}` |
//! | `/slow`    | 100 ms delay then `200 OK` "slow" |
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin hyper_bench_server -- --port 8080 --workers 4
//! ```

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use tokio::net::TcpListener;
use tracing::info;

fn main() {
    // ── CLI parsing ──────────────────────────────────────────────────────────
    let args: Vec<String> = std::env::args().collect();
    let port = parse_arg(&args, "--port", 8080u16);
    let workers = parse_arg(&args, "--workers", 4usize);

    // ── Logging ──────────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("armageddon_forge=info".parse().unwrap()),
        )
        .init();

    info!(port, workers, "hyper_bench_server starting");

    // ── Runtime ──────────────────────────────────────────────────────────────
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .thread_name("hyper-bench-worker")
        .build()
        .expect("failed to build tokio runtime");

    rt.block_on(run_server(port));
}

async fn run_server(port: u16) {
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("invalid addr");
    let listener = TcpListener::bind(addr).await.expect("bind failed");
    info!(addr = %addr, "hyper_bench_server listening");

    loop {
        let (tcp, remote_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(error = %e, "accept error");
                continue;
            }
        };

        let io = TokioIo::new(tcp);

        tokio::spawn(async move {
            let svc = service_fn(|req: Request<Incoming>| async move {
                Ok::<_, Infallible>(handle(req).await)
            });

            if let Err(e) = AutoBuilder::new(TokioExecutor::new())
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!(error = %e, remote = %remote_addr, "connection error");
            }
        });
    }
}

async fn handle(req: Request<Incoming>) -> Response<Full<Bytes>> {
    let path = req.uri().path().to_string();
    let method = req.method().to_string();

    match path.as_str() {
        "/healthz" => text_response(200, "ok"),
        "/echo" => {
            let body = format!(r#"{{"method":"{method}","path":"{path}"}}"#);
            text_response(200, &body)
        }
        "/slow" => {
            tokio::time::sleep(Duration::from_millis(100)).await;
            text_response(200, "slow")
        }
        _ => text_response(404, "not found"),
    }
}

fn text_response(status: u16, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header("content-type", "text/plain")
        .body(Full::new(Bytes::copy_from_slice(body.as_bytes())))
        .expect("response build failed")
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
