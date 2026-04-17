// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Error types for the armageddon-xds ADS consumer.

use thiserror::Error;

/// All errors that can be returned by the xDS ADS consumer.
#[derive(Debug, Error)]
pub enum XdsError {
    /// Failed to establish a gRPC channel to the control plane.
    #[error("xDS connection failed to '{endpoint}': {source}")]
    Connection {
        endpoint: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    /// The bidirectional ADS stream was broken (network, server restart, etc.).
    /// `run()` will catch this and trigger reconnection.
    #[error("xDS ADS stream broken: {0}")]
    StreamBroken(#[from] tonic::Status),

    /// A `google.protobuf.Any` resource could not be decoded into its expected
    /// proto message.  The per-type version is NOT advanced and a NACK is sent.
    #[error("failed to decode xDS resource (type_url={type_url}): {source}")]
    DecodeFailure {
        type_url: String,
        #[source]
        source: prost::DecodeError,
    },

    /// The resource type_url in a DiscoveryResponse is unknown or unsupported.
    #[error("unsupported xDS resource type_url: {0}")]
    UnsupportedResourceType(String),

    /// No DiscoveryResponse received within the 30 s idle timeout.
    #[error("xDS ADS stream idle timeout after {secs}s")]
    IdleTimeout { secs: u64 },

    /// A callback returned an error during resource application.
    #[error("xDS callback failed: {0}")]
    CallbackError(String),
}
