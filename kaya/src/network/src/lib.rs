//! KAYA Network: TCP server with TLS, connection handling, pipeline support.

pub mod tracking;
pub mod client;

#[cfg(all(target_os = "linux", feature = "io_uring"))]
pub mod io_uring_backend;

#[cfg(all(target_os = "linux", feature = "io_uring"))]
pub use io_uring_backend::IoUringServer;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::BytesMut;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::Duration;

use kaya_protocol::{Command, Decoder, Encoder, Frame, ProtocolError};

// ---------------------------------------------------------------------------
// Global client ID counter
// ---------------------------------------------------------------------------

/// Monotonically incrementing counter used to assign unique IDs to new
/// connections. IDs start at 1; 0 is reserved for "no connection".
static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);

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
    /// Seconds to wait for any data on an idle connection before closing it.
    /// Prevents slow-loris style half-open connection exhaustion.
    #[serde(default = "ServerConfig::default_read_timeout_secs")]
    pub read_timeout_secs: u64,
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
            read_timeout_secs: Self::default_read_timeout_secs(),
        }
    }
}

impl ServerConfig {
    fn default_read_timeout_secs() -> u64 {
        30
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

    /// Process a command that may require per-connection session state (client
    /// ID and a push channel). The default implementation delegates to the
    /// synchronous [`handle_command`] for backward compatibility.
    ///
    /// Implementors that support Pub/Sub, CLIENT TRACKING, or FUNCTION
    /// commands should override this to route session-state commands
    /// asynchronously.
    fn handle_command_with_session(
        &self,
        cmd: Command,
        _client_id: u64,
        _push_sink: mpsc::Sender<Frame>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Frame> + Send + '_>> {
        let frame = self.handle_command(cmd);
        Box::pin(async move { frame })
    }
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
    /// Unique, monotonic ID assigned at connection creation time.
    client_id: u64,
    /// Channel for push frames (Pub/Sub, invalidations) destined for this
    /// client. The `Sender` half is given to the command layer; the `Receiver`
    /// half is drained inside the read loop via `tokio::select!`.
    push_tx: mpsc::Sender<Frame>,
    push_rx: mpsc::Receiver<Frame>,
    /// Seconds of inactivity before the read loop closes the connection.
    /// Mitigates slow-loris style connection exhaustion.
    read_timeout_secs: u64,
}

impl Connection {
    /// Capacity of the per-connection push frame channel.
    const PUSH_CHANNEL_CAPACITY: usize = 1024;

    pub fn new(
        stream: TcpStream,
        addr: SocketAddr,
        pipeline_max: usize,
        password: Option<String>,
        read_timeout_secs: u64,
    ) -> Self {
        let client_id = NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
        let authenticated = password.is_none();
        let (push_tx, push_rx) = mpsc::channel(Self::PUSH_CHANNEL_CAPACITY);
        Self {
            stream,
            addr,
            read_buf: BytesMut::with_capacity(4096),
            write_buf: BytesMut::with_capacity(4096),
            pipeline_max,
            authenticated,
            password,
            tx_state: TxState::None,
            client_id,
            push_tx,
            push_rx,
            read_timeout_secs,
        }
    }

    /// Return the unique ID assigned to this connection.
    pub fn client_id(&self) -> u64 {
        self.client_id
    }

    /// Return a clone of the push channel sender.
    ///
    /// The command layer stores this so it can push async frames (e.g. Pub/Sub
    /// messages, CLIENT TRACKING invalidations) directly to the client.
    pub fn push_sender(&self) -> mpsc::Sender<Frame> {
        self.push_tx.clone()
    }

