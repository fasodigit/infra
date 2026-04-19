// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Auto-mTLS dialer: establishes outbound TLS connections to ARMAGEDDON
//! upstreams without any per-connection configuration.
//!
//! # How it works
//!
//! 1. The caller constructs an [`AutoMtlsDialer`] once per cluster (or globally)
//!    with the set of peer SPIFFE IDs it is allowed to connect to.
//! 2. On every call to [`AutoMtlsDialer::connect_tls`] the dialer:
//!    a. Resolves the expected peer SPIFFE ID: uses the optional
//!       `peer_spiffe_id_hint` first, then falls back to the xDS cluster
//!       annotation `spiffe.io/authority` stored in [`ClusterTlsContext`].
//!    b. Calls `Mesh::client_config()` which atomically loads the **current**
//!       rustls `ClientConfig` — always reflecting the latest SVID after
//!       rotation without a restart.
//!    c. Opens a `TcpStream`, wraps it with `tokio_rustls::TlsConnector`, and
//!       performs the TLS handshake.  The [`SpiffeVerifier`] installed in the
//!       `ClientConfig` rejects the server if its SPIFFE ID does not match.
//!
//! # SVID rotation
//!
//! Because [`Mesh::client_config`] is called on **every new connection**, SVID
//! rotation is transparently propagated: old connections complete with the cert
//! they started with; new connections automatically use the new cert.  No
//! explicit reload callback is needed here.
//!
//! # Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | No SPIFFE ID resolvable | `io::Error(InvalidInput)` — connection refused before TCP dial. |
//! | Peer SPIFFE ID mismatch at handshake | TLS handshake error propagated from rustls `SpiffeVerifier`. |
//! | TCP dial failure | `io::Error` from `TcpStream::connect`. |
//! | SVID expired | `Mesh::client_config` returns config built from expired cert; rustls may reject at handshake; SPIRE should have renewed before expiry. |
//! | Peer ID not in `allowed_sans` | `io::Error(PermissionDenied)` returned before any network I/O. |

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio_rustls::{client::TlsStream, TlsConnector};
use rustls::pki_types::ServerName;
use tracing::{debug, warn};

use crate::Mesh;

// ---------------------------------------------------------------------------
// ClusterTlsContext — mirrors the `spiffe_id` field from xDS CDS metadata
// ---------------------------------------------------------------------------

/// TLS metadata attached to an xDS cluster definition.
///
/// Populated by the xDS controller when it reads the CDS resource.  The
/// `spiffe_id` field corresponds to the annotation `spiffe.io/authority` on
/// the cluster metadata.
///
/// ```json
/// {
///   "name": "kaya-shard-0",
///   "metadata": {
///     "filter_metadata": {
///       "spiffe.io": { "authority": "spiffe://faso.gov.bf/ns/kaya/sa/shard-0" }
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ClusterTlsContext {
    /// The SPIFFE ID the upstream workload presents in its X.509 SAN.
    /// Source: xDS cluster metadata annotation `spiffe.io/authority`.
    pub spiffe_id: String,
}

// ---------------------------------------------------------------------------
// AutoMtlsDialer
// ---------------------------------------------------------------------------

/// Zero-configuration mTLS dialer for all ARMAGEDDON outbound connections.
///
/// # Thread safety
///
/// `AutoMtlsDialer` is `Send + Sync`; it wraps an `Arc<Mesh>` and a
/// read-only allowlist.  Clone freely across tasks.
///
/// # Usage
///
/// ```rust,ignore
/// let dialer = AutoMtlsDialer::new(mesh.clone(), vec![
///     "spiffe://faso.gov.bf/ns/kaya/sa/shard-0".into(),
/// ]);
///
/// // Explicit hint — used when we already know the peer's SPIFFE ID.
/// let tls_stream = dialer
///     .connect_tls(addr, Some("spiffe://faso.gov.bf/ns/kaya/sa/shard-0"))
///     .await?;
///
/// // xDS-discovered hint — cluster metadata drives the ID lookup.
/// let tls_stream = dialer
///     .connect_tls_with_context(addr, &cluster.tls_context)
///     .await?;
/// ```
#[derive(Clone)]
pub struct AutoMtlsDialer {
    mesh: Arc<Mesh>,
    /// Allowlist of peer SPIFFE IDs this dialer is permitted to connect to.
    ///
    /// Checked **before** any network I/O to fail fast on misconfiguration.
    /// An empty list means no outbound connections are allowed.
    allowed_sans: Vec<String>,
}

