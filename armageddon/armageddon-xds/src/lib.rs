// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Active xDS v3 ADS consumer.
//!
//! Opens a single bidirectional gRPC stream toward the FASO `xds-controller`
//! and receives all resource types (CDS / EDS / LDS / RDS / SDS) over it.
//!
//! # Architecture
//!
//! ```text
//! AdsClient::run()
//!   │
//!   ├── send DiscoveryRequest  (type_url + resource_names + ACK/NACK nonce)
//!   │
//!   └── recv DiscoveryResponse
//!         ├── decode google.protobuf.Any  →  typed resource
//!         ├── invoke XdsCallback::{on_cluster,on_endpoint,on_listener,on_route,on_secret}_update
//!         └── send ACK  (or NACK on decode failure, version_info NOT advanced)
//! ```
//!
//! # Failure modes
//!
//! * **Leader / control-plane loss**: `run()` catches stream errors and reconnects
//!   with exponential back-off (100 ms base, 32 s cap).  On reconnect the client
//!   re-sends the last ACK'd `version_info` and `nonce` so the server can resume
//!   from the correct point; no duplicate callbacks are fired for already-applied
//!   versions.
//!
//! * **Malformed resource**: `prost::Message::decode` failure triggers a NACK.
//!   The per-type `version_info` is NOT advanced; the previous version remains
//!   active.  A subsequent push from the server will be decoded fresh.
//!
//! * **Network partition (no response for 30 s)**: the stream is torn down and
//!   reconnect logic takes over.  A Prometheus counter
//!   `xds_stream_timeout_total{type="ads"}` is incremented.
//!
//! * **Duplicate responses** (same version + nonce): `Subscription::is_duplicate`
//!   returns `true`, the callback is skipped, and an ACK is still sent so the
//!   server does not stall.

pub mod ads_client;
pub mod debouncer;
pub mod error;
pub mod metrics;
pub mod mtls;
pub mod proto;
pub mod resources;
pub mod subscription;

#[cfg(test)]
mod tests;

pub use ads_client::{AdsClient, XdsCallback};
pub use error::XdsError;
pub use mtls::{
    InlineSvid, MtlsError, SvidSource, XdsMtlsConfig, XdsServerMtlsConfig,
    build_channel, inc_handshake, inc_reconnect, is_authorized_client,
    mtls_handshakes, mtls_reconnects,
};
pub use resources::ResourceCache;
pub use subscription::Subscription;
