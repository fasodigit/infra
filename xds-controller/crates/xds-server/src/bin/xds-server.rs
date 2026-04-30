// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// xds-server entry point.
//
// Loads the YAML config file, instantiates an empty ConfigStore (KAYA
// is plugged via the discovery backends after boot), and runs the gRPC
// server that ARMAGEDDON connects to via xDS.
//
// Usage:
//   xds-server --config /etc/xds-controller/xds-controller.yaml

use std::path::PathBuf;
use clap::Parser;
use xds_server::{ServerConfig, XdsServer};
use xds_store::ConfigStore;

#[derive(Parser, Debug)]
#[command(name = "xds-server", about = "FASO xDS Controller — gRPC ADS server for ARMAGEDDON")]
struct Cli {
    /// Path to the YAML configuration file.
    #[arg(long, default_value = "/etc/xds-controller/xds-controller.yaml")]
    config: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
struct FileConfig {
    server: ServerConfig,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,xds_server=debug")),
        )
        .json()
        .init();

    let cli = Cli::parse();
    let raw = std::fs::read_to_string(&cli.config)
        .map_err(|e| format!("failed to read {}: {e}", cli.config.display()))?;
    let file_cfg: FileConfig = serde_yaml::from_str(&raw)
        .map_err(|e| format!("failed to parse {}: {e}", cli.config.display()))?;

    tracing::info!(
        addr = %file_cfg.server.listen_addr,
        port = file_cfg.server.listen_port,
        cp_id = %file_cfg.server.control_plane_id,
        "xds-server starting",
    );

    let store = ConfigStore::new();
    let server = XdsServer::new(file_cfg.server, store);

    server.run().await
}