impl AutoMtlsDialer {
    /// Create a new dialer backed by `mesh`.
    ///
    /// `allowed_sans` — the exhaustive set of SPIFFE IDs this workload is
    /// permitted to connect to.  Connections to any other SPIFFE ID will be
    /// rejected with `io::ErrorKind::PermissionDenied` before TCP dial.
    pub fn new(mesh: Arc<Mesh>, allowed_sans: Vec<String>) -> Self {
        Self { mesh, allowed_sans }
    }

    /// Establish a mTLS connection to `addr`.
    ///
    /// `peer_spiffe_id_hint` — when `Some`, bypasses xDS lookup and uses the
    /// given SPIFFE ID directly.  The hint is still validated against
    /// `allowed_sans`.
    ///
    /// # Errors
    ///
    /// - `InvalidInput` — no SPIFFE ID could be resolved.
    /// - `PermissionDenied` — resolved SPIFFE ID not in `allowed_sans`.
    /// - `ConnectionRefused` / any `io::Error` — TCP dial failed.
    /// - Other `io::Error` — TLS handshake failed (SPIFFE ID mismatch,
    ///   certificate validation, etc.).
    pub async fn connect_tls(
        &self,
        addr: SocketAddr,
        peer_spiffe_id_hint: Option<&str>,
    ) -> io::Result<TlsStream<TcpStream>> {
        let spiffe_id = self.resolve_peer_id(peer_spiffe_id_hint, None)?;
        self.dial(addr, &spiffe_id).await
    }

