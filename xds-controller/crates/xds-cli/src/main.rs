// xdsctl: CLI tool for managing xDS Controller configuration.
//
// Provides commands to manage clusters, routes, endpoints, listeners,
// and certificates stored in KAYA. Changes take effect immediately
// and are pushed to ARMAGEDDON via xDS.
//
// Usage:
//   xdsctl cluster add <name> --type eds --lb round-robin
//   xdsctl cluster list
//   xdsctl cluster remove <name>
//   xdsctl route add <name> --domain api.faso.bf --prefix / --cluster api
//   xdsctl endpoint add <cluster> --address 10.0.1.5 --port 8080
//   xdsctl snapshot show

use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "xdsctl",
    about = "CLI for the FASO xDS Controller (manages ARMAGEDDON configuration)",
    version
)]
struct Cli {
    /// xDS Controller gRPC address.
    #[arg(long, default_value = "http://127.0.0.1:18000", global = true)]
    server: String,

    /// Output format.
    #[arg(long, default_value = "table", global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Yaml,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage backend clusters.
    Cluster {
        #[command(subcommand)]
        action: ClusterAction,
    },
    /// Manage routing rules.
    Route {
        #[command(subcommand)]
        action: RouteAction,
    },
    /// Manage endpoints within clusters.
    Endpoint {
        #[command(subcommand)]
        action: EndpointAction,
    },
    /// Manage network listeners.
    Listener {
        #[command(subcommand)]
        action: ListenerAction,
    },
    /// Show the current configuration snapshot.
    Snapshot,
    /// Server health and status.
    Status,
}

#[derive(Subcommand)]
enum ClusterAction {
    /// Add or update a cluster.
    Add {
        /// Cluster name.
        name: String,
        /// Discovery type.
        #[arg(long, default_value = "eds")]
        r#type: String,
        /// Load balancing policy.
        #[arg(long, default_value = "round-robin")]
        lb: String,
        /// Connection timeout in milliseconds.
        #[arg(long, default_value = "5000")]
        timeout: u64,
    },
    /// List all clusters.
    List,
    /// Remove a cluster.
    Remove {
        /// Cluster name.
        name: String,
    },
    /// Show cluster details.
    Show {
        /// Cluster name.
        name: String,
    },
}

#[derive(Subcommand)]
enum RouteAction {
    /// Add or update a route configuration.
    Add {
        /// Route configuration name.
        name: String,
        /// Domain to match.
        #[arg(long)]
        domain: String,
        /// Path prefix to match.
        #[arg(long, default_value = "/")]
        prefix: String,
        /// Target cluster.
        #[arg(long)]
        cluster: String,
    },
    /// List all route configurations.
    List,
    /// Remove a route configuration.
    Remove {
        /// Route configuration name.
        name: String,
    },
}

#[derive(Subcommand)]
enum EndpointAction {
    /// Add an endpoint to a cluster.
    Add {
        /// Cluster name.
        cluster: String,
        /// Endpoint IP address.
        #[arg(long)]
        address: String,
        /// Endpoint port.
        #[arg(long)]
        port: u16,
        /// Load balancing weight.
        #[arg(long, default_value = "1")]
        weight: u32,
    },
    /// List endpoints for a cluster.
    List {
        /// Cluster name.
        cluster: String,
    },
}

#[derive(Subcommand)]
enum ListenerAction {
    /// Add or update a listener.
    Add {
        /// Listener name.
        name: String,
        /// Bind address.
        #[arg(long, default_value = "0.0.0.0")]
        address: String,
        /// Bind port.
        #[arg(long)]
        port: u16,
    },
    /// List all listeners.
    List,
    /// Remove a listener.
    Remove {
        /// Listener name.
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("xdsctl=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Cluster { action } => commands::cluster::handle(action, &cli.server).await?,
        Commands::Route { action } => commands::route::handle(action, &cli.server).await?,
        Commands::Endpoint { action } => commands::endpoint::handle(action, &cli.server).await?,
        Commands::Listener { action } => commands::listener::handle(action, &cli.server).await?,
        Commands::Snapshot => commands::snapshot::handle(&cli.server).await?,
        Commands::Status => commands::status::handle(&cli.server).await?,
    }

    Ok(())
}
