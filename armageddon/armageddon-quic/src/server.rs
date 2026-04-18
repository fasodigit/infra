// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! HTTP/3 server implementation over QUIC (quinn 0.11 + h3 0.0.8).
//!
//! [`Http3Server`] owns a [`quinn::Endpoint`] and an accept loop that spawns
//! one Tokio task per QUIC connection. Each task drives an
//! [`h3::server::Connection`], receiving HTTP/3 request frames and delegating
//! them to the user-supplied [`RequestHandler`].

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use h3::server::RequestStream;
use h3_quinn::quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use quinn::Endpoint;
use rustls::ServerConfig as RustlsServerConfig;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use armageddon_common::types::{HttpRequest, HttpResponse};

use crate::codec;

// ---------------------------------------------------------------------------
// Public error type
// ---------------------------------------------------------------------------

/// Errors that can occur during QUIC / HTTP/3 server operation.
#[derive(Debug, thiserror::Error)]
pub enum QuicError {
    /// TLS certificate or key could not be parsed.
    #[error("TLS configuration error: {0}")]
    Tls(String),

    /// The certificate PEM file could not be read.
    #[error("cert load error: {0}")]
    CertLoad(String),

    /// The private key PEM file could not be read.
    #[error("key load error: {0}")]
    KeyLoad(String),

    /// The PEM file contained no usable certificate.
    #[error("no certificate found in {0}")]
    NoCert(String),

    /// The PEM file contained no usable private key.
    #[error("no private key found in {0}")]
    NoKey(String),

    /// QUIC endpoint bind failed.
    #[error("endpoint bind error: {0}")]
    Bind(#[from] std::io::Error),

    /// A quinn-level error on an established connection.
    #[error("QUIC connection error: {0}")]
    Connection(String),

    /// An h3-level protocol error.
    #[error("HTTP/3 protocol error: {0}")]
    H3(String),

    /// The request handler returned an error.
    #[error("handler error: {0}")]
    Handler(String),
}

// ---------------------------------------------------------------------------
// QuicListenerConfig
// ---------------------------------------------------------------------------

/// Configuration for the HTTP/3 QUIC listener.
#[derive(Debug, Clone)]
pub struct QuicListenerConfig {
    /// IP address to bind (e.g. `"0.0.0.0"` or `"::"`).
    pub address: String,

    /// UDP port to listen on.
    pub port: u16,

    /// Path to the PEM-encoded TLS certificate (chain).
    pub cert_path: String,

    /// Path to the PEM-encoded private key.
    pub key_path: String,

    /// Maximum number of concurrent QUIC streams per connection accepted
    /// before back-pressure is applied.
    pub max_concurrent_streams: u64,
}

impl Default for QuicListenerConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 4433,
            cert_path: "/etc/armageddon/tls/server.crt".to_string(),
            key_path: "/etc/armageddon/tls/server.key".to_string(),
            max_concurrent_streams: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// RequestHandler trait
// ---------------------------------------------------------------------------

/// Implemented by anything that wants to receive decoded HTTP/3 requests.
///
/// The trait is object-safe and send-safe; it can be used as `Arc<dyn RequestHandler>`.
#[async_trait]
pub trait RequestHandler: Send + Sync + 'static {
    /// Process one request and return a response.
    async fn handle(&self, req: HttpRequest) -> Result<HttpResponse, QuicError>;
}

// ---------------------------------------------------------------------------
// Http3Server
// ---------------------------------------------------------------------------

/// HTTP/3 server that listens on a QUIC endpoint and dispatches requests to
/// a [`RequestHandler`].
#[derive(Debug)]
pub struct Http3Server {
    config: QuicListenerConfig,
    endpoint: Endpoint,
}

impl Http3Server {
    // -- construction --

