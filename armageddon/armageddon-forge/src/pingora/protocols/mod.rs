// SPDX-License-Identifier: AGPL-3.0-or-later
//! Protocol-specific sub-features of the Pingora gateway.
//!
//! | Sub-module        | Replaces (hyper path)           | Status         |
//! |-------------------|---------------------------------|----------------|
//! | `compression`     | `src/compression.rs`            | M4-1 wired ✓   |
//! | `grpc_web`        | `src/grpc_web.rs`               | M4-2 ported ✓  |
//! | `websocket`       | `src/websocket.rs`              | M4-3 ported ✓  |
//! | `traffic_split`   | `src/traffic_split.rs`          | M4-4 ported ✓  |

pub mod compression;
pub mod grpc_web;
pub mod traffic_split;
pub mod websocket;

pub use compression::{
    CompressionFilter, CompressionLevel, CompressionStream, Encoding, NegotiationOutcome,
};

pub use grpc_web::{
    GrpcWebConfig, GrpcWebError, GrpcWebVariant,
    assemble_grpc_web_body, build_grpc_frame, build_trailer_frame,
    decode_grpc_web_text_body, detect_grpc_web, grpc_web_cors_expose_headers,
    is_grpc_web_preflight, parse_grpc_frame, parse_trailer_payload,
    upstream_grpc_content_type,
};

pub use traffic_split::{
    SplitDecision, SplitDecisionMode, SplitError, SplitMode, SplitSpec,
    TrafficSplitter, Variant, decide_with,
};

pub use websocket::{
    WebSocketConfig, WebSocketProxy, WsError,
    check_upgrade_headers, compute_websocket_accept, detect_ws_upgrade,
    DEFAULT_IDLE_TIMEOUT_MS, DEFAULT_MAX_FRAME_SIZE, DEFAULT_PING_INTERVAL_MS,
};
