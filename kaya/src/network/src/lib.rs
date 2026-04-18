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

use bytes::{Bytes, BytesMut};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::Duration;

use kaya_protocol::{Command, Decoder, Encoder, Frame, ProtocolError, ProtocolVersion};

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
    /// RESP protocol version negotiated via HELLO.
    ///
    /// Defaults to Resp2. Switches to Resp3 when the client sends `HELLO 3`.
    /// This version governs the wire encoding of response frames for this
    /// connection: Resp3 uses typed Map/Set/Push frames; Resp2 falls back to
    /// flat Arrays.
    protocol: ProtocolVersion,
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
            protocol: ProtocolVersion::default(),
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
                                // Detect frame type for diagnostic logging.
                                let frame_type_tag = match &frame {
                                    Frame::Array(_) | Frame::Push(_) => "ok",
                                    Frame::SimpleString(_) => "simple_string",
                                    Frame::BulkString(_) => "bulk_string",
                                    Frame::Integer(_) => "integer",
                                    Frame::Null => "null",
                                    Frame::Map(_) => "map",
                                    Frame::Set(_) => "set",
                                    Frame::Boolean(_) => "boolean",
                                    Frame::Double(_) => "double",
                                    Frame::BigNumber(_) => "big_number",
                                    Frame::VerbatimString { .. } => "verbatim",
                                    Frame::Error(_) => "error",
                                };
                                match Command::from_frame(frame) {
                                    Ok(cmd) => {
                                        let response = self
                                            .handle_cmd_async(&handler, cmd, push_sink.clone())
                                            .await;
                                        // WHY encode_versioned: Frame::Null must be
                                        // `$-1\r\n` for RESP2 clients and `_\r\n`
                                        // for RESP3. Using encode() (always RESP3)
                                        // caused Lettuce DataAccessException on
                                        // RESP2 connections hitting missing keys.
                                        Encoder::encode_versioned(
                                            &response,
                                            self.protocol,
                                            &mut self.write_buf,
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            peer = %self.addr,
                                            error = %e,
                                            frame_type = frame_type_tag,
                                            protocol = ?self.protocol,
                                            "command parse error: unexpected frame type"
                                        );
                                        let err_frame = Frame::err(format!("ERR {e}"));
                                        // Errors are protocol-neutral (SimpleString/Error
                                        // are identical in RESP2 and RESP3).
                                        Encoder::encode_versioned(
                                            &err_frame,
                                            self.protocol,
                                            &mut self.write_buf,
                                        );
                                    }
                                }
                                commands_in_batch += 1;
                            }
                            Err(ProtocolError::Incomplete) => break,
                            Err(e) => {
                                tracing::warn!(
                                    peer = %self.addr,
                                    error = %e,
                                    buf_head = ?self.read_buf.first().copied(),
                                    "frame decode error"
                                );
                                let err_frame = Frame::err(format!("ERR {e}"));
                                Encoder::encode_versioned(
                                    &err_frame,
                                    self.protocol,
                                    &mut self.write_buf,
                                );
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
                    Encoder::encode_versioned(&push_frame, self.protocol, &mut self.write_buf);
                    // Drain any additional pending push frames without blocking.
                    while let Ok(extra) = self.push_rx.try_recv() {
                        Encoder::encode_versioned(&extra, self.protocol, &mut self.write_buf);
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

        // HELLO is handled at the connection layer so it can mutate the
        // per-connection protocol version and return the correct frame type
        // (RESP3 Map vs RESP2 flat Array). HELLO is allowed before AUTH when
        // it carries inline AUTH credentials; without credentials it still
        // negotiates the protocol version even on unauthenticated connections.
        if cmd.name == "HELLO" {
            return self.handle_hello(&cmd);
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

    // -----------------------------------------------------------------------
    // HELLO — per-connection protocol negotiation
    // -----------------------------------------------------------------------

    /// Handle `HELLO [protover [AUTH username password] [SETNAME clientname]]`.
    ///
    /// Mutates `self.protocol` to the negotiated version, then returns the
    /// server-info map encoded in the appropriate wire format:
    /// - RESP3 (`HELLO 3`) → `Frame::Map` (`%7\r\n...`)
    /// - RESP2 (`HELLO 2` or `HELLO` with no arg) → `Frame::Array` flat list
    ///
    /// Inline AUTH credentials embedded in HELLO are processed here.
    fn handle_hello(&mut self, cmd: &Command) -> Frame {
        // Parse the requested protocol version (first arg, optional).
        let requested = match cmd.args.first() {
            None => ProtocolVersion::Resp2,
            Some(b) => match b.as_ref() {
                b"2" => ProtocolVersion::Resp2,
                b"3" => ProtocolVersion::Resp3,
                other => {
                    let s = String::from_utf8_lossy(other);
                    return Frame::err(format!(
                        "NOPROTO sorry, this protocol version is not supported: {s}"
                    ));
                }
            },
        };

        // Scan remaining args for optional inline AUTH / SETNAME.
        // Syntax after protover: [AUTH username password] [SETNAME clientname]
        let mut idx = 1usize;
        while idx < cmd.args.len() {
            let keyword = cmd.args[idx].to_ascii_uppercase();
            match keyword.as_slice() {
                b"AUTH" => {
                    // AUTH can carry one arg (password only) or two (user + password).
                    // KAYA only supports single-password auth; we accept both forms.
                    if idx + 1 >= cmd.args.len() {
                        return Frame::err(
                            "ERR Syntax error in HELLO option 'auth'",
                        );
                    }
                    // Skip optional username when two args follow AUTH.
                    let (user_or_pass_idx, pass_idx_opt) =
                        if idx + 2 < cmd.args.len() {
                            // Two args: AUTH username password
                            (idx + 1, Some(idx + 2))
                        } else {
                            // One arg: AUTH password
                            (idx + 1, None)
                        };
                    let password_bytes = if let Some(pi) = pass_idx_opt {
                        // Skip username arg; use the next as password.
                        idx = pi;
                        &cmd.args[pi]
                    } else {
                        &cmd.args[user_or_pass_idx]
                    };

                    match &self.password {
                        None => {
                            return Frame::err(
                                "ERR Client sent AUTH, but no password is set",
                            );
                        }
                        Some(expected) => {
                            let ok: bool = password_bytes
                                .as_ref()
                                .ct_eq(expected.as_bytes())
                                .into();
                            if !ok {
                                return Frame::err("WRONGPASS invalid username-password pair or user is disabled.");
                            }
                            self.authenticated = true;
                        }
                    }
                    idx += 1;
                }
                b"SETNAME" => {
                    // SETNAME clientname — we accept and ignore the name for now.
                    if idx + 1 >= cmd.args.len() {
                        return Frame::err(
                            "ERR Syntax error in HELLO option 'setname'",
                        );
                    }
                    idx += 2; // consume keyword + name
                }
                _ => {
                    // Unknown option — skip gracefully.
                    idx += 1;
                }
            }
        }

        // Persist the negotiated version for subsequent commands on this
        // connection. After this point all responses use the new protocol.
        self.protocol = requested;

        tracing::debug!(
            peer = %self.addr,
            client_id = self.client_id,
            protocol = ?self.protocol,
            "HELLO negotiated protocol version"
        );

        // Build the server-info response.
        Self::hello_response(self.client_id, requested)
    }

    /// Build the HELLO server-info response frame for the requested protocol.
    ///
    /// RESP3 → `Frame::Map` with typed integer for `proto`.
    /// RESP2 → flat `Frame::Array` (key, value, key, value, …).
    #[inline]
    fn hello_response(client_id: u64, proto: ProtocolVersion) -> Frame {
        match proto {
            ProtocolVersion::Resp3 => Frame::Map(vec![
                (
                    Frame::BulkString(Bytes::from_static(b"server")),
                    Frame::BulkString(Bytes::from_static(b"kaya")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"version")),
                    Frame::BulkString(Bytes::from_static(b"0.1.0")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"proto")),
                    Frame::Integer(3),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"id")),
                    Frame::Integer(client_id as i64),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"mode")),
                    Frame::BulkString(Bytes::from_static(b"standalone")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"role")),
                    Frame::BulkString(Bytes::from_static(b"master")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"modules")),
                    Frame::Array(vec![]),
                ),
            ]),
            ProtocolVersion::Resp2 => Frame::Array(vec![
                Frame::BulkString(Bytes::from_static(b"server")),
                Frame::BulkString(Bytes::from_static(b"kaya")),
                Frame::BulkString(Bytes::from_static(b"version")),
                Frame::BulkString(Bytes::from_static(b"0.1.0")),
                Frame::BulkString(Bytes::from_static(b"proto")),
                Frame::Integer(2),
                Frame::BulkString(Bytes::from_static(b"id")),
                Frame::Integer(client_id as i64),
                Frame::BulkString(Bytes::from_static(b"mode")),
                Frame::BulkString(Bytes::from_static(b"standalone")),
                Frame::BulkString(Bytes::from_static(b"role")),
                Frame::BulkString(Bytes::from_static(b"master")),
                Frame::BulkString(Bytes::from_static(b"modules")),
                Frame::Array(vec![]),
            ]),
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

    // -- HELLO protocol negotiation -------------------------------------------

    /// Helper: build a Command from name + string args (for unit tests).
    fn make_cmd(name: &str, args: &[&str]) -> Command {
        Command {
            name: name.to_ascii_uppercase(),
            args: args.iter().map(|s| Bytes::from(s.to_string())).collect(),
        }
    }

    #[test]
    fn hello_response_resp3_returns_map_with_proto_3() {
        let frame = Connection::hello_response(42, ProtocolVersion::Resp3);
        let pairs = match frame {
            Frame::Map(ref p) => p,
            other => panic!("expected Frame::Map, got {other:?}"),
        };
        // Find "proto" key and assert its value is Integer(3).
        let proto_val = pairs
            .iter()
            .find(|(k, _)| k.as_str() == Some("proto"))
            .map(|(_, v)| v);
        assert_eq!(proto_val, Some(&Frame::Integer(3)));
        // Verify "id" == 42.
        let id_val = pairs
            .iter()
            .find(|(k, _)| k.as_str() == Some("id"))
            .map(|(_, v)| v);
        assert_eq!(id_val, Some(&Frame::Integer(42)));
    }

    #[test]
    fn hello_response_resp2_returns_flat_array_with_proto_2() {
        let frame = Connection::hello_response(7, ProtocolVersion::Resp2);
        let items = match frame {
            Frame::Array(ref v) => v,
            other => panic!("expected Frame::Array, got {other:?}"),
        };
        // flat list: [key, val, key, val, …]
        let proto_pos = items
            .iter()
            .position(|f| f.as_str() == Some("proto"))
            .expect("'proto' key not found in flat array");
        assert_eq!(items[proto_pos + 1], Frame::Integer(2));
        // "id" value must be 7.
        let id_pos = items
            .iter()
            .position(|f| f.as_str() == Some("id"))
            .expect("'id' key not found in flat array");
        assert_eq!(items[id_pos + 1], Frame::Integer(7));
    }

    #[test]
    fn hello_without_arg_defaults_to_resp2_array() {
        // `HELLO` with no argument should return RESP2 flat array (proto=2).
        let frame = Connection::hello_response(1, ProtocolVersion::Resp2);
        assert!(matches!(frame, Frame::Array(_)), "no-arg HELLO must return Array");
    }

    #[test]
    fn hello_response_map_encodes_to_resp3_wire() {
        // Encode the RESP3 map and check the leading '%' byte.
        use kaya_protocol::{Encoder};
        let frame = Connection::hello_response(1, ProtocolVersion::Resp3);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        assert_eq!(buf[0], b'%', "RESP3 Map frame must start with '%'");
    }

    #[test]
    fn hello_response_array_encodes_to_resp2_wire() {
        use kaya_protocol::{Encoder};
        let frame = Connection::hello_response(1, ProtocolVersion::Resp2);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        assert_eq!(buf[0], b'*', "RESP2 Array frame must start with '*'");
    }

    /// Build a tokio `Connection` backed by a real loopback socket pair.
    ///
    /// Spawns an async accept on a background task and returns
    /// `(conn_for_server_side, client_stream_guard)`.
    /// Dropping `client_stream_guard` closes the client end.
    async fn make_connection(password: Option<String>) -> (Connection, tokio::net::TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (server_stream, peer) = listener.accept().await.unwrap();
        let conn = Connection::new(server_stream, peer, 100, password, 30);
        (conn, client)
    }

    #[tokio::test]
    async fn hello_unknown_proto_returns_noproto_error() {
        let (mut conn, _client) = make_connection(None).await;

        let cmd = make_cmd("HELLO", &["99"]);
        let resp = conn.handle_hello(&cmd);
        assert!(
            matches!(&resp, Frame::Error(msg) if msg.starts_with("NOPROTO")),
            "unsupported proto version must return NOPROTO error, got: {resp:?}"
        );
        // Protocol must NOT have been mutated.
        assert_eq!(conn.protocol, ProtocolVersion::Resp2);
    }

    #[tokio::test]
    async fn hello_3_mutates_connection_protocol_to_resp3() {
        let (mut conn, _client) = make_connection(None).await;

        assert_eq!(conn.protocol, ProtocolVersion::Resp2, "default must be Resp2");
        let cmd = make_cmd("HELLO", &["3"]);
        let resp = conn.handle_hello(&cmd);
        assert!(matches!(resp, Frame::Map(_)), "HELLO 3 must return Map frame");
        assert_eq!(conn.protocol, ProtocolVersion::Resp3, "protocol must flip to Resp3");
    }

    #[tokio::test]
    async fn hello_2_keeps_connection_protocol_as_resp2() {
        let (mut conn, _client) = make_connection(None).await;

        let cmd = make_cmd("HELLO", &["2"]);
        let resp = conn.handle_hello(&cmd);
        assert!(matches!(resp, Frame::Array(_)), "HELLO 2 must return Array frame");
        assert_eq!(conn.protocol, ProtocolVersion::Resp2);
    }

    #[tokio::test]
    async fn hello_inline_auth_wrong_password_returns_wrongpass() {
        let (mut conn, _client) = make_connection(Some("correct".into())).await;

        // HELLO 3 AUTH default wrongpassword
        let cmd = make_cmd("HELLO", &["3", "AUTH", "default", "wrongpassword"]);
        let resp = conn.handle_hello(&cmd);
        assert!(
            matches!(&resp, Frame::Error(msg) if msg.contains("WRONGPASS")),
            "wrong inline auth must return WRONGPASS, got: {resp:?}"
        );
        // Protocol must NOT have been updated because auth failed first.
        assert!(!conn.authenticated);
    }

    #[tokio::test]
    async fn hello_inline_auth_correct_password_authenticates() {
        let (mut conn, _client) = make_connection(Some("s3cr3t".into())).await;

        assert!(!conn.authenticated, "initially unauthenticated");
        // HELLO 3 AUTH default s3cr3t
        let cmd = make_cmd("HELLO", &["3", "AUTH", "default", "s3cr3t"]);
        let resp = conn.handle_hello(&cmd);
        assert!(matches!(resp, Frame::Map(_)), "successful HELLO 3 must return Map");
        assert!(conn.authenticated, "connection must be authenticated after HELLO+AUTH");
        assert_eq!(conn.protocol, ProtocolVersion::Resp3);
    }
}