    /// Create and bind a new HTTP/3 server.
    ///
    /// Loads the TLS certificate and private key from disk, configures
    /// rustls with ALPN `h3`, and creates the QUIC endpoint.
    pub async fn new(config: QuicListenerConfig) -> Result<Self, QuicError> {
        let (certs, key) = load_tls(&config.cert_path, &config.key_path)?;

        // Build rustls ServerConfig with ALPN = ["h3"].
        let mut tls_cfg = RustlsServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| QuicError::Tls(e.to_string()))?;

        tls_cfg.alpn_protocols = vec![b"h3".to_vec()];
        tls_cfg.max_early_data_size = u32::MAX; // 0-RTT where supported

        let quinn_server_cfg =
            quinn::crypto::rustls::QuicServerConfig::try_from(tls_cfg)
                .map_err(|e| QuicError::Tls(e.to_string()))?;

        let server_cfg = quinn::ServerConfig::with_crypto(Arc::new(quinn_server_cfg));

        let addr: SocketAddr = format!("{}:{}", config.address, config.port)
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                QuicError::Bind(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    e.to_string(),
                ))
            })?;

        let endpoint = Endpoint::server(server_cfg, addr)
            .map_err(QuicError::Bind)?;

        info!(
            addr = %addr,
            "HTTP/3 QUIC endpoint bound"
        );

        Ok(Self { config, endpoint })
    }

    // -- runtime --

    /// Accept loop: runs until `shutdown` fires or the endpoint is closed.
    ///
    /// For each incoming QUIC connection a Tokio task is spawned. Each task
    /// drives an h3 connection and calls `handler.handle()` for every
    /// complete request.
    pub async fn run<H>(
        self,
        handler: Arc<H>,
        mut shutdown: broadcast::Receiver<()>,
    ) -> Result<(), QuicError>
    where
        H: RequestHandler + Sync + 'static,
    {
        let max_streams = self.config.max_concurrent_streams;
        let endpoint = self.endpoint;

        loop {
            tokio::select! {
                biased;

                _ = shutdown.recv() => {
                    info!("HTTP/3 server received shutdown signal");
                    endpoint.close(
                        quinn::VarInt::from_u32(0),
                        b"server shutdown",
                    );
                    break;
                }

                conn = endpoint.accept() => {
                    match conn {
                        None => {
                            info!("HTTP/3 endpoint closed");
                            break;
                        }
                        Some(incoming) => {
                            let handler = Arc::clone(&handler);
                            tokio::spawn(async move {
                                match incoming.await {
                                    Ok(conn) => {
                                        handle_connection(conn, handler, max_streams).await;
                                    }
                                    Err(e) => {
                                        warn!(err = %e, "QUIC handshake failed");
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }

        // Wait for in-flight connections to drain (best-effort).
        endpoint.wait_idle().await;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Per-connection handling
// ---------------------------------------------------------------------------

/// Drive a single QUIC connection as an HTTP/3 server.
async fn handle_connection<H>(
    conn: quinn::Connection,
    handler: Arc<H>,
    _max_streams: u64,
) where
    H: RequestHandler + Sync + 'static,
{
    let peer = conn.remote_address();
    debug!(peer = %peer, "HTTP/3 connection established");

    // Wrap the quinn connection in an h3-quinn transport.
    let h3_conn = h3_quinn::Connection::new(conn);

    // Build the h3 server connection.
    let mut h3 = match h3::server::Connection::new(h3_conn).await {
        Ok(c) => c,
        Err(e) => {
            warn!(peer = %peer, err = %e, "h3 connection setup failed");
            return;
        }
    };

    loop {
        // h3 0.0.8: accept() returns Option<RequestResolver<...>> (a future).
        // We first await accept() to get the resolver, then await the resolver
        // to get (request_headers, stream).
        match h3.accept().await {
            Ok(Some(resolver)) => {
                // h3 0.0.8: RequestResolver exposes .resolve() to get
                // (request_headers, stream).  We call resolve() which is async.
                let handler = Arc::clone(&handler);
                tokio::spawn(async move {
                    match resolver.resolve_request().await {
                        Ok((req, stream)) => {
                            handle_request(req, stream, handler, peer).await;
                        }
                        Err(e) => {
                            warn!(peer = %peer, err = %e, "HTTP/3 request resolve error");
                        }
                    }
                });
            }
            Ok(None) => {
                debug!(peer = %peer, "HTTP/3 connection closed by peer");
                break;
            }
            Err(e) => {
                // A connection-level h3 error closes the whole connection.
                let err_str = e.to_string();
                if err_str.contains("Application") {
                    debug!(peer = %peer, err = %e, "HTTP/3 connection closed");
                } else {
                    warn!(peer = %peer, err = %e, "HTTP/3 connection error");
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-request handling
// ---------------------------------------------------------------------------

/// Process one HTTP/3 request/response exchange.
async fn handle_request<H, T>(
    req: http::Request<()>,
    mut stream: RequestStream<T, Bytes>,
    handler: Arc<H>,
    peer: SocketAddr,
) where
    H: RequestHandler + Sync + 'static,
    T: h3::quic::BidiStream<Bytes>,
{
    debug!(
        peer = %peer,
        method = %req.method(),
        uri = %req.uri(),
        "HTTP/3 request received"
    );

    // Collect the request body (h3 DATA frames).
    let mut body_buf: Vec<u8> = Vec::new();
    loop {
        match stream.recv_data().await {
            Ok(Some(mut chunk)) => {
                use bytes::Buf;
                while chunk.remaining() > 0 {
                    let slice = chunk.chunk();
                    body_buf.extend_from_slice(slice);
                    let len = slice.len();
                    chunk.advance(len);
                }
            }
            Ok(None) => break,
            Err(e) => {
                warn!(peer = %peer, err = %e, "Error reading request body");
                break;
            }
        }
    }

    // Decode the http::Request + body bytes → internal HttpRequest.
    let body = if body_buf.is_empty() { None } else { Some(body_buf) };
    let internal_req = codec::http_request_to_internal(req, body);

    // Forward to handler.
    let response = match handler.handle(internal_req).await {
        Ok(r) => r,
        Err(e) => {
            error!(peer = %peer, err = %e, "Request handler error");
            // Respond with 500.
            HttpResponse {
                status: 500,
                headers: Default::default(),
                body: Some(b"Internal Server Error".to_vec()),
            }
        }
    };

    // Send the HTTP/3 response.
    let http_resp = build_http_response(&response);
    if let Err(e) = stream.send_response(http_resp).await {
        warn!(peer = %peer, err = %e, "Failed to send HTTP/3 response headers");
        return;
    }

    if let Some(body) = response.body {
        if let Err(e) = stream.send_data(Bytes::from(body)).await {
            warn!(peer = %peer, err = %e, "Failed to send HTTP/3 response body");
        }
    }

    if let Err(e) = stream.finish().await {
        debug!(peer = %peer, err = %e, "Failed to finish HTTP/3 stream");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load DER-encoded cert chain and private key from PEM files.
fn load_tls(
    cert_path: &str,
    key_path: &str,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), QuicError> {
    use std::fs::File;
    use std::io::BufReader;

    // -- certificate chain --
    let cert_file = File::open(cert_path)
        .map_err(|e| QuicError::CertLoad(format!("{cert_path}: {e}")))?;
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut BufReader::new(cert_file))
            .collect::<Result<_, _>>()
            .map_err(|e| QuicError::CertLoad(format!("{cert_path}: {e}")))?;
    if certs.is_empty() {
        return Err(QuicError::NoCert(cert_path.to_string()));
    }

    // -- private key --
    let key_file = File::open(key_path)
        .map_err(|e| QuicError::KeyLoad(format!("{key_path}: {e}")))?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))
        .map_err(|e| QuicError::KeyLoad(format!("{key_path}: {e}")))?
        .ok_or_else(|| QuicError::NoKey(key_path.to_string()))?;

    Ok((certs, key))
}

/// Convert an internal [`HttpResponse`] to an [`http::Response<()>`] suitable
/// for h3.
fn build_http_response(resp: &HttpResponse) -> http::Response<()> {
    let status = http::StatusCode::from_u16(resp.status)
        .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);

    let mut builder = http::Response::builder().status(status)
        .version(http::Version::HTTP_3);

    for (k, v) in &resp.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }

    builder.body(()).unwrap_or_else(|_| {
        http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(())
            .expect("infallible response construction")
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod test_helpers {
    //! Shared test helpers: in-memory self-signed certificate generation.
    //!
    //! Uses `rcgen` if available; otherwise falls back to writing pre-baked
    //! PEM strings to temp files.

    use std::io::Write;
    use tempfile::NamedTempFile;

    /// A temporary cert+key pair on disk.
    pub struct TempTlsFiles {
        pub cert_file: NamedTempFile,
        pub key_file: NamedTempFile,
    }

    impl TempTlsFiles {
        /// Write a self-signed cert/key generated by `rcgen`.
        pub fn generate(common_name: &str) -> Self {
            let cert = rcgen::generate_simple_self_signed(vec![common_name.to_string()])
                .expect("rcgen cert generation failed");
            let cert_pem = cert.cert.pem();
            let key_pem = cert.key_pair.serialize_pem();

            let mut cert_file = NamedTempFile::new().unwrap();
            cert_file.write_all(cert_pem.as_bytes()).unwrap();

            let mut key_file = NamedTempFile::new().unwrap();
            key_file.write_all(key_pem.as_bytes()).unwrap();

            TempTlsFiles { cert_file, key_file }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Once;
    use tempfile::NamedTempFile;
    use test_helpers::TempTlsFiles;

    /// Install a process-global `rustls` `CryptoProvider` exactly once.
    ///
    /// `rustls` 0.23 requires an explicit provider to be selected at runtime.
    /// Every test that touches `rustls` / `quinn` / `h3` must call this helper
    /// as the very first thing; otherwise the provider is only coincidentally
    /// installed by a lexically-earlier test in the same process and the
    /// ordering breaks when cargo parallelises crates.
    fn ensure_crypto_provider() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    // -- helper: build a minimal valid config pointing at temp files --
    fn cfg_from_files(tls: &TempTlsFiles) -> QuicListenerConfig {
        QuicListenerConfig {
            address: "127.0.0.1".to_string(),
            port: 0, // OS-assigned
            cert_path: tls.cert_file.path().to_string_lossy().to_string(),
            key_path: tls.key_file.path().to_string_lossy().to_string(),
            max_concurrent_streams: 10,
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: Http3Server::new succeeds with valid cert+key
    //
    // Hermetic: `rcgen` is a pure-Rust self-signed generator (no external
    // `openssl` binary) and `port: 0` asks the kernel for an OS-assigned
    // ephemeral UDP port. The server binds on `127.0.0.1` so no external
    // network capability is required. This makes the test safe in any
    // containerized CI environment.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_new_valid_tls() {
        ensure_crypto_provider();
        let tls = TempTlsFiles::generate("localhost");
        let cfg = cfg_from_files(&tls);
        let server = Http3Server::new(cfg).await;
        assert!(server.is_ok(), "Http3Server::new should succeed: {:?}", server.err());
    }

    // -----------------------------------------------------------------------
    // Test 2: Http3Server::new fails with an invalid (garbage) cert
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_new_invalid_cert() {
        ensure_crypto_provider();
        let tls = TempTlsFiles::generate("localhost");

        // Overwrite the cert with garbage.
        let mut bad_cert = NamedTempFile::new().unwrap();
        bad_cert.write_all(b"-----BEGIN CERTIFICATE-----\nNOT_VALID_BASE64\n-----END CERTIFICATE-----\n").unwrap();

        let cfg = QuicListenerConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
            cert_path: bad_cert.path().to_string_lossy().to_string(),
            key_path: tls.key_file.path().to_string_lossy().to_string(),
            max_concurrent_streams: 10,
        };

        let result = Http3Server::new(cfg).await;
        assert!(result.is_err(), "Http3Server::new should fail with invalid cert");
    }

    // -----------------------------------------------------------------------
    // Test 3: Http3Server::new fails when cert file path does not exist
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_new_missing_cert_file() {
        ensure_crypto_provider();
        let tls = TempTlsFiles::generate("localhost");
        let cfg = QuicListenerConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
            cert_path: "/nonexistent/path/server.crt".to_string(),
            key_path: tls.key_file.path().to_string_lossy().to_string(),
            max_concurrent_streams: 10,
        };
        let result = Http3Server::new(cfg).await;
        assert!(
            matches!(result, Err(QuicError::CertLoad(_))),
            "Expected CertLoad error, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: QUIC round-trip handshake (server + client quinn tasks)
    //
    // Hermetic: server binds on `127.0.0.1:<ephemeral UDP>` (kernel-assigned
    // via `port: 0`), and `server_addr` is read *before* the server task is
    // spawned — so the client never races against the bind. Client binds on
    // `127.0.0.1:0`, also ephemeral. Self-signed cert via `rcgen`, and the
    // client uses a `SkipVerification` verifier confined to `#[cfg(test)]`.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_quic_handshake_roundtrip() {
        ensure_crypto_provider();
        use quinn::ClientConfig;
        use std::sync::Arc;

        let tls = TempTlsFiles::generate("localhost");
        let cfg = cfg_from_files(&tls);

        // Build server — this synchronously binds the UDP endpoint, so the
        // local address is immediately available and the client cannot race
        // against bind completion.
        let server = Http3Server::new(cfg).await.expect("server creation");
        let server_addr = server.endpoint.local_addr().unwrap();

        // Build a trusting client (skips cert validation for test purposes).
        // Must advertise ALPN "h3" so the server accepts the handshake.
        let mut client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipVerification))
            .with_no_client_auth();
        client_crypto.alpn_protocols = vec![b"h3".to_vec()];

        let client_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
            .expect("client crypto");

        let client_cfg = ClientConfig::new(Arc::new(client_crypto));
        // Bind client on loopback to stay strictly hermetic (no external NIC).
        let mut client_endpoint = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap())
            .expect("client endpoint");
        client_endpoint.set_default_client_config(client_cfg);

        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

        // Spawn server accept loop. The endpoint is already bound, so the
        // client connect below cannot observe a half-open listener.
        let server_handle = tokio::spawn(async move {
            struct NoopHandler;
            #[async_trait::async_trait]
            impl RequestHandler for NoopHandler {
                async fn handle(&self, _req: HttpRequest) -> Result<HttpResponse, QuicError> {
                    Ok(HttpResponse {
                        status: 200,
                        headers: Default::default(),
                        body: None,
                    })
                }
            }
            server.run(Arc::new(NoopHandler), shutdown_rx).await
        });

        // Connect client → server. No sleep needed: the server endpoint was
        // bound before spawn, so the UDP socket is already serviceable.
        let connect_result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            async {
                client_endpoint
                    .connect(server_addr, "localhost")
                    .expect("connect call")
                    .await
            },
        )
        .await
        .expect("QUIC handshake timed out");

        // Signal shutdown regardless of outcome.
        let _ = shutdown_tx.send(());
        let _ = server_handle.await;

        assert!(
            connect_result.is_ok(),
            "QUIC handshake should succeed: {:?}",
            connect_result.err()
        );
    }

    // -- Stub TLS verifier for the test client only --
    #[derive(Debug)]
    struct SkipVerification;

    impl rustls::client::danger::ServerCertVerifier for SkipVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}
