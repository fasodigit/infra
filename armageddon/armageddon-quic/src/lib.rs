// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! HTTP/3 QUIC listener for ARMAGEDDON gateway.
//!
//! Exposes [`Http3Server`] which binds a UDP endpoint, performs a TLS 1.3 +
//! QUIC handshake with ALPN `h3`, and forwards decoded HTTP/3 frames to any
//! [`RequestHandler`] implementation.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use armageddon_quic::{Http3Server, QuicListenerConfig, RequestHandler};
//! use armageddon_common::types::HttpRequest;
//! use tokio::sync::broadcast;
//!
//! struct Echo;
//!
//! #[async_trait::async_trait]
//! impl RequestHandler for Echo {
//!     async fn handle(&self, req: HttpRequest)
//!         -> Result<armageddon_common::types::HttpResponse,
//!                   armageddon_quic::QuicError>
//!     {
//!         Ok(armageddon_common::types::HttpResponse {
//!             status: 200,
//!             headers: Default::default(),
//!             body: req.body,
//!         })
//!     }
//! }
//!
//! # async fn run() -> Result<(), armageddon_quic::QuicError> {
//! let cfg = QuicListenerConfig {
//!     address: "0.0.0.0".into(),
//!     port: 4433,
//!     cert_path: "/tls/server.crt".into(),
//!     key_path: "/tls/server.key".into(),
//!     max_concurrent_streams: 100,
//! };
//! let (tx, rx) = broadcast::channel(1);
//! Http3Server::new(cfg).await?.run(Arc::new(Echo), rx).await
//! # }
//! ```

pub mod codec;
pub mod server;

pub use server::{Http3Server, QuicListenerConfig, QuicError, RequestHandler};
