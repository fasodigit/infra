//! Comparative benchmark: standard TcpServer vs IoUringServer.
//!
//! # Usage
//!
//! ```text
//! # Standard tokio backend only (always available):
//! cargo run --bin kaya-bench-uring --release
//!
//! # Enable io_uring comparison (Linux >= 5.13 required):
//! cargo run --bin kaya-bench-uring --release --features kaya-network/io_uring
//! ```
//!
//! The benchmark sends N PING commands over a loopback TCP connection and
//! prints ops/sec for each backend side-by-side.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use kaya_network::{NetworkError, RequestHandler, ServerConfig, TcpServer};
use kaya_protocol::{Command, Encoder, Frame};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "kaya-bench-uring",
    about = "Comparative benchmark: standard tokio vs io_uring network backend"
)]
struct Args {
    /// Number of PING commands to send per backend.
    #[arg(short, long, default_value_t = 100_000)]
    requests: u64,
}

// ---------------------------------------------------------------------------
// Minimal stub handler (PING -> PONG)
// ---------------------------------------------------------------------------

struct PingHandler;

impl RequestHandler for PingHandler {
    fn handle_command(&self, cmd: Command) -> Frame {
        if cmd.name == "PING" {
            Frame::SimpleString("PONG".into())
        } else {
            Frame::err(format!("ERR unknown command '{}'", cmd.name))
        }
    }

    fn handle_multi(&self, commands: &[Command]) -> Frame {
        let responses: Vec<Frame> = commands
            .iter()
            .map(|c| self.handle_command(c.clone()))
            .collect();
        Frame::Array(responses)
    }
}

// ---------------------------------------------------------------------------
// RESP3-encoded PING frame (reused across all sends)
// ---------------------------------------------------------------------------

fn ping_bytes() -> Vec<u8> {
    let frame = Frame::Array(vec![Frame::bulk(bytes::Bytes::from_static(b"PING"))]);
    let mut buf = bytes::BytesMut::new();
    Encoder::encode(&frame, &mut buf);
    buf.to_vec()
}

// Expected PONG response: "+PONG\r\n"
const PONG_RESPONSE: &[u8] = b"+PONG\r\n";

// ---------------------------------------------------------------------------
// Benchmark a listening server at `addr` with N PING requests
// ---------------------------------------------------------------------------

async fn run_bench(addr: SocketAddr, requests: u64) -> Result<f64> {
    let ping = ping_bytes();

    // Wait for the server to be ready (simple retry loop).
    let mut stream = {
        let mut attempts = 0u32;
        loop {
            match TcpStream::connect(addr).await {
                Ok(s) => break s,
                Err(_) if attempts < 20 => {
                    attempts += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    };

    let mut response_buf = vec![0u8; PONG_RESPONSE.len()];

    let start = Instant::now();

    for _ in 0..requests {
        stream.write_all(&ping).await?;
        stream.read_exact(&mut response_buf).await?;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let ops_per_sec = requests as f64 / elapsed;
    Ok(ops_per_sec)
}

// ---------------------------------------------------------------------------
// Standard tokio backend benchmark
// ---------------------------------------------------------------------------

async fn bench_standard(port: u16, requests: u64) -> Result<f64> {
    let cfg = ServerConfig {
        bind: "127.0.0.1".into(),
        resp_port: port,
        ..ServerConfig::default()
    };

    let handler = Arc::new(PingHandler);
    let server = TcpServer::new(cfg);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    let addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;

    // Spawn server task.
    let server_handle = {
        let h = handler.clone();
        tokio::spawn(async move {
            if let Err(e) = server.run(h, shutdown_rx).await {
                match e {
                    NetworkError::Shutdown => {}
                    _ => tracing::error!(error = %e, "bench standard server error"),
                }
            }
        })
    };

    let ops = run_bench(addr, requests).await?;

    // Shutdown cleanly.
    let _ = shutdown_tx.send(());
    server_handle.abort();

    Ok(ops)
}

// ---------------------------------------------------------------------------
// io_uring backend benchmark (Linux + feature gate)
// ---------------------------------------------------------------------------

#[cfg(all(target_os = "linux", feature = "io_uring"))]
fn bench_io_uring(port: u16, requests: u64) -> Result<f64> {
    use kaya_network::IoUringServer;

    let cfg = ServerConfig {
        bind: "127.0.0.1".into(),
        resp_port: port,
        ..ServerConfig::default()
    };

    let handler = Arc::new(PingHandler);
    let server = IoUringServer::new(cfg);

    // io_uring server runs its own executor; start it on a dedicated OS thread.
    let h = handler.clone();
    let server_thread = std::thread::spawn(move || {
        server.run(h).expect("io_uring server failed");
    });

    // Measure from inside a fresh tokio runtime.
    let ops = tokio::runtime::Runtime::new()?
        .block_on(run_bench(format!("127.0.0.1:{port}").parse()?, requests))?;

    // The server thread loops forever; just detach it — the process will exit.
    drop(server_thread);

    Ok(ops)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("KAYA network backend benchmark");
    println!("================================");
    println!("Requests per backend: {}", args.requests);
    println!();

    // -- Standard tokio backend -----------------------------------------------
    print!("Standard tokio TcpServer ... ");
    let std_ops = bench_standard(16380, args.requests).await?;
    println!("{std_ops:.0} ops/sec");

    // -- io_uring backend (Linux + feature gate) ------------------------------
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    {
        print!("io_uring IoUringServer     ... ");
        // Give the OS a moment to release port 16380.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        match bench_io_uring(16381, args.requests) {
            Ok(uring_ops) => {
                println!("{uring_ops:.0} ops/sec");
                let ratio = uring_ops / std_ops;
                println!();
                println!("io_uring speedup: {ratio:.2}x");
            }
            Err(e) => {
                println!("SKIPPED ({e})");
            }
        }
    }

    #[cfg(not(all(target_os = "linux", feature = "io_uring")))]
    {
        println!("io_uring IoUringServer     ... SKIPPED (build without --features io_uring or non-Linux)");
        println!();
        println!("Standard baseline: {std_ops:.0} ops/sec");
    }

    Ok(())
}