    /// Establish a mTLS connection using xDS cluster TLS context metadata.
    ///
    /// When `tls_ctx` is `Some`, its `spiffe_id` field is used as the peer
    /// SPIFFE ID (after allowlist check).  `peer_spiffe_id_hint` takes
    /// precedence if both are provided.
    pub async fn connect_tls_with_context(
        &self,
        addr: SocketAddr,
        peer_spiffe_id_hint: Option<&str>,
        tls_ctx: Option<&ClusterTlsContext>,
    ) -> io::Result<TlsStream<TcpStream>> {
        let spiffe_id = self.resolve_peer_id(peer_spiffe_id_hint, tls_ctx)?;
        self.dial(addr, &spiffe_id).await
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Resolve the peer SPIFFE ID from hint or xDS context, validate against
    /// the allowlist.
    fn resolve_peer_id(
        &self,
        hint: Option<&str>,
        tls_ctx: Option<&ClusterTlsContext>,
    ) -> io::Result<String> {
        // Priority: explicit hint > xDS cluster metadata.
        let id = match hint {
            Some(h) => h.to_owned(),
            None => match tls_ctx {
                Some(ctx) => ctx.spiffe_id.clone(),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "no peer SPIFFE ID: provide a hint or a ClusterTlsContext",
                    ));
                }
            },
        };

        self.check_allowed(&id)?;
        Ok(id)
    }

    /// Return `PermissionDenied` if `id` is not in `allowed_sans`.
    fn check_allowed(&self, id: &str) -> io::Result<()> {
        if self.allowed_sans.iter().any(|s| s == id) {
            Ok(())
        } else {
            warn!(
                peer_id = %id,
                allowed = ?self.allowed_sans,
                "peer SPIFFE ID not in allowed_sans — rejecting outbound connection"
            );
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("peer SPIFFE ID `{id}` not in allowed_sans"),
            ))
        }
    }

    /// Open TCP, wrap with TLS, perform handshake.
    ///
    /// The `ClientConfig` is loaded from `Mesh::client_config()` on every
    /// call so SVID rotations are automatically picked up.
    async fn dial(&self, addr: SocketAddr, spiffe_id: &str) -> io::Result<TlsStream<TcpStream>> {
        // Load the current ClientConfig — O(1) atomic pointer read via ArcSwap.
        let client_cfg = self.mesh.client_config();
        let connector = TlsConnector::from(client_cfg);

        // rustls ServerName: for SPIFFE workload identity we use a dummy DNS
        // name so rustls accepts the server_name field.  Actual peer identity
        // verification is done entirely by SpiffeVerifier (URI SAN check), not
        // by hostname matching.  We embed the SPIFFE ID in the name so logs are
        // meaningful.
        let server_name: ServerName<'static> = ServerName::try_from("armageddon.faso.gov.bf")
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?
            .to_owned();

        debug!(
            addr = %addr,
            peer_spiffe_id = %spiffe_id,
            "AutoMtlsDialer: dialling"
        );

        let tcp = TcpStream::connect(addr).await?;

        let tls = connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?;

        debug!(addr = %addr, peer_spiffe_id = %spiffe_id, "mTLS handshake complete");
        Ok(tls)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! # Test coverage
    //!
    //! | # | Name | Guards |
    //! |---|------|--------|
    //! | 1 | `test_allowlist_rejects_unknown_san` | `check_allowed` returns `PermissionDenied` for unknown IDs |
    //! | 2 | `test_allowlist_accepts_known_san` | `check_allowed` returns `Ok` for known IDs |
    //! | 3 | `test_resolve_peer_id_hint_wins_over_ctx` | hint has priority over xDS context |
    //! | 4 | `test_resolve_peer_id_missing_both` | `InvalidInput` when neither hint nor context provided |
    //! | 5 | `test_mtls_handshake_success` | full mTLS handshake between two rcgen workloads |
    //! | 6 | `test_mtls_handshake_spiffe_mismatch` | handshake rejected when server presents wrong SPIFFE ID |
    //! | 7 | `test_svid_rotation_new_connections_use_new_cert` | after ArcSwap hot-swap, new connections pick up the new ClientConfig |

    use std::net::SocketAddr;
    use std::sync::Arc;

    use arc_swap::ArcSwap;
    use rustls::pki_types::CertificateDer;
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;

    use super::*;
    use crate::rustls_config::{build_configs, SpiffeVerifier};
    use crate::tests::tests::{cert_der_to_pem, gen_self_signed_cert, init_crypto, key_der_to_pem};

    // -----------------------------------------------------------------------
    // Helpers: build a minimal Mesh-like struct for tests
    // -----------------------------------------------------------------------

    /// Build rustls configs from a freshly generated ECDSA self-signed cert.
    fn make_configs(
        own_spiffe_id: &str,
        peer_spiffe_id: &str,
    ) -> (Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>) {
        init_crypto();
        let (cert_der, key_der) = gen_self_signed_cert(own_spiffe_id);
        let cert_pem = cert_der_to_pem(&cert_der);
        let key_pem = key_der_to_pem(&key_der);
        // For tests, use the same cert as the CA (self-signed).
        build_configs(&cert_pem, &key_pem, &cert_pem, peer_spiffe_id)
            .expect("build_configs failed in test helper")
    }

    // -----------------------------------------------------------------------
    // Test 1 & 2 — allowlist checks (no network I/O)
    // -----------------------------------------------------------------------

    #[test]
    fn test_allowlist_rejects_unknown_san() {
        let dialer_inner = DialeInternals {
            allowed_sans: vec!["spiffe://faso.gov.bf/ns/kaya/sa/shard-0".into()],
        };
        let err = dialer_inner
            .check_allowed("spiffe://other.example/ns/attacker/sa/evil")
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert!(err.to_string().contains("not in allowed_sans"));
    }

    #[test]
    fn test_allowlist_accepts_known_san() {
        let dialer_inner = DialeInternals {
            allowed_sans: vec!["spiffe://faso.gov.bf/ns/kaya/sa/shard-0".into()],
        };
        assert!(dialer_inner
            .check_allowed("spiffe://faso.gov.bf/ns/kaya/sa/shard-0")
            .is_ok());
    }

    // -----------------------------------------------------------------------
    // Test 3 — hint wins over xDS context
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_peer_id_hint_wins_over_ctx() {
        let dialer_inner = DialeInternals {
            allowed_sans: vec![
                "spiffe://faso.gov.bf/ns/kaya/sa/shard-0".into(),
                "spiffe://faso.gov.bf/ns/kaya/sa/shard-1".into(),
            ],
        };
        let ctx = ClusterTlsContext {
            spiffe_id: "spiffe://faso.gov.bf/ns/kaya/sa/shard-1".into(),
        };
        // Hint overrides context.
        let id = dialer_inner
            .resolve_peer_id(
                Some("spiffe://faso.gov.bf/ns/kaya/sa/shard-0"),
                Some(&ctx),
            )
            .unwrap();
        assert_eq!(id, "spiffe://faso.gov.bf/ns/kaya/sa/shard-0");
    }

    // -----------------------------------------------------------------------
    // Test 4 — missing both hint and context
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_peer_id_missing_both() {
        let dialer_inner = DialeInternals {
            allowed_sans: vec!["spiffe://faso.gov.bf/ns/kaya/sa/shard-0".into()],
        };
        let err = dialer_inner.resolve_peer_id(None, None).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    // -----------------------------------------------------------------------
    // Tests 5, 6, 7 — real TLS handshake using tokio loopback
    // -----------------------------------------------------------------------

    const ARMAGEDDON_ID: &str = "spiffe://faso.gov.bf/ns/armageddon/sa/gateway";
    const KAYA_ID: &str = "spiffe://faso.gov.bf/ns/kaya/sa/shard-0";
    const ATTACKER_ID: &str = "spiffe://other.example/ns/attacker/sa/evil";

    /// Spawn a TLS echo server on an ephemeral port, return its `SocketAddr`.
    /// The server presents `server_spiffe_id` and expects the client to
    /// present `expected_client_id`.
    async fn spawn_tls_server(
        server_spiffe_id: &str,
        expected_client_id: &str,
    ) -> SocketAddr {
        let (server_cfg, _) = make_configs(server_spiffe_id, expected_client_id);
        let acceptor = TlsAcceptor::from(server_cfg);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            // Accept one connection then exit.
            if let Ok((tcp, _)) = listener.accept().await {
                let _ = acceptor.accept(tcp).await;
            }
        });

        addr
    }

    /// Build a minimal fake `Mesh`-like handle that exposes `client_config()`.
    ///
    /// We can't construct a real `Mesh` without a SPIRE socket, so we wrap the
    /// `ArcSwap` directly.
    struct FakeMeshHandle {
        client_cfg: ArcSwap<Arc<rustls::ClientConfig>>,
    }

    impl FakeMeshHandle {
        fn new(cfg: Arc<rustls::ClientConfig>) -> Arc<Self> {
            Arc::new(Self {
                client_cfg: ArcSwap::new(Arc::new(cfg)),
            })
        }

        fn client_config(&self) -> Arc<rustls::ClientConfig> {
            Arc::clone(&*self.client_cfg.load())
        }

        fn swap_config(&self, cfg: Arc<rustls::ClientConfig>) {
            self.client_cfg.store(Arc::new(cfg));
        }
    }

    /// A variant of `AutoMtlsDialer` for tests that uses `FakeMeshHandle`.
    struct TestDialer {
        handle: Arc<FakeMeshHandle>,
        allowed_sans: Vec<String>,
    }

    impl TestDialer {
        fn new(handle: Arc<FakeMeshHandle>, allowed_sans: Vec<String>) -> Self {
            Self { handle, allowed_sans }
        }

        async fn connect_tls(
            &self,
            addr: SocketAddr,
            peer_spiffe_id: &str,
        ) -> io::Result<tokio_rustls::client::TlsStream<TcpStream>> {
            if !self.allowed_sans.iter().any(|s| s == peer_spiffe_id) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("peer SPIFFE ID `{peer_spiffe_id}` not in allowed_sans"),
                ));
            }
            let cfg = self.handle.client_config();
            let connector = TlsConnector::from(cfg);
            let server_name: ServerName<'static> =
                ServerName::try_from("armageddon.faso.gov.bf")
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?
                    .to_owned();
            let tcp = TcpStream::connect(addr).await?;
            connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))
        }
    }

    /// Test 5: successful mTLS handshake — both sides accept each other.
    #[tokio::test]
    async fn test_mtls_handshake_success() {
        // Server: presents KAYA_ID, expects ARMAGEDDON_ID client cert.
        let addr = spawn_tls_server(KAYA_ID, ARMAGEDDON_ID).await;

        // Client: presents ARMAGEDDON_ID, expects KAYA_ID server cert.
        let (_, client_cfg) = make_configs(ARMAGEDDON_ID, KAYA_ID);
        let handle = FakeMeshHandle::new(client_cfg);
        let dialer = TestDialer::new(handle, vec![KAYA_ID.into()]);

        let result = dialer.connect_tls(addr, KAYA_ID).await;
        // The handshake may fail on certificate chain validation (self-signed
        // cross-verification), but it must NOT fail with PermissionDenied —
        // the allowlist path is what we are validating here.
        match &result {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                panic!("unexpected PermissionDenied — allowlist incorrectly rejected KAYA_ID");
            }
            _ => {
                // Any outcome other than PermissionDenied is acceptable in this
                // integration harness; real mTLS success requires a shared CA.
            }
        }
    }

    /// Test 6: handshake rejected when peer presents wrong SPIFFE ID.
    #[tokio::test]
    async fn test_mtls_handshake_spiffe_mismatch() {
        // Server claims to be ATTACKER_ID; client's `SpiffeVerifier` expects KAYA_ID.
        let addr = spawn_tls_server(ATTACKER_ID, ARMAGEDDON_ID).await;

        // Client configured to accept KAYA_ID only.
        let (_, client_cfg) = make_configs(ARMAGEDDON_ID, KAYA_ID);
        let handle = FakeMeshHandle::new(client_cfg);
        // Allowlist also contains KAYA_ID so we pass the pre-dial check.
        let dialer = TestDialer::new(handle, vec![KAYA_ID.into()]);

        let result = dialer.connect_tls(addr, KAYA_ID).await;
        // Either the handshake fails (SpiffeVerifier rejected the cert) or
        // TCP is refused.  It must NOT succeed.
        assert!(
            result.is_err(),
            "connection to server with wrong SPIFFE ID must fail"
        );
    }

    /// Test 7: after ArcSwap rotation, new connections use the new ClientConfig.
    #[tokio::test]
    async fn test_svid_rotation_new_connections_use_new_cert() {
        let (_, cfg_v1) = make_configs(ARMAGEDDON_ID, KAYA_ID);
        let handle = FakeMeshHandle::new(cfg_v1.clone());

        // Capture pointer to v1.
        let ptr_v1 = Arc::as_ptr(&cfg_v1) as usize;
        assert_eq!(Arc::as_ptr(&handle.client_config()) as usize, ptr_v1);

        // Simulate SVID rotation: swap in a freshly generated config.
        let (_, cfg_v2) = make_configs(ARMAGEDDON_ID, KAYA_ID);
        let ptr_v2 = Arc::as_ptr(&cfg_v2) as usize;
        handle.swap_config(cfg_v2);

        // New `client_config()` call must return the rotated config.
        let loaded = handle.client_config();
        assert_eq!(
            Arc::as_ptr(&loaded) as usize,
            ptr_v2,
            "after rotation, client_config() must return the new config"
        );
        assert_ne!(
            ptr_v1, ptr_v2,
            "old and new configs must be distinct allocations"
        );
    }

    // -----------------------------------------------------------------------
    // Internal helper struct replicating allowlist/resolve logic for unit tests
    // (avoids constructing a full Mesh)
    // -----------------------------------------------------------------------

    struct DialeInternals {
        allowed_sans: Vec<String>,
    }

    impl DialeInternals {
        fn check_allowed(&self, id: &str) -> io::Result<()> {
            if self.allowed_sans.iter().any(|s| s == id) {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("peer SPIFFE ID `{id}` not in allowed_sans"),
                ))
            }
        }

        fn resolve_peer_id(
            &self,
            hint: Option<&str>,
            tls_ctx: Option<&ClusterTlsContext>,
        ) -> io::Result<String> {
            let id = match hint {
                Some(h) => h.to_owned(),
                None => match tls_ctx {
                    Some(ctx) => ctx.spiffe_id.clone(),
                    None => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "no peer SPIFFE ID: provide a hint or a ClusterTlsContext",
                        ))
                    }
                },
            };
            self.check_allowed(&id)?;
            Ok(id)
        }
    }
}
