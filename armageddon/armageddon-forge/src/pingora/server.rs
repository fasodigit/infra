// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pingora [`Server`] bootstrap — wires a [`PingoraGateway`] into a
//! fully-bootstrapped `pingora_core::Server` instance.
//!
//! The returned server can be started with `server.run_forever()`.
//! Graceful restart (SIGUSR2) is provided by Pingora itself.

use pingora_core::prelude::*;
use pingora_proxy::http_proxy_service;

use crate::pingora::gateway::PingoraGateway;

/// Build a Pingora [`Server`] that listens on `listen_addr` and forwards
/// traffic through `gateway`.
///
/// # Errors
///
/// - If Pingora fails to initialise its internal state (`Server::new`).
/// - If the TCP listener cannot be bound during `proxy.add_tcp`.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "pingora")]
/// # {
/// use armageddon_forge::pingora::gateway::{PingoraGateway, PingoraGatewayConfig, UpstreamRegistry};
/// use armageddon_forge::pingora::server::build_server;
/// use std::sync::Arc;
///
/// let registry = Arc::new(UpstreamRegistry::new());
/// let gw = PingoraGateway::new(PingoraGatewayConfig::default(), registry);
/// let mut server = build_server(gw, "0.0.0.0:8080").expect("build ok");
/// // server.run_forever();  // blocking; SIGUSR2 triggers graceful restart
/// # }
/// ```
pub fn build_server(gateway: PingoraGateway, listen_addr: &str) -> anyhow::Result<Server> {
    let mut server = Server::new(None)?;
    server.bootstrap();

    let mut proxy = http_proxy_service(&server.configuration, gateway);
    proxy.add_tcp(listen_addr);

    server.add_service(proxy);
    Ok(server)
}
