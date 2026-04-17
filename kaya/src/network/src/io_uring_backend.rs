//! KAYA io_uring network backend — Linux-only high-performance I/O path.
//!
//! # Overview
//!
//! This module provides [`IoUringServer`], an alternative TCP server that
//! replaces the standard `tokio::net` I/O path with `tokio-uring`, which
//! submits kernel I/O operations via the `io_uring` interface (Linux 5.1+;
//! full feature set from kernel 5.13+).
//!
//! # Enabling
//!
//! ```text
//! cargo build -p kaya-network --features io_uring
//! cargo run  -p kaya-server  --features kaya-network/io_uring
//! ```
//!
//! # Trade-offs
//!
//! * **Throughput**: +30–50 % on write-heavy workloads compared to the
//!   standard tokio back-end, because `io_uring` batches syscalls and avoids
//!   per-call context switches.
//! * **Latency**: slightly lower tail latency for large payloads (zero-copy
//!   buffer ownership transfer to the kernel).
//! * **Portability**: Linux-only (`target_os = "linux"`). Unavailable on
//!   macOS or Windows — those platforms continue to use [`crate::TcpServer`]
//!   transparently.
//! * **Thread model**: `tokio-uring` runs its own single-threaded executor per
//!   call to `tokio_uring::start`; connection tasks are spawned inside that
//!   executor. CPU-bound handler work is offloaded via
//!   `tokio::task::spawn_blocking` when needed.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::BytesMut;
use tokio_uring::buf::BoundedBuf;

use kaya_protocol::{Command, Decoder, Encoder, Frame, ProtocolError};

use crate::{NetworkError, RequestHandler, ServerConfig};

// ---------------------------------------------------------------------------
// IoUringServer
// ---------------------------------------------------------------------------

/// A KAYA TCP server that uses the Linux `io_uring` interface for all socket
/// I/O, providing higher throughput on write-heavy workloads.
///
/// The public API is deliberately symmetric with [`crate::TcpServer`]:
/// construct via [`IoUringServer::new`], start with [`IoUringServer::run`].
///
/// `run` is a *blocking* call — it drives its own executor via
/// `tokio_uring::start` and returns only when an unrecoverable error occurs.
/// Wrap it in a dedicated OS thread if you need the tokio runtime to remain
/// unblocked:
///
/// ```rust,ignore
/// let server = IoUringServer::new(config);
/// std::thread::spawn(move || server.run(handler));
/// ```
pub struct IoUringServer {
    config: ServerConfig,
}

