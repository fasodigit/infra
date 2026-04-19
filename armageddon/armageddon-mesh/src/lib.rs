// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! # `armageddon-mesh` — SPIFFE/SPIRE mTLS for ARMAGEDDON
//!
//! This crate manages the full SVID lifecycle for the ARMAGEDDON security
//! gateway: it connects to the local SPIRE agent via the workload API, watches
//! the X.509 SVID stream, and exposes **hot-swappable** rustls
//! [`ServerConfig`] and [`ClientConfig`] via [`ArcSwap`].
//!
//! ## Trust domain
//!
//! `spiffe://faso.gov.bf/` — all workloads in the FASO DIGITALISATION
//! ecosystem share this trust domain.  NEXUS and KAYA are peer workloads;
//! they obtain their SVIDs from the same SPIRE server.
//!
//! ## SVID rotation
//!
//! Default SPIRE TTL is 24 hours; renewal occurs 12 h before expiry (SPIRE
//! default).  `SvidManager` receives the rotation via the streaming workload
//! API and calls `rebuild_configs` atomically — **no restart required**.
//!
//! ## Hot-swap guarantee
//!
//! `ServerConfig` and `ClientConfig` are stored in [`ArcSwap<Arc<…>>`].
//! An `ArcSwap::load` takes a single atomic pointer-read; readers never block.
//! In-flight TLS sessions hold their own `Arc` clone and complete with the
//! config they started.
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | Leader loss / quorum loss | Not applicable (stateless mesh layer). |
//! | SPIRE agent socket unreachable | `Mesh::new` returns `MeshError::Spiffe`. `run` retries every 5 s. |
//! | SVID expired before renewal | `MeshError::SvidExpired` on new handshakes; existing sessions complete. |
//! | Network partition (SPIRE ↔ workload) | Previous SVID served until expiry; `replication_lag_seconds` metric should be surfaced by the operator. |
//! | Shutdown signal | `run` exits cleanly; rotation channel is dropped. |
//!
//! ## mTLS all-cluster RPCs
//!
//! All cluster-to-cluster RPCs (ARMAGEDDON → KAYA, ARMAGEDDON → NEXUS, etc.)
//! MUST go through a rustls connector that calls `Mesh::client_config()` on
//! each connection setup.  Server listeners MUST call `Mesh::server_config()`.
//!
//! ## Wire-up (by the caller after this crate is complete)
//!
//! ```rust,ignore
//! let mesh = Mesh::new(Path::new("/run/spire/sockets/agent.sock")).await?;
//! let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
//! tokio::spawn(Arc::clone(&mesh).run(shutdown_rx));
//! // pass mesh.client_config() / mesh.server_config() to hyper/tonic connectors
//! ```

pub mod auto_mtls;
pub mod error;
pub mod rustls_config;
pub mod spire_client;
pub mod svid_manager;

#[cfg(test)]
mod tests;

pub use auto_mtls::{AutoMtlsDialer, ClusterTlsContext};
pub use error::MeshError;
pub use rustls_config::{SpiffeVerifier, TRUST_DOMAIN_PREFIX};
pub use svid_manager::{RotationEvent, SvidManager};

use std::path::Path;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rustls::{ClientConfig, ServerConfig};
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::svid_manager::SvidManager as SvidMgr;

/// Entry point for the ARMAGEDDON mTLS mesh.
///
/// Holds hot-swappable rustls configs produced from the SPIRE SVID.
/// Create via [`Mesh::new`], then call [`Mesh::run`] in a dedicated task.
pub struct Mesh {
    svid_mgr: Arc<SvidMgr>,
    client_cfg: ArcSwap<Arc<ClientConfig>>,
    server_cfg: ArcSwap<Arc<ServerConfig>>,
    /// PEM for the CA trust bundle — kept so we can rebuild configs on
    /// rotation without going back to disk.
    ca_bundle_pem: Vec<u8>,
    /// Expected peer SPIFFE ID (used by `SpiffeVerifier`).
    expected_peer_id: String,
}

