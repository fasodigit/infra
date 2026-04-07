//! KAYA Network: TCP server with TLS, connection handling, pipeline support.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::BytesMut;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use kaya_protocol::{Command, Decoder, Encoder, Frame, ProtocolError};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("connection closed")]
    ConnectionClosed,

    #[error("server shutdown")]
    Shutdown,
}

// ---------------------------------------------------------------------------
// Server configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub resp_port: u16,
    pub grpc_port: u16,
    pub max_connections: usize,
    pub backlog: u32,
    pub timeout: u64,
    pub pipeline_max: usize,
    #[serde(default)]
    pub password: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".into(),
            resp_port: 6380,
            grpc_port: 6381,
            max_connections: 10_000,
            backlog: 1024,
            timeout: 300,
            pipeline_max: 1000,
            password: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Connection handler trait
// ---------------------------------------------------------------------------

/// Trait implemented by the command layer to handle incoming commands.
pub trait RequestHandler: Send + Sync + 'static {
    /// Process a single command and return the response frame.
    fn handle_command(&self, cmd: Command) -> Frame;

    /// Process a MULTI/EXEC batch of commands and return an array of responses.
    fn handle_multi(&self, commands: &[Command]) -> Frame;
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

/// Per-connection state for MULTI/EXEC transactions.
enum TxState {
    /// Normal mode -- no transaction in progress.
    None,
    /// MULTI has been issued; commands are being queued.
    Queued(Vec<Command>),
}

/// Represents a single client connection.
pub struct Connection {
    stream: TcpStream,
    addr: SocketAddr,
    read_buf: BytesMut,
    write_buf: BytesMut,
    pipeline_max: usize,
    /// Whether the client has authenticated (only relevant if password is set).
    authenticated: bool,
    /// Password required for this connection (None means no auth required).
    password: Option<String>,
    /// MULTI/EXEC transaction state.
    tx_state: TxState,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        addr: SocketAddr,
        pipeline_max: usize,
        password: Option<String>,
    ) -> Self {
        let authenticated = password.is_none();
        Self {
            stream,
            addr,
            read_buf: BytesMut::with_capacity(4096),
            write_buf: BytesMut::with_capacity(4096),
            pipeline_max,
            authenticated,
            password,
            tx_state: TxState::None,
        }
    }

    /// Run the connection loop: read frames, dispatch to handler, write responses.
    pub async fn run<H: RequestHandler>(
        &mut self,
        handler: Arc<H>,
    ) -> Result<(), NetworkError> {
        loop {
            // Read data from socket.
            let n = self.stream.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(NetworkError::ConnectionClosed);
            }

            // Parse and handle all complete frames (pipelining).
            let mut commands_in_batch = 0;
            loop {
                if commands_in_batch >= self.pipeline_max {
                    break;
                }

                match Decoder::decode(&mut self.read_buf) {
                    Ok(frame) => {
                        match Command::from_frame(frame) {
                            Ok(cmd) => {
                                let response = self.handle_cmd(&handler, cmd);
                                Encoder::encode(&response, &mut self.write_buf);
                            }
                            Err(e) => {
                                let err_frame = Frame::err(format!("ERR {e}"));
                                Encoder::encode(&err_frame, &mut self.write_buf);
                            }
                        }
                        commands_in_batch += 1;
                    }
                    Err(ProtocolError::Incomplete) => break,
                    Err(e) => {
                        let err_frame = Frame::err(format!("ERR {e}"));
                        Encoder::encode(&err_frame, &mut self.write_buf);
                        break;
                    }
                }
            }

            // Flush write buffer.
            if !self.write_buf.is_empty() {
                self.stream.write_all(&self.write_buf).await?;
                self.write_buf.clear();
            }
        }
    }

    /// Handle a single command, managing AUTH and MULTI/EXEC state.
    fn handle_cmd<H: RequestHandler>(
        &mut self,
        handler: &Arc<H>,
        cmd: Command,
    ) -> Frame {
        // AUTH is always allowed regardless of authentication state.
        if cmd.name == "AUTH" {
            return self.handle_auth(&cmd);
        }

        // If not authenticated, reject everything except AUTH and QUIT.
        if !self.authenticated && cmd.name != "QUIT" {
            return Frame::Error("NOAUTH Authentication required".into());
        }

        // MULTI/EXEC/DISCARD handling
        match cmd.name.as_str() {
            "MULTI" => {
                self.tx_state = TxState::Queued(Vec::new());
                return Frame::ok();
            }
            "EXEC" => {
                match std::mem::replace(&mut self.tx_state, TxState::None) {
                    TxState::Queued(commands) => {
                        return handler.handle_multi(&commands);
                    }
                    TxState::None => {
                        return Frame::err("ERR EXEC without MULTI");
                    }
                }
            }
            "DISCARD" => {
                match &self.tx_state {
                    TxState::Queued(_) => {
                        self.tx_state = TxState::None;
                        return Frame::ok();
                    }
                    TxState::None => {
                        return Frame::err("ERR DISCARD without MULTI");
                    }
                }
            }
            _ => {}
        }

        // If in MULTI mode, queue the command
        if let TxState::Queued(ref mut queue) = self.tx_state {
            queue.push(cmd);
            return Frame::SimpleString("QUEUED".into());
        }

        // Normal execution
        handler.handle_command(cmd)
    }

    /// Handle AUTH command internally.
    fn handle_auth(&mut self, cmd: &Command) -> Frame {
        match &self.password {
            None => {
                Frame::err("ERR Client sent AUTH, but no password is set")
            }
            Some(expected) => {
                if cmd.arg_count() < 1 {
                    return Frame::err("ERR wrong number of arguments for 'AUTH' command");
                }
                match cmd.arg_str(0) {
                    Ok(provided) => {
                        if provided == expected {
                            self.authenticated = true;
                            Frame::ok()
                        } else {
                            Frame::err("ERR invalid password")
                        }
                    }
                    Err(_) => Frame::err("ERR invalid password"),
                }
            }
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

// ---------------------------------------------------------------------------
// TCP Server
// ---------------------------------------------------------------------------

/// The main KAYA TCP server.
pub struct TcpServer {
    config: ServerConfig,
}

impl TcpServer {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Start listening and accepting connections.
    pub async fn run<H: RequestHandler>(
        &self,
        handler: Arc<H>,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<(), NetworkError> {
        let addr = format!("{}:{}", self.config.bind, self.config.resp_port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!(addr = %addr, "KAYA server listening");

        loop {
            tokio::select! {
                accept = listener.accept() => {
                    let (stream, addr) = accept?;
                    let handler = handler.clone();
                    let pipeline_max = self.config.pipeline_max;
                    let password = self.config.password.clone();

                    tokio::spawn(async move {
                        let mut conn = Connection::new(stream, addr, pipeline_max, password);
                        tracing::debug!(peer = %addr, "new connection");
                        if let Err(e) = conn.run(handler).await {
                            match e {
                                NetworkError::ConnectionClosed => {
                                    tracing::debug!(peer = %addr, "connection closed");
                                }
                                _ => {
                                    tracing::warn!(peer = %addr, error = %e, "connection error");
                                }
                            }
                        }
                    });
                }
                _ = shutdown.recv() => {
                    tracing::info!("shutting down TCP server");
                    return Ok(());
                }
            }
        }
    }

    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}
