//! KAYA Server: main binary tying all crates together.
//!
//! Starts the RESP3 TCP server, initializes the store, streams, scripting,
//! and observability subsystems.

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use tokio::signal;

use kaya_commands::{CommandContext, CommandRouter};
use kaya_network::{RequestHandler, ServerConfig, TcpServer};
use kaya_observe::ObserveConfig;
use kaya_protocol::{Command, Frame};
use kaya_scripting::{ScriptConfig, ScriptEngine};
use kaya_store::{BloomManager, Store, StoreConfig};
use kaya_streams::StreamManager;

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "kaya-server", version, about = "KAYA distributed in-memory database")]
struct Args {
    /// Path to configuration file.
    #[arg(short, long, default_value = "config/default.yaml")]
    config: String,

    /// Override bind address.
    #[arg(long)]
    bind: Option<String>,

    /// Override RESP port.
    #[arg(long)]
    port: Option<u16>,

    /// Override log level.
    #[arg(long)]
    log_level: Option<String>,

    /// Set a password for AUTH.
    #[arg(long)]
    password: Option<String>,
}

// ---------------------------------------------------------------------------
// Full config (deserialized from YAML)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct KayaConfig {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    store: StoreConfig,
    #[serde(default)]
    compression: kaya_compress::CompressConfig,
    #[serde(default)]
    observe: ObserveConfig,
    #[serde(default)]
    scripting: ScriptConfig,
}

impl Default for KayaConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            store: StoreConfig::default(),
            compression: kaya_compress::CompressConfig::default(),
            observe: ObserveConfig::default(),
            scripting: ScriptConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Request handler bridging network -> commands
// ---------------------------------------------------------------------------

struct KayaHandler {
    router: CommandRouter,
}

impl RequestHandler for KayaHandler {
    fn handle_command(&self, cmd: Command) -> Frame {
        self.router.execute(&cmd)
    }

    fn handle_multi(&self, commands: &[Command]) -> Frame {
        self.router.execute_multi(commands)
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration.
    let config: KayaConfig = if std::path::Path::new(&args.config).exists() {
        let content = std::fs::read_to_string(&args.config)?;
        serde_yaml::from_str(&content)?
    } else {
        tracing::warn!(path = %args.config, "config file not found, using defaults");
        KayaConfig::default()
    };

    // Initialize logging.
    let observe_config = ObserveConfig {
        log_level: args.log_level.unwrap_or(config.observe.log_level.clone()),
        ..config.observe.clone()
    };
    if let Err(e) = kaya_observe::init_logging(&observe_config) {
        eprintln!("failed to init logging: {e}");
    }

    tracing::info!("starting KAYA v{}", env!("CARGO_PKG_VERSION"));

    // Initialize store.
    let store = Arc::new(Store::new(config.store, config.compression));
    tracing::info!(shards = store.num_shards(), "store initialized");

    // Initialize streams.
    let streams = Arc::new(StreamManager::default());

    // Initialize bloom filters.
    let blooms = Arc::new(BloomManager::new());

    // Initialize scripting engine.
    let scripting = if config.scripting.rhai_enabled {
        let engine = ScriptEngine::new(config.scripting.clone(), store.clone());
        tracing::info!("Rhai scripting engine initialized");
        Some(Arc::new(engine))
    } else {
        None
    };

    // Determine password.
    let password = args.password.or(config.server.password.clone());

    // Initialize command context and router.
    let mut ctx = CommandContext::new(store.clone(), streams.clone(), blooms);
    ctx = ctx.with_password(password.clone());
    if let Some(ref engine) = scripting {
        ctx = ctx.with_scripting(engine.clone());
    }
    let ctx = Arc::new(ctx);
    let router = CommandRouter::new(ctx);
    let handler = Arc::new(KayaHandler { router });

    // Server config with CLI overrides.
    let server_config = ServerConfig {
        bind: args.bind.unwrap_or(config.server.bind),
        resp_port: args.port.unwrap_or(config.server.resp_port),
        password,
        ..config.server
    };

    // Start TCP server.
    let server = TcpServer::new(server_config);
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    // Spawn eviction task (active expiration -- scans shards every second).
    let evict_store = store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            evict_store.run_eviction();
        }
    });

    // Run server until shutdown signal.
    tokio::select! {
        result = server.run(handler, shutdown_rx) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "server error");
            }
        }
        _ = signal::ctrl_c() => {
            tracing::info!("received shutdown signal");
            let _ = shutdown_tx.send(());
        }
    }

    tracing::info!("KAYA server stopped");
    Ok(())
}
