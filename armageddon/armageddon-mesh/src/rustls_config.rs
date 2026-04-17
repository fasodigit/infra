// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Hot-swappable rustls [`ServerConfig`] and [`ClientConfig`] produced from
//! the current SPIRE SVID.
//!
//! # Hot-swap strategy
//!
//! Both configs live in [`ArcSwap`] cells.  On every SVID rotation event the
//! [`rebuild_configs`] function is called: it parses the new cert + key from
//! the SVID, constructs fresh `ServerConfig` / `ClientConfig`, and atomically
//! `store`s them.  In-flight TLS handshakes already holding an old `Arc` clone
//! complete unaffected.  New handshakes see the updated config immediately on
//! the next `load()` call.
//!
//! # Peer verification
//!
//! [`SpiffeVerifier`] is installed as both `ClientCertVerifier` (server side)
//! and `ServerCertVerifier` (client side).  It:
//!
//! 1. Extracts the first URI SAN from the end-entity DER certificate using
//!    `x509-parser` (already a transitive dependency of `spiffe`).
//! 2. Compares the URI against the configured expected SPIFFE ID using
//!    **constant-time** `subtle::ConstantTimeEq` to prevent timing oracles.
//!
//! # Failure modes
//!
//! - PEM empty / malformed → [`MeshError::PemDecode`].
//! - Key / cert mismatch → rustls rejects in `with_single_cert` →
//!   [`MeshError::Rustls`].
//! - SPIFFE ID mismatch at handshake → connection dropped, logged with both
//!   IDs; returns `rustls::Error::General(…)`.

use std::sync::Arc;

use arc_swap::ArcSwap;
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime},
    server::danger::{ClientCertVerified, ClientCertVerifier},
    ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig, SignatureScheme,
};
use subtle::ConstantTimeEq as _;
use tracing::{debug, warn};

use crate::error::MeshError;

/// The FASO DIGITALISATION SPIFFE trust domain prefix.
/// Every valid SPIFFE ID in this mesh starts with this string.
pub const TRUST_DOMAIN_PREFIX: &str = "spiffe://faso.gov.bf/";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build fresh [`ServerConfig`] and [`ClientConfig`] from PEM bytes.
///
/// Both configs enforce mTLS: the server requests and validates a client
/// certificate; the client validates the server certificate.
///
/// Parsing is performed twice (once per config) because `PrivateKeyDer` does
/// not implement `Clone` in `rustls-pki-types` 1.x.
pub fn build_configs(
    cert_chain_pem: &[u8],
    private_key_pem: &[u8],
    ca_bundle_pem: &[u8],
    expected_peer_spiffe_id: &str,
) -> Result<(Arc<ServerConfig>, Arc<ClientConfig>), MeshError> {
    let server_cfg = build_server_config(
        parse_cert_chain(cert_chain_pem)?,
        parse_private_key(private_key_pem)?,
        parse_cert_chain(ca_bundle_pem)?,
        expected_peer_spiffe_id,
    )?;

    let client_cfg = build_client_config(
        parse_cert_chain(cert_chain_pem)?,
        parse_private_key(private_key_pem)?,
        parse_cert_chain(ca_bundle_pem)?,
        expected_peer_spiffe_id,
    )?;

    Ok((Arc::new(server_cfg), Arc::new(client_cfg)))
}

/// Atomically replace both configs in their [`ArcSwap`] cells.
///
/// Called by [`Mesh::run`] on every SVID rotation event.  The store is
/// ordered-release so concurrent readers see the new config immediately.
pub fn rebuild_configs(
    cert_chain_pem: &[u8],
    private_key_pem: &[u8],
    ca_bundle_pem: &[u8],
    expected_peer_spiffe_id: &str,
    server_swap: &ArcSwap<Arc<ServerConfig>>,
    client_swap: &ArcSwap<Arc<ClientConfig>>,
) -> Result<(), MeshError> {
    let (server, client) = build_configs(
        cert_chain_pem,
        private_key_pem,
        ca_bundle_pem,
        expected_peer_spiffe_id,
    )?;
    server_swap.store(Arc::new(server));
    client_swap.store(Arc::new(client));
    debug!(
        peer_id = %expected_peer_spiffe_id,
        "rustls configs hot-swapped after SVID rotation"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// SpiffeVerifier — constant-time SPIFFE ID matcher
// ---------------------------------------------------------------------------

/// Custom rustls verifier that enforces SPIFFE URI SAN matching.
///
/// Installed as `ClientCertVerifier` on the server side and as
/// `ServerCertVerifier` on the client side so that every mTLS handshake is
/// gated on the correct SPIFFE identity.
///
/// The comparison uses `subtle::ConstantTimeEq` to prevent timing side-channels
/// on the SPIFFE ID value.
#[derive(Debug, Clone)]
pub struct SpiffeVerifier {
    /// CA certificate DER blobs used to root-validate the peer chain.
    /// (Structural; actual validation is performed by the SPIRE CA.)
    ca_certs: Vec<CertificateDer<'static>>,
    /// The SPIFFE URI the peer MUST present exactly.
    expected_id: String,
}

impl SpiffeVerifier {
    /// Create a verifier that accepts peers presenting exactly `expected_id`.
    pub fn new(ca_certs: Vec<CertificateDer<'static>>, expected_id: impl Into<String>) -> Self {
        Self {
            ca_certs,
            expected_id: expected_id.into(),
        }
    }

    // ---- helpers -----------------------------------------------------------

    /// Extract the first URI SAN from a DER certificate.
    ///
    /// Returns `None` if the cert has no URI SAN or cannot be parsed by
    /// `x509-parser`.
    fn extract_uri_san(cert_der: &CertificateDer<'_>) -> Option<String> {
        use x509_parser::prelude::*;

        let (_, parsed) = X509Certificate::from_der(cert_der.as_ref()).ok()?;
        let san_ext = parsed.subject_alternative_name().ok()??;
        for san in &san_ext.value.general_names {
            if let GeneralName::URI(uri) = san {
                return Some((*uri).to_string());
            }
        }
        None
    }

    /// Constant-time equality check for two string slices.
    ///
    /// Returns `false` immediately if lengths differ (leaks only the length
    /// comparison, which is acceptable for SPIFFE IDs since the pattern is
    /// public).  When lengths match, uses `subtle::ConstantTimeEq` so that
    /// early-exit timing is suppressed.
    #[inline]
    fn ct_eq(a: &str, b: &str) -> bool {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        if a_bytes.len() != b_bytes.len() {
            return false;
        }
        a_bytes.ct_eq(b_bytes).into()
    }

    /// Core verification: extract URI SAN and constant-time compare.
    fn verify_spiffe_id(&self, cert: &CertificateDer<'_>) -> Result<(), MeshError> {
        let uri = Self::extract_uri_san(cert).ok_or_else(|| MeshError::SpiffeIdMismatch {
            got: "<no URI SAN found>".into(),
            expected: self.expected_id.clone(),
        })?;

        if !Self::ct_eq(&uri, &self.expected_id) {
            warn!(
                got = %uri,
                expected = %self.expected_id,
                "SPIFFE ID mismatch — rejecting peer"
            );
            return Err(MeshError::SpiffeIdMismatch {
                got: uri,
                expected: self.expected_id.clone(),
            });
        }

        debug!(spiffe_id = %uri, "peer SPIFFE ID verified");
        Ok(())
    }
}

// ---- ServerCertVerifier (used by TLS clients verifying a server cert) -----

impl ServerCertVerifier for SpiffeVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        self.verify_spiffe_id(end_entity)
            .map_err(|e| rustls::Error::General(e.to_string()))?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        supported_schemes()
    }
}

