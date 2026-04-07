//! KAYA Bench: benchmarking suite for KAYA performance testing.

use std::time::Instant;

use clap::Parser;
use kaya_sdk::{ClientConfig, KayaClient};

#[derive(Parser, Debug)]
#[command(name = "kaya-bench", version, about = "KAYA benchmarking tool")]
struct Args {
    /// Server host.
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Server port.
    #[arg(short, long, default_value_t = 6380)]
    port: u16,

    /// Number of requests per benchmark.
    #[arg(short, long, default_value_t = 10_000)]
    requests: u64,

    /// Number of concurrent clients.
    #[arg(short, long, default_value_t = 4)]
    clients: u32,

    /// Key size in bytes.
    #[arg(long, default_value_t = 16)]
    key_size: usize,

    /// Value size in bytes.
    #[arg(long, default_value_t = 64)]
    value_size: usize,

    /// Benchmark type: set, get, mixed, pipeline
    #[arg(short = 't', long, default_value = "mixed")]
    bench_type: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("KAYA Benchmark");
    println!("==============");
    println!("Host: {}:{}", args.host, args.port);
    println!("Requests: {}", args.requests);
    println!("Clients: {}", args.clients);
    println!("Key size: {} bytes", args.key_size);
    println!("Value size: {} bytes", args.value_size);
    println!("Benchmark: {}", args.bench_type);
    println!();

    let config = ClientConfig {
        host: args.host,
        port: args.port,
        ..ClientConfig::default()
    };

    let requests_per_client = args.requests / args.clients as u64;
    let mut handles = Vec::new();

    let start = Instant::now();

    for client_id in 0..args.clients {
        let config = config.clone();
        let bench_type = args.bench_type.clone();
        let value_size = args.value_size;
        let key_size = args.key_size;

        handles.push(tokio::spawn(async move {
            let mut client = match KayaClient::connect(&config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("client {client_id}: connection failed: {e}");
                    return 0u64;
                }
            };

            let value: String = "x".repeat(value_size);
            let mut completed = 0u64;

            for i in 0..requests_per_client {
                let key = format!("bench:{client_id}:{i:0>width$}", width = key_size);

                let result = match bench_type.as_str() {
                    "set" => client.set(&key, &value).await.map(|_| ()),
                    "get" => client.get(&key).await.map(|_| ()),
                    "mixed" | _ => {
                        if i % 2 == 0 {
                            client.set(&key, &value).await.map(|_| ())
                        } else {
                            client.get(&key).await.map(|_| ())
                        }
                    }
                };

                if result.is_ok() {
                    completed += 1;
                }
            }

            completed
        }));
    }

    let mut total_completed = 0u64;
    for handle in handles {
        total_completed += handle.await.unwrap_or(0);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = if elapsed.as_secs_f64() > 0.0 {
        total_completed as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    println!("Results:");
    println!("  Completed: {total_completed} / {}", args.requests);
    println!("  Duration:  {:.3}s", elapsed.as_secs_f64());
    println!("  Throughput: {ops_per_sec:.0} ops/sec");
    println!(
        "  Avg latency: {:.3}ms",
        elapsed.as_secs_f64() * 1000.0 / total_completed.max(1) as f64
    );

    Ok(())
}