impl Mesh {
    /// Initialise the mesh by fetching the first SVID from the SPIRE agent.
    ///
    /// `socket_path` — path to the SPIRE workload-API Unix socket, typically
    /// `/run/spire/sockets/agent.sock`.
    ///
    /// `ca_bundle_pem` — PEM-encoded CA certificates from the SPIRE trust
    /// bundle.  In production these are obtained from
    /// `WorkloadApiClient::fetch_x509_bundles`; in tests they are fixtures.
    ///
    /// `expected_peer_id` — the SPIFFE ID the remote peer MUST present, e.g.
    /// `spiffe://faso.gov.bf/ns/default/sa/kaya`.
    ///
    /// # Errors
    ///
    /// Returns `MeshError::Spiffe` if the socket is unreachable, or
    /// `MeshError::PemDecode` / `MeshError::Rustls` if the initial cert
    /// material cannot be parsed.
    pub async fn new(
        socket_path: &Path,
        ca_bundle_pem: Vec<u8>,
        expected_peer_id: impl Into<String>,
    ) -> Result<Arc<Self>, MeshError> {
        let expected_peer_id = expected_peer_id.into();
        let svid_mgr = SvidMgr::new(socket_path).await?;
        let current = svid_mgr.current_svid();

        let (cert_chain_pem, private_key_pem) = svid_to_pem(&current)?;

        let (server, client) = rustls_config::build_configs(
            &cert_chain_pem,
            &private_key_pem,
            &ca_bundle_pem,
            &expected_peer_id,
        )?;

        info!(
            peer_id = %expected_peer_id,
            "Mesh initialised with initial SVID"
        );

        Ok(Arc::new(Self {
            svid_mgr,
            client_cfg: ArcSwap::new(Arc::new(client)),
            server_cfg: ArcSwap::new(Arc::new(server)),
            ca_bundle_pem,
            expected_peer_id,
        }))
    }

    /// Return the current rustls `ClientConfig` (zero-copy Arc clone).
    ///
    /// Call this on every new outbound connection so the latest SVID is used.
    pub fn client_config(&self) -> Arc<ClientConfig> {
        Arc::clone(&*self.client_cfg.load())
    }

    /// Return the current rustls `ServerConfig` (zero-copy Arc clone).
    ///
    /// Attach to your TLS acceptor; new connections will use the latest SVID.
    pub fn server_config(&self) -> Arc<ServerConfig> {
        Arc::clone(&*self.server_cfg.load())
    }