    /// Run the connection loop: read frames, dispatch to handler, write responses.
    ///
    /// The loop multiplexes two event sources via `tokio::select!`:
    /// 1. Incoming command frames from the TCP socket.
    /// 2. Outbound push frames queued on the internal push channel
    ///    (Pub/Sub messages, CLIENT TRACKING invalidations, etc.).
    pub async fn run<H: RequestHandler>(
        &mut self,
        handler: Arc<H>,
    ) -> Result<(), NetworkError> {
        let push_sink = self.push_sender();

        let read_timeout = Duration::from_secs(self.read_timeout_secs);

        loop {
            tokio::select! {
                // -- inbound command path -------------------------------------
                // WHY: wrap with timeout to close slow-loris connections that
                // send partial data indefinitely (SecFinding-SLOWLORIS-READ).
                result = tokio::time::timeout(read_timeout, self.stream.read_buf(&mut self.read_buf)) => {
                    let io_result = match result {
                        Ok(r) => r,
                        Err(_elapsed) => {
                            tracing::warn!(
                                peer = %self.addr,
                                timeout_secs = self.read_timeout_secs,
                                "read timeout — closing idle connection"
                            );
                            return Err(NetworkError::ConnectionClosed);
                        }
                    };
                    let n = io_result?;
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
                                        let response = self
                                            .handle_cmd_async(&handler, cmd, push_sink.clone())
                                            .await;
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

                // -- outbound push path ---------------------------------------
                Some(push_frame) = self.push_rx.recv() => {
                    Encoder::encode(&push_frame, &mut self.write_buf);
                    // Drain any additional pending push frames without blocking.
                    while let Ok(extra) = self.push_rx.try_recv() {
                        Encoder::encode(&extra, &mut self.write_buf);
                    }
                    self.stream.write_all(&self.write_buf).await?;
                    self.write_buf.clear();
                }
            }
        }
    }

    /// Handle a single command, managing AUTH and MULTI/EXEC state.
    ///
    /// This async version preserves the connection-level state machine
    /// (AUTH, MULTI/EXEC/DISCARD) while delegating normal command execution to
    /// [`RequestHandler::handle_command_with_session`] so that session-state
    /// commands (Pub/Sub, CLIENT TRACKING, FUNCTION) can access the push
    /// channel and client ID.
    async fn handle_cmd_async<H: RequestHandler>(
        &mut self,
        handler: &Arc<H>,
        cmd: Command,
        push_sink: mpsc::Sender<Frame>,
    ) -> Frame {
        // AUTH is always allowed regardless of authentication state.
        if cmd.name == "AUTH" {
            return self.handle_auth(&cmd);
        }

        // If not authenticated, reject everything except QUIT.
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

        // If in MULTI mode, queue the command.
        if let TxState::Queued(ref mut queue) = self.tx_state {
            queue.push(cmd);
            return Frame::SimpleString("QUEUED".into());
        }

        // Normal execution — use session-aware path.
        handler
            .handle_command_with_session(cmd, self.client_id, push_sink)
            .await
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
                        // WHY: constant-time comparison prevents timing-oracle
                        // attacks on the cleartext password (SecFinding-AUTH-TIMING).
                        if provided.as_bytes().ct_eq(expected.as_bytes()).into() {
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
                    let read_timeout_secs = self.config.read_timeout_secs;

                    tokio::spawn(async move {
                        let mut conn = Connection::new(stream, addr, pipeline_max, password, read_timeout_secs);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    struct NoopHandler;
    impl RequestHandler for NoopHandler {
        fn handle_command(&self, _cmd: Command) -> Frame {
            Frame::ok()
        }
        fn handle_multi(&self, _commands: &[Command]) -> Frame {
            Frame::ok()
        }
    }

    // -- ct_eq: confirm constant-time auth rejects wrong password -------------

    #[test]
    fn constant_time_auth_rejects_wrong_password() {
        use subtle::ConstantTimeEq;
        let expected = "correct-horse-battery";
        let provided_wrong = "correct-horse-battery!";
        // Verify the invariant: byte-level comparison must be false.
        let ct: bool = provided_wrong.as_bytes().ct_eq(expected.as_bytes()).into();
        assert!(!ct, "constant-time comparison must reject differing passwords");

        let ct_equal: bool = expected.as_bytes().ct_eq(expected.as_bytes()).into();
        assert!(ct_equal, "constant-time comparison must accept matching passwords");
    }

    // -- slow-loris: read timeout closes idle connection ----------------------

    #[tokio::test]
    async fn read_timeout_closes_idle_connection() {
        // Start a real TCP listener to get an address.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Accept one connection on a background task with a 1-second timeout.
        let server_task = tokio::spawn(async move {
            let (stream, peer) = listener.accept().await.unwrap();
            let mut conn = Connection::new(
                stream,
                peer,
                100,
                None,
                1, // 1-second read timeout
            );
            let handler = Arc::new(NoopHandler);
            conn.run(handler).await
        });

        // Client connects but sends nothing — simulates slow-loris.
        let _client = tokio::net::TcpStream::connect(addr).await.unwrap();

        // The server task must complete within ~2 seconds with ConnectionClosed.
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            server_task,
        )
        .await
        .expect("server task must finish within timeout")
        .expect("task must not panic");

        assert!(
            matches!(result, Err(NetworkError::ConnectionClosed)),
            "idle connection must be closed after read timeout, got: {result:?}"
        );
    }

    // -- ServerConfig defaults ------------------------------------------------

    #[test]
    fn server_config_default_read_timeout_is_30s() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.read_timeout_secs, 30);
    }
}
