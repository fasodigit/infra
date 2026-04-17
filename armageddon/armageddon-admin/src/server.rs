// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Admin HTTP server — loopback-only by default (127.0.0.1:9901).

use serde::{Deserialize, Serialize};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::routes;
use crate::state::AdminState;

// -- config --

/// Configuration for the admin server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    /// Bind address. Defaults to 127.0.0.1 (loopback only).
    #[serde(default = "default_bind_addr")]
    pub bind_addr: IpAddr,

    /// TCP port. Defaults to 9901.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Optional constant-time secret for `X-Admin-Token` auth.
    /// When `None`, authentication is disabled.
    pub admin_token: Option<String>,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            port: default_port(),
            admin_token: None,
        }
    }
}

fn default_bind_addr() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn default_port() -> u16 {
    9901
}

// -- server --

/// Axum-based Admin HTTP server.
pub struct AdminServer {
    /// Server configuration.
    pub config: AdminConfig,
    /// Shared mutable state.
    pub state: Arc<AdminState>,
}

impl AdminServer {
    /// Create a new server.
    pub fn new(config: AdminConfig, state: Arc<AdminState>) -> Self {
        Self { config, state }
    }

    /// Bind and serve. Exits gracefully when `shutdown` fires.
    pub async fn run(self, mut shutdown: broadcast::Receiver<()>) -> io::Result<()> {
        // Security: refuse to bind on a non-loopback address unless explicitly
        // configured. Log a warning when a non-loopback addr is chosen.
        let bind_addr = self.config.bind_addr;
        if !bind_addr.is_loopback() {
            tracing::warn!(
                addr = %bind_addr,
                "admin server is binding on a non-loopback address — \
                 ensure network-level access control is in place"
            );
        }

        let socket_addr = SocketAddr::new(bind_addr, self.config.port);
        let listener = TcpListener::bind(socket_addr).await?;
        tracing::info!(addr = %socket_addr, "admin server listening");

        let router = routes::build_router(Arc::clone(&self.state), self.config.clone());

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown.recv().await;
                tracing::info!("admin server shutting down");
            })
            .await
    }
}
