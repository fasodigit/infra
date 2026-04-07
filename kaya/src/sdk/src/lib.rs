//! KAYA SDK: Rust client library for connecting to a KAYA server.
//!
//! Provides a high-level async client with connection pooling and
//! automatic reconnection.

pub mod client;
pub mod pool;

use thiserror::Error;
use serde::{Deserialize, Serialize};

pub use client::KayaClient;
pub use pool::ConnectionPool;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("connection failed: {0}")]
    Connection(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(#[from] kaya_protocol::ProtocolError),

    #[error("server error: {0}")]
    Server(String),

    #[error("timeout")]
    Timeout,

    #[error("pool exhausted")]
    PoolExhausted,
}

// ---------------------------------------------------------------------------
// Client configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub database: u32,
    pub connect_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub pool_size: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 6380,
            password: None,
            database: 0,
            connect_timeout_ms: 5000,
            command_timeout_ms: 5000,
            pool_size: 4,
        }
    }
}