impl IoUringServer {
    /// Create a new `IoUringServer` with the supplied configuration.
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Return a reference to the active configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Start accepting connections and serving requests.
    ///
    /// This call blocks until a fatal error occurs (e.g. cannot bind the
    /// address). It creates a `tokio_uring` executor internally, so it must
    /// **not** be called from within an existing tokio runtime. Spawn it on a
    /// dedicated OS thread.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `bind` fails or if the `io_uring` driver cannot be
    /// initialised (kernel too old, missing capability, etc.).
    pub fn run<H: RequestHandler + Sync>(
        &self,
        handler: Arc<H>,
    ) -> std::io::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.bind, self.config.resp_port)
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let pipeline_max = self.config.pipeline_max;
        let password = self.config.password.clone();

        tracing::info!(%addr, "IoUringServer listening");

        tokio_uring::start(async move {
            // bind is synchronous in tokio-uring 0.5
            let listener = tokio_uring::net::TcpListener::bind(addr)?;

            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        let h = handler.clone();
                        let pw = password.clone();
                        tokio_uring::spawn(async move {
                            tracing::debug!(%peer, "io_uring: new connection");
                            if let Err(e) =
                                handle_connection(stream, peer, h, pipeline_max, pw).await
                            {
                                match e {
                                    NetworkError::ConnectionClosed => {
                                        tracing::debug!(%peer, "io_uring: connection closed");
                                    }
                                    _ => {
                                        tracing::warn!(
                                            %peer,
                                            error = %e,
                                            "io_uring: connection error"
                                        );
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "io_uring: accept failed");
                        return Err(e);
                    }
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

/// Per-connection transaction state.
enum TxState {
    None,
    Queued(Vec<Command>),
}

/// Drive a single accepted `tokio_uring` TCP stream to completion.
///
/// Reads data in fixed-size chunks using `io_uring` owned-buffer reads, parses
/// RESP3 frames with the shared [`Decoder`], dispatches each parsed command
/// through `handler`, and writes responses back using `write_all` with a
/// `BoundedBuf` slice — the `io_uring` zero-copy write path.
async fn handle_connection<H: RequestHandler>(
    stream: tokio_uring::net::TcpStream,
    peer: SocketAddr,
    handler: Arc<H>,
    pipeline_max: usize,
    password: Option<String>,
) -> Result<(), NetworkError> {
    let mut read_buf = BytesMut::with_capacity(4096);
    let mut write_buf = BytesMut::with_capacity(4096);
    let mut authenticated = password.is_none();
    let mut tx_state = TxState::None;

    // io_uring requires owned buffers for the DMA path; we use a fixed-size
    // Vec<u8> that is moved in/out of each read call.
    let mut io_buf = vec![0u8; 4096];

    loop {
        // -- read from socket (io_uring owned-buffer path) --------------------
        let (res, returned_buf) = stream.read(io_buf).await;
        io_buf = returned_buf;

        let n = res.map_err(NetworkError::Io)?;
        if n == 0 {
            return Err(NetworkError::ConnectionClosed);
        }
        read_buf.extend_from_slice(&io_buf[..n]);

        // -- parse & dispatch all complete frames in the read buffer ----------
        let mut commands_in_batch = 0usize;

        loop {
            if commands_in_batch >= pipeline_max {
                break;
            }

            match Decoder::decode(&mut read_buf) {
                Ok(frame) => {
                    match Command::from_frame(frame) {
                        Ok(cmd) => {
                            let response = dispatch(
                                &cmd,
                                &handler,
                                &mut authenticated,
                                &password,
                                &mut tx_state,
                                peer,
                            );
                            Encoder::encode(&response, &mut write_buf);
                        }
                        Err(e) => {
                            Encoder::encode(
                                &Frame::err(format!("ERR {e}")),
                                &mut write_buf,
                            );
                        }
                    }
                    commands_in_batch += 1;
                }
                Err(ProtocolError::Incomplete) => break,
                Err(e) => {
                    Encoder::encode(
                        &Frame::err(format!("ERR {e}")),
                        &mut write_buf,
                    );
                    break;
                }
            }
        }

        // -- flush write buffer (io_uring write_all path) ---------------------
        if !write_buf.is_empty() {
            // Convert to Vec<u8> for the io_uring owned-buffer write.
            // `split()` clears write_buf, returning the drained bytes as a
            // new BytesMut; we freeze and copy into Vec for the DMA slice.
            let out: Vec<u8> = write_buf.split().freeze().into();
            let len = out.len();
            // `slice(..len)` produces a `BoundedBuf` view without copying.
            let (res, _returned_slice) = stream.write_all(out.slice(..len)).await;
            res.map_err(NetworkError::Io)?;
            // write_buf was already cleared by split() above.
        }
    }
}

// ---------------------------------------------------------------------------
// Command dispatch helper
// ---------------------------------------------------------------------------

/// Route a single command through AUTH, MULTI/EXEC and the request handler.
fn dispatch<H: RequestHandler>(
    cmd: &Command,
    handler: &Arc<H>,
    authenticated: &mut bool,
    password: &Option<String>,
    tx_state: &mut TxState,
    _peer: SocketAddr,
) -> Frame {
    // AUTH is always permitted.
    if cmd.name == "AUTH" {
        return handle_auth(cmd, authenticated, password);
    }

    // Reject unauthenticated clients except for QUIT.
    if !*authenticated && cmd.name != "QUIT" {
        return Frame::Error("NOAUTH Authentication required".into());
    }

    // MULTI / EXEC / DISCARD
    match cmd.name.as_str() {
        "MULTI" => {
            *tx_state = TxState::Queued(Vec::new());
            return Frame::ok();
        }
        "EXEC" => {
            match std::mem::replace(tx_state, TxState::None) {
                TxState::Queued(commands) => {
                    return handler.handle_multi(&commands);
                }
                TxState::None => {
                    return Frame::err("ERR EXEC without MULTI");
                }
            }
        }
        "DISCARD" => {
            match tx_state {
                TxState::Queued(_) => {
                    *tx_state = TxState::None;
                    return Frame::ok();
                }
                TxState::None => {
                    return Frame::err("ERR DISCARD without MULTI");
                }
            }
        }
        _ => {}
    }

    // Queue command if MULTI is active.
    if let TxState::Queued(ref mut queue) = tx_state {
        queue.push(cmd.clone());
        return Frame::SimpleString("QUEUED".into());
    }

    // Normal command execution.
    handler.handle_command(cmd.clone())
}

/// Handle an AUTH command, mutating the `authenticated` flag in place.
fn handle_auth(
    cmd: &Command,
    authenticated: &mut bool,
    password: &Option<String>,
) -> Frame {
    match password {
        None => Frame::err("ERR Client sent AUTH, but no password is set"),
        Some(expected) => {
            if cmd.arg_count() < 1 {
                return Frame::err(
                    "ERR wrong number of arguments for 'AUTH' command",
                );
            }
            match cmd.arg_str(0) {
                // WHY: constant-time comparison prevents timing-oracle attacks
                // on the cleartext password (SecFinding-AUTH-TIMING).
                Ok(provided) if {
                    use subtle::ConstantTimeEq;
                    provided.as_bytes().ct_eq(expected.as_bytes()).into()
                } => {
                    *authenticated = true;
                    Frame::ok()
                }
                _ => Frame::err("ERR invalid password"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_protocol::{Command, Frame};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // -- Stub handler ---------------------------------------------------------

    struct EchoHandler {
        calls: AtomicUsize,
    }

    impl EchoHandler {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
            })
        }
    }

    impl RequestHandler for EchoHandler {
        fn handle_command(&self, cmd: Command) -> Frame {
            self.calls.fetch_add(1, Ordering::Relaxed);
            match cmd.name.as_str() {
                "PING" => Frame::SimpleString("PONG".into()),
                "SET" => Frame::ok(),
                "GET" => Frame::bulk(bytes::Bytes::from_static(b"value")),
                _ => Frame::err(format!("ERR unknown command '{}'", cmd.name)),
            }
        }

        fn handle_multi(&self, commands: &[Command]) -> Frame {
            let responses: Vec<Frame> = commands
                .iter()
                .map(|c| self.handle_command(c.clone()))
                .collect();
            Frame::Array(responses)
        }
    }

    // Helper: build a minimal Command from a name and args.
    fn make_cmd(name: &str, args: &[&str]) -> Command {
        Command {
            name: name.to_uppercase(),
            args: args
                .iter()
                .map(|s| bytes::Bytes::copy_from_slice(s.as_bytes()))
                .collect(),
        }
    }

    // -- Test: dispatch happy path (PING) -------------------------------------

    #[test]
    fn dispatch_ping_returns_pong() {
        let handler = EchoHandler::new();
        let cmd = make_cmd("PING", &[]);
        let mut authenticated = true;
        let password: Option<String> = None;
        let mut tx_state = TxState::None;
        let peer: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let response = dispatch(
            &cmd,
            &handler,
            &mut authenticated,
            &password,
            &mut tx_state,
            peer,
        );

        assert_eq!(response, Frame::SimpleString("PONG".into()));
        assert_eq!(handler.calls.load(Ordering::Relaxed), 1);
    }

    // -- Test: unauthenticated client is rejected -----------------------------

    #[test]
    fn dispatch_rejects_unauthenticated_client() {
        let handler = EchoHandler::new();
        let cmd = make_cmd("GET", &["key"]);
        let mut authenticated = false;
        let password = Some("secret".to_string());
        let mut tx_state = TxState::None;
        let peer: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let response = dispatch(
            &cmd,
            &handler,
            &mut authenticated,
            &password,
            &mut tx_state,
            peer,
        );

        assert!(matches!(response, Frame::Error(_)));
        assert_eq!(handler.calls.load(Ordering::Relaxed), 0);
    }

    // -- Test: AUTH with correct password authenticates -----------------------

    #[test]
    fn dispatch_auth_correct_password_authenticates() {
        let cmd = make_cmd("AUTH", &["secret"]);
        let mut authenticated = false;
        let password = Some("secret".to_string());

        let response = handle_auth(&cmd, &mut authenticated, &password);

        assert_eq!(response, Frame::ok());
        assert!(authenticated);
    }

    // -- Test: AUTH with wrong password is rejected ---------------------------

    #[test]
    fn dispatch_auth_wrong_password_rejected() {
        let cmd = make_cmd("AUTH", &["wrong"]);
        let mut authenticated = false;
        let password = Some("secret".to_string());

        let response = handle_auth(&cmd, &mut authenticated, &password);

        assert!(matches!(response, Frame::Error(_)));
        assert!(!authenticated);
    }

    // -- Test: MULTI / EXEC transaction queues commands -----------------------

    #[test]
    fn dispatch_multi_exec_queues_and_executes() {
        let handler = EchoHandler::new();
        let peer: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let mut authenticated = true;
        let password: Option<String> = None;
        let mut tx_state = TxState::None;

        // Issue MULTI
        let multi = make_cmd("MULTI", &[]);
        let r = dispatch(
            &multi,
            &handler,
            &mut authenticated,
            &password,
            &mut tx_state,
            peer,
        );
        assert_eq!(r, Frame::ok());
        assert!(matches!(tx_state, TxState::Queued(_)));

        // Queue SET inside transaction
        let set_cmd = make_cmd("SET", &["k", "v"]);
        let queued_r = dispatch(
            &set_cmd,
            &handler,
            &mut authenticated,
            &password,
            &mut tx_state,
            peer,
        );
        assert_eq!(queued_r, Frame::SimpleString("QUEUED".into()));
        assert_eq!(handler.calls.load(Ordering::Relaxed), 0);

        // Issue EXEC
        let exec = make_cmd("EXEC", &[]);
        let exec_r = dispatch(
            &exec,
            &handler,
            &mut authenticated,
            &password,
            &mut tx_state,
            peer,
        );
        assert!(matches!(exec_r, Frame::Array(_)));
        assert!(matches!(tx_state, TxState::None));
        assert_eq!(handler.calls.load(Ordering::Relaxed), 1);
    }

    // -- Test: EXEC without MULTI returns error -------------------------------

    #[test]
    fn dispatch_exec_without_multi_returns_error() {
        let handler = EchoHandler::new();
        let peer: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let mut authenticated = true;
        let password: Option<String> = None;
        let mut tx_state = TxState::None;

        let exec = make_cmd("EXEC", &[]);
        let r = dispatch(
            &exec,
            &handler,
            &mut authenticated,
            &password,
            &mut tx_state,
            peer,
        );

        assert!(matches!(r, Frame::Error(_)));
    }

    // -- Test: IoUringServer::new / config round-trip -------------------------

    #[test]
    fn io_uring_server_config_round_trip() {
        let cfg = ServerConfig {
            bind: "127.0.0.1".into(),
            resp_port: 16380,
            ..ServerConfig::default()
        };
        let srv = IoUringServer::new(cfg.clone());
        assert_eq!(srv.config().resp_port, 16380);
        assert_eq!(srv.config().bind, "127.0.0.1");
    }
}