    /// Drive the SVID rotation loop.
    ///
    /// Should be spawned in a dedicated `tokio::spawn` task.  Exits when
    /// `shutdown` fires.
    pub async fn run(self: Arc<Self>, mut shutdown: broadcast::Receiver<()>) {
        let mut rotations = self.svid_mgr.watch_rotations();

        // Also drive the underlying SvidManager reconnect loop.
        let mgr = Arc::clone(&self.svid_mgr);
        let (inner_shutdown_tx, inner_shutdown_rx) = broadcast::channel::<()>(1);
        tokio::spawn(async move {
            mgr.run(inner_shutdown_rx).await;
        });

        loop {
            tokio::select! {
                biased;

                _ = shutdown.recv() => {
                    info!("Mesh::run received shutdown");
                    // Signal the inner SvidManager to stop.
                    let _ = inner_shutdown_tx.send(());
                    return;
                }

                event = rotations.recv() => {
                    match event {
                        Ok(ev) => {
                            if let Err(e) = self.apply_rotation() {
                                error!(
                                    spiffe_id = %ev.spiffe_id,
                                    err = %e,
                                    "failed to rebuild rustls configs after rotation"
                                );
                            } else {
                                info!(
                                    spiffe_id = %ev.spiffe_id,
                                    "rustls configs refreshed after SVID rotation"
                                );
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // We missed rotation events; force a rebuild now.
                            error!(missed = n, "rotation receiver lagged; force-rebuilding configs");
                            let _ = self.apply_rotation();
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("rotation channel closed; exiting Mesh::run");
                            return;
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Rebuild and hot-swap both rustls configs from the current SVID.
    fn apply_rotation(&self) -> Result<(), MeshError> {
        let current = self.svid_mgr.current_svid();
        let (cert_pem, key_pem) = svid_to_pem(&current)?;

        rustls_config::rebuild_configs(
            &cert_pem,
            &key_pem,
            &self.ca_bundle_pem,
            &self.expected_peer_id,
            &self.server_cfg,
            &self.client_cfg,
        )
    }
}

// ---------------------------------------------------------------------------
// SVID → PEM conversion
// ---------------------------------------------------------------------------

/// Convert an [`X509Svid`] to `(cert_chain_pem, private_key_pem)` byte
/// vectors suitable for the `rustls-pemfile` parser.
///
/// `spiffe 0.4` stores certificates as DER-encoded bytes and the private key
/// as a PKCS#8 DER blob.  We re-wrap them in standard PEM armor.
fn svid_to_pem(
    svid: &spiffe::svid::x509::X509Svid,
) -> Result<(Vec<u8>, Vec<u8>), MeshError> {
    // --- Certificate chain ---
    // X509Svid::cert_chain() returns &[Certificate] where Certificate exposes
    // the DER via AsRef<[u8]>.
    let chain = svid.cert_chain();
    if chain.is_empty() {
        return Err(MeshError::PemDecode("SVID cert chain is empty".into()));
    }

    let mut cert_pem = String::new();
    for cert in chain {
        pem_encode_block(&mut cert_pem, "CERTIFICATE", cert.as_ref());
    }

    // --- Private key (PKCS#8 DER) ---
    // X509Svid::private_key() returns &PrivateKey which is also AsRef<[u8]>.
    let key_der = svid.private_key();
    let mut key_pem = String::new();
    pem_encode_block(&mut key_pem, "PRIVATE KEY", key_der.as_ref());

    Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
}

/// Append a single PEM block (header + base64 + footer) to `buf`.
fn pem_encode_block(buf: &mut String, label: &str, der: &[u8]) {
    use std::fmt::Write as _;
    let b64 = base64_encode(der);
    writeln!(buf, "-----BEGIN {label}-----").ok();
    for chunk in b64.as_bytes().chunks(64) {
        // SAFETY: base64 output is always valid ASCII.
        writeln!(buf, "{}", unsafe { std::str::from_utf8_unchecked(chunk) }).ok();
    }
    writeln!(buf, "-----END {label}-----").ok();
}

/// Minimal base64 encoder (standard alphabet, no padding needed for PEM but
/// we include `=` padding for correctness).
fn base64_encode(data: &[u8]) -> String {
    // Use the standard alphabet with padding.
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() * 4 / 3) + 4);
    let mut i = 0;
    while i + 2 < data.len() {
        let b0 = data[i] as usize;
        let b1 = data[i + 1] as usize;
        let b2 = data[i + 2] as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 0x3) << 4) | (b1 >> 4)] as char);
        out.push(TABLE[((b1 & 0xf) << 2) | (b2 >> 6)] as char);
        out.push(TABLE[b2 & 0x3f] as char);
        i += 3;
    }
    match data.len() - i {
        1 => {
            let b0 = data[i] as usize;
            out.push(TABLE[b0 >> 2] as char);
            out.push(TABLE[(b0 & 0x3) << 4] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let b0 = data[i] as usize;
            let b1 = data[i + 1] as usize;
            out.push(TABLE[b0 >> 2] as char);
            out.push(TABLE[((b0 & 0x3) << 4) | (b1 >> 4)] as char);
            out.push(TABLE[(b1 & 0xf) << 2] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}
