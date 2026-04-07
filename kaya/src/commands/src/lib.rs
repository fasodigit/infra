//! KAYA Commands: Command router and handler implementations.
//!
//! Routes parsed RESP3 commands to the appropriate store/stream operations
//! and produces RESP3 response frames.

pub mod handler;
pub mod router;

use std::sync::Arc;

use thiserror::Error;

use kaya_protocol::Frame;
use kaya_scripting::ScriptEngine;
use kaya_store::{Store, BloomManager};
use kaya_streams::StreamManager;

pub use handler::CommandHandler;
pub use router::CommandRouter;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    #[error("wrong number of arguments for '{command}' command")]
    WrongArity { command: String },

    #[error("store error: {0}")]
    Store(#[from] kaya_store::StoreError),

    #[error("stream error: {0}")]
    Stream(#[from] kaya_streams::StreamError),

    #[error("protocol error: {0}")]
    Protocol(#[from] kaya_protocol::ProtocolError),

    #[error("syntax error: {0}")]
    Syntax(String),

    #[error("NOAUTH Authentication required")]
    AuthRequired,

    #[error("script error: {0}")]
    Script(String),
}

impl CommandError {
    pub fn to_frame(&self) -> Frame {
        match self {
            CommandError::AuthRequired => Frame::Error("NOAUTH Authentication required".into()),
            _ => Frame::Error(format!("ERR {self}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Command context: shared state passed to every command handler
// ---------------------------------------------------------------------------

/// Shared state available to all command handlers.
pub struct CommandContext {
    pub store: Arc<Store>,
    pub streams: Arc<StreamManager>,
    pub blooms: Arc<BloomManager>,
    pub scripting: Option<Arc<ScriptEngine>>,
    /// Optional password for AUTH. If None, no authentication is required.
    pub password: Option<String>,
}

impl CommandContext {
    pub fn new(
        store: Arc<Store>,
        streams: Arc<StreamManager>,
        blooms: Arc<BloomManager>,
    ) -> Self {
        Self {
            store,
            streams,
            blooms,
            scripting: None,
            password: None,
        }
    }

    pub fn with_scripting(mut self, engine: Arc<ScriptEngine>) -> Self {
        self.scripting = Some(engine);
        self
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.password = password;
        self
    }
}
