// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Error types for the ARMAGEDDON mTLS mesh.
//!
//! # Failure modes
//!
//! - **`Spiffe`** — the SPIRE workload-API socket is unreachable or the gRPC
//!   stream is severed.  [`SvidManager`] backs off and retries; during the
//!   retry window the previous SVID stays active until expiry.
//!
//! - **`SvidExpired`** — the in-memory SVID has passed its `not_after`
//!   timestamp.  ARMAGEDDON refuses new mTLS connections until SPIRE delivers
//!   a fresh SVID.
//!
//! - **`SpiffeIdMismatch`** — the peer's X.509 URI SAN does not match the
//!   expected SPIFFE ID pattern for the target workload.  All connections with
//!   this error MUST be rejected; there is no fallback.
//!
//! - **`Rustls`** — a rustls configuration error (bad PEM, unsupported cipher).
//!
//! - **`Io`** — low-level I/O error when connecting to the SPIRE Unix socket.
//!
//! - **`PemDecode`** — PEM material is structurally valid but the DER inside
//!   cannot be interpreted as the expected X.509 or PKCS8 type.

use thiserror::Error;

/// Unified error type for `armageddon-mesh`.
#[derive(Debug, Error)]
pub enum MeshError {
    /// Error originating from the `spiffe` workload-API client (gRPC or
    /// socket-level failure).
    #[error("SPIFFE/SPIRE error: {0}")]
    Spiffe(String),

    /// Error originating from rustls configuration or certificate parsing.
    #[error("rustls error: {0}")]
    Rustls(String),

    /// Low-level I/O error (e.g. socket file not found).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The cached SVID has expired and no renewal has arrived yet.
    #[error("SVID has expired; refusing to serve mTLS until renewed by SPIRE")]
    SvidExpired,

    /// The peer's SPIFFE URI SAN does not match the expected trust domain or
    /// service-account path.
    #[error("SPIFFE ID mismatch: got `{got}`, expected pattern `{expected}`")]
    SpiffeIdMismatch { got: String, expected: String },

    /// The rotation broadcast channel was dropped before shutdown.
    #[error("rotation broadcast channel closed")]
    ChannelClosed,

    /// PEM material could not be decoded.
    #[error("PEM decode error: {0}")]
    PemDecode(String),
}

impl From<rustls::Error> for MeshError {
    fn from(e: rustls::Error) -> Self {
        MeshError::Rustls(e.to_string())
    }
}