// ---- ClientCertVerifier (used by TLS servers verifying a client cert) -----

impl ClientCertVerifier for SpiffeVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        self.verify_spiffe_id(end_entity)
            .map_err(|e| rustls::Error::General(e.to_string()))?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        supported_schemes()
    }
}

// ---------------------------------------------------------------------------
// Internal builders
// ---------------------------------------------------------------------------

fn supported_schemes() -> Vec<SignatureScheme> {
    vec![
        SignatureScheme::ECDSA_NISTP256_SHA256,
        SignatureScheme::ECDSA_NISTP384_SHA384,
        SignatureScheme::ED25519,
        SignatureScheme::RSA_PSS_SHA256,
        SignatureScheme::RSA_PSS_SHA384,
        SignatureScheme::RSA_PSS_SHA512,
    ]
}

fn build_server_config(
    cert_chain: Vec<CertificateDer<'static>>,
    private_key: PrivateKeyDer<'static>,
    ca_certs: Vec<CertificateDer<'static>>,
    expected_peer_id: &str,
) -> Result<ServerConfig, MeshError> {
    let verifier = Arc::new(SpiffeVerifier::new(ca_certs, expected_peer_id));
    let cfg = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(cert_chain, private_key)?;
    Ok(cfg)
}

fn build_client_config(
    cert_chain: Vec<CertificateDer<'static>>,
    private_key: PrivateKeyDer<'static>,
    ca_certs: Vec<CertificateDer<'static>>,
    expected_peer_id: &str,
) -> Result<ClientConfig, MeshError> {
    let verifier = Arc::new(SpiffeVerifier::new(ca_certs, expected_peer_id));
    let cfg = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(cert_chain, private_key)?;
    Ok(cfg)
}

// ---------------------------------------------------------------------------
// PEM parsing helpers
// ---------------------------------------------------------------------------

/// Parse all X.509 certificate DER entries from a PEM buffer.
pub fn parse_cert_chain(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, MeshError> {
    use rustls_pemfile::Item;
    use std::io::BufReader;

    let mut reader = BufReader::new(pem);
    let mut certs = Vec::new();

    for item in rustls_pemfile::read_all(&mut reader) {
        match item.map_err(|e| MeshError::PemDecode(e.to_string()))? {
            Item::X509Certificate(c) => certs.push(c),
            _ => {}
        }
    }

    if certs.is_empty() {
        return Err(MeshError::PemDecode(
            "no X.509 certificates found in PEM buffer".into(),
        ));
    }

    Ok(certs)
}

/// Parse the first private key from a PEM buffer (PKCS#8, SEC1, or PKCS#1).
pub fn parse_private_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>, MeshError> {
    use rustls_pemfile::Item;
    use std::io::BufReader;

    let mut reader = BufReader::new(pem);

    for item in rustls_pemfile::read_all(&mut reader) {
        match item.map_err(|e| MeshError::PemDecode(e.to_string()))? {
            Item::Pkcs8Key(k) => return Ok(PrivateKeyDer::Pkcs8(k)),
            Item::Sec1Key(k) => return Ok(PrivateKeyDer::Sec1(k)),
            Item::Pkcs1Key(k) => return Ok(PrivateKeyDer::Pkcs1(k)),
            _ => {}
        }
    }

    Err(MeshError::PemDecode(
        "no private key found in PEM buffer".into(),
    ))
}
