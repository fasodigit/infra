// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! WebSocket upgrade and bidirectional proxy for ARMAGEDDON FORGE.
//!
//! Handles the full WebSocket lifecycle:
//! 1. Validates the HTTP upgrade handshake headers from the caller.
//! 2. Performs the server-side WebSocket handshake with the connecting client.
//! 3. Opens a WebSocket connection to the upstream backend.
//! 4. Forwards frames bidirectionally with backpressure via bounded channels.
//! 5. Propagates graceful Close frames with the original reason code.
//!
//! # Note on integration
//! `upgrade_and_proxy` receives a **raw** `TcpStream` that has **not** yet
//! been upgraded.  The caller (router / network layer) is responsible for
//! detecting the `Upgrade: websocket` request and handing the raw stream here.
//! The HTTP 101 response is sent internally by `tokio-tungstenite`.

use futures_util::{SinkExt, StreamExt};
use http::{HeaderMap, Uri};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    accept_async_with_config, connect_async_with_config,
    tungstenite::protocol::{
        frame::coding::CloseCode, CloseFrame, Message, WebSocketConfig,
    },
};
use tracing::{debug, info, warn};

// -- constants --

/// Default maximum WebSocket frame payload size (512 KiB).
pub const DEFAULT_MAX_FRAME_SIZE: usize = 512 * 1024;

/// Capacity of the per-direction bounded channel used for backpressure.
const CHANNEL_CAPACITY: usize = 256;

// -- errors --

/// Errors specific to the WebSocket proxy path.
#[derive(Error, Debug)]
pub enum WsError {
    /// The incoming HTTP headers do not constitute a valid WebSocket upgrade.
    #[error("missing or invalid WebSocket upgrade headers")]
    InvalidUpgrade,

    /// The server-side WebSocket handshake with the client failed.
    #[error("WebSocket handshake failed: {0}")]
    Handshake(String),

    /// Could not establish a WebSocket connection to the upstream.
    #[error("upstream WebSocket connection failed: {0}")]
    UpstreamConnect(String),

    /// A received frame exceeded the configured size limit.
    #[error("frame size {actual} exceeds configured limit {limit}")]
    FrameTooLarge { actual: usize, limit: usize },

    /// An underlying transport error occurred.
    #[error("transport error: {0}")]
    Transport(String),

    /// A raw I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<tokio_tungstenite::tungstenite::Error> for WsError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        WsError::Transport(e.to_string())
    }
}

// -- types --

/// WebSocket reverse-proxy.
///
/// A single `WebSocketProxy` instance is typically shared behind an `Arc` and
/// reused across connections.  All fields are immutable after construction.
#[derive(Debug, Clone)]
pub struct WebSocketProxy {
    /// Default upstream URL base; callers may override per-request via the
    /// `upstream_uri` argument to `upgrade_and_proxy`.
    pub upstream: String,
    /// Maximum accepted WebSocket frame payload size in bytes.
    ///
    /// Frames that exceed this limit cause the session to be closed with
    /// close code 1009 (Message Too Big).  Defaults to [`DEFAULT_MAX_FRAME_SIZE`].
    pub max_frame_size: usize,
}

impl WebSocketProxy {
    /// Create a new proxy with the given upstream base URL and the default
    /// maximum frame size (512 KiB).
    pub fn new(upstream: impl Into<String>) -> Self {
        Self {
            upstream: upstream.into(),
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
        }
    }

    /// Create a new proxy with an explicit `max_frame_size`.
    pub fn with_max_frame_size(upstream: impl Into<String>, max_frame_size: usize) -> Self {
        Self {
            upstream: upstream.into(),
            max_frame_size,
        }
    }

    /// Upgrade the raw TCP connection to a WebSocket session and proxy frames
    /// bidirectionally to `upstream_uri`.
    ///
    /// The caller must have already validated (or confirmed) the upgrade intent
    /// via [`check_upgrade_headers`] before calling this method.  The HTTP 101
    /// switching-protocols response is sent by `tokio-tungstenite` internally.
    ///
    /// # Parameters
    /// - `client`:       Raw TCP stream from the connecting client.
    /// - `upstream_uri`: Full WebSocket URI to connect to upstream, e.g.
    ///                   `"ws://backend:8080/ws/path"`.
    /// - `headers`:      Original HTTP request headers (informational; not
    ///                   forwarded automatically in this L7 proxy path).
    pub async fn upgrade_and_proxy(
        &self,
        client: TcpStream,
        upstream_uri: Uri,
        headers: HeaderMap,
    ) -> Result<(), WsError> {
        // Validate upgrade headers (guard; callers should check first).
        check_upgrade_headers(&headers)?;

        let max_frame = self.max_frame_size;
        let ws_config = build_ws_config(max_frame);

        // -- server-side handshake with the connecting client --
        let client_ws = accept_async_with_config(client, Some(ws_config))
            .await
            .map_err(|e| WsError::Handshake(e.to_string()))?;

        debug!("WebSocket client handshake complete");

        // -- establish WebSocket connection to upstream --
        let upstream_url = upstream_uri.to_string();
        let (upstream_ws, _) = connect_async_with_config(&upstream_url, Some(ws_config), false)
            .await
            .map_err(|e| WsError::UpstreamConnect(e.to_string()))?;

        debug!("WebSocket upstream connection established: {}", upstream_url);

        // -- split into sink + stream halves --
        let (mut client_sink, mut client_stream) = client_ws.split();
        let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();

        // Bounded channels for backpressure in each direction.
        let (to_upstream_tx, mut to_upstream_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);
        let (to_client_tx, mut to_client_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);

        // -- task: client stream → to_upstream channel --
        let client_read = tokio::spawn(async move {
            while let Some(result) = client_stream.next().await {
                match result {
                    Ok(msg) => {
                        let payload_len = msg.len();
                        if payload_len > max_frame {
                            warn!(
                                "client WebSocket frame too large: {} > {} bytes",
                                payload_len, max_frame
                            );
                            let _ = to_upstream_tx
                                .send(Message::Close(Some(CloseFrame {
                                    code: CloseCode::Size,
                                    reason: "frame too large".into(),
                                })))
                                .await;
                            break;
                        }
                        let is_close = msg.is_close();
                        if to_upstream_tx.send(msg).await.is_err() || is_close {
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("client WebSocket stream error: {}", e);
                        break;
                    }
                }
            }
        });

        // -- task: upstream stream → to_client channel --
        let upstream_read = tokio::spawn(async move {
            while let Some(result) = upstream_stream.next().await {
                match result {
                    Ok(msg) => {
                        let is_close = msg.is_close();
                        if to_client_tx.send(msg).await.is_err() || is_close {
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("upstream WebSocket stream error: {}", e);
                        break;
                    }
                }
            }
        });

        // -- task: to_upstream channel → upstream sink --
        let upstream_write = tokio::spawn(async move {
            while let Some(msg) = to_upstream_rx.recv().await {
                let is_close = msg.is_close();
                if upstream_sink.send(msg).await.is_err() || is_close {
                    break;
                }
            }
            let _ = upstream_sink.close().await;
        });

        // -- task: to_client channel → client sink --
        let client_write = tokio::spawn(async move {
            while let Some(msg) = to_client_rx.recv().await {
                let is_close = msg.is_close();
                if client_sink.send(msg).await.is_err() || is_close {
                    break;
                }
            }
            let _ = client_sink.close().await;
        });

        // Wait for any task to finish; the others will drain naturally.
        tokio::select! {
            _ = client_read    => {}
            _ = upstream_read  => {}
            _ = upstream_write => {}
            _ = client_write   => {}
        }

        info!("WebSocket session closed (upstream: {})", upstream_url);
        Ok(())
    }
}

// -- helpers --

/// Validate that the incoming HTTP headers carry a valid WebSocket upgrade
/// request as per RFC 6455 §4.2.1.
///
/// Required conditions:
/// - `Connection` header contains `"upgrade"` (case-insensitive).
/// - `Upgrade` header equals `"websocket"` (case-insensitive).
/// - `Sec-WebSocket-Key` header is present and non-empty.
/// - `Sec-WebSocket-Version` header equals `"13"`.
pub fn check_upgrade_headers(headers: &HeaderMap) -> Result<(), WsError> {
    let connection_ok = headers
        .get(http::header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("upgrade"))
        .unwrap_or(false);

    let upgrade_ok = headers
        .get(http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase() == "websocket")
        .unwrap_or(false);

    let key_ok = headers
        .get("sec-websocket-key")
        .map(|v| !v.as_bytes().is_empty())
        .unwrap_or(false);

    let version_ok = headers
        .get("sec-websocket-version")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim() == "13")
        .unwrap_or(false);

    if connection_ok && upgrade_ok && key_ok && version_ok {
        Ok(())
    } else {
        Err(WsError::InvalidUpgrade)
    }
}

/// Build a [`WebSocketConfig`] that enforces the given `max_frame_size`.
fn build_ws_config(max_frame_size: usize) -> WebSocketConfig {
    WebSocketConfig {
        max_frame_size: Some(max_frame_size),
        max_message_size: Some(max_frame_size),
        ..Default::default()
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message as WsMsg};
    use futures_util::{SinkExt, StreamExt};

    // -- shared test helper --

    fn valid_upgrade_headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            http::header::CONNECTION,
            HeaderValue::from_static("Upgrade"),
        );
        h.insert(
            http::header::UPGRADE,
            HeaderValue::from_static("websocket"),
        );
        h.insert(
            "sec-websocket-key",
            HeaderValue::from_static("dGhlIHNhbXBsZSBub25jZQ=="),
        );
        h.insert("sec-websocket-version", HeaderValue::from_static("13"));
        h
    }

    // -----------------------------------------------------------------
    // Test 1 — valid upgrade headers are accepted
    // -----------------------------------------------------------------
    #[test]
    fn test_check_upgrade_headers_valid() {
        assert!(check_upgrade_headers(&valid_upgrade_headers()).is_ok());
    }

    // -----------------------------------------------------------------
    // Test 2 — missing Upgrade header → InvalidUpgrade
    // -----------------------------------------------------------------
    #[test]
    fn test_check_upgrade_headers_missing_upgrade() {
        let mut h = valid_upgrade_headers();
        h.remove(http::header::UPGRADE);
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    // -----------------------------------------------------------------
    // Test 3 — wrong Sec-WebSocket-Version → InvalidUpgrade
    // -----------------------------------------------------------------
    #[test]
    fn test_check_upgrade_headers_wrong_version() {
        let mut h = valid_upgrade_headers();
        h.insert("sec-websocket-version", HeaderValue::from_static("8"));
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    // -----------------------------------------------------------------
    // Test 4 — missing Sec-WebSocket-Key → InvalidUpgrade
    // -----------------------------------------------------------------
    #[test]
    fn test_check_upgrade_headers_missing_key() {
        let mut h = valid_upgrade_headers();
        h.remove("sec-websocket-key");
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    // -----------------------------------------------------------------
    // Test 5 — text frame roundtrip: client → gateway → upstream → client
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_websocket_text_roundtrip() {
        // Fake upstream: echo the first received message back.
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            if let Some(Ok(msg)) = ws.next().await {
                let _ = ws.send(msg).await;
            }
        });

        // Fake gateway listener.
        let gw_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gw_addr = gw_listener.local_addr().unwrap();

        let proxy = WebSocketProxy::new(format!("ws://{}", upstream_addr));
        let upstream_uri: Uri = format!("ws://{}", upstream_addr).parse().unwrap();
        let headers = valid_upgrade_headers();

        tokio::spawn(async move {
            let (stream, _) = gw_listener.accept().await.unwrap();
            let _ = proxy.upgrade_and_proxy(stream, upstream_uri, headers).await;
        });

        let (mut client, _) = connect_async(format!("ws://{}", gw_addr))
            .await
            .expect("client connect failed");

        client.send(WsMsg::Text("hello ARMAGEDDON".into())).await.unwrap();
        let reply = client.next().await.unwrap().unwrap();
        assert_eq!(reply, WsMsg::Text("hello ARMAGEDDON".into()));
    }

    // -----------------------------------------------------------------
    // Test 6 — binary frame 100 KB passes through without truncation
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_websocket_binary_100kb() {
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            if let Some(Ok(msg)) = ws.next().await {
                let _ = ws.send(msg).await;
            }
        });

        let gw_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gw_addr = gw_listener.local_addr().unwrap();

        let proxy = WebSocketProxy::new(format!("ws://{}", upstream_addr));
        let upstream_uri: Uri = format!("ws://{}", upstream_addr).parse().unwrap();
        let headers = valid_upgrade_headers();

        tokio::spawn(async move {
            let (stream, _) = gw_listener.accept().await.unwrap();
            let _ = proxy.upgrade_and_proxy(stream, upstream_uri, headers).await;
        });

        let (mut client, _) = connect_async(format!("ws://{}", gw_addr))
            .await
            .unwrap();

        let payload: bytes::Bytes = bytes::Bytes::from(vec![0xABu8; 100 * 1024]);
        client.send(WsMsg::Binary(payload.to_vec())).await.unwrap();
        let reply = client.next().await.unwrap().unwrap();
        match reply {
            WsMsg::Binary(data) => assert_eq!(data.len(), 100 * 1024),
            other => panic!("unexpected message type: {:?}", other),
        }
    }

    // -----------------------------------------------------------------
    // Test 7 — Close frame from client propagates to upstream
    // -----------------------------------------------------------------
    #[tokio::test]
    async fn test_websocket_close_propagation() {
        use tokio::sync::oneshot;

        let (close_tx, close_rx) = oneshot::channel::<bool>();

        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            // Drain until we see a Close or the connection drops.
            while let Some(result) = ws.next().await {
                match result {
                    Ok(WsMsg::Close(_)) | Err(_) => {
                        let _ = close_tx.send(true);
                        break;
                    }
                    _ => {}
                }
            }
        });

        let gw_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gw_addr = gw_listener.local_addr().unwrap();

        let proxy = WebSocketProxy::new(format!("ws://{}", upstream_addr));
        let upstream_uri: Uri = format!("ws://{}", upstream_addr).parse().unwrap();
        let headers = valid_upgrade_headers();

        tokio::spawn(async move {
            let (stream, _) = gw_listener.accept().await.unwrap();
            let _ = proxy.upgrade_and_proxy(stream, upstream_uri, headers).await;
        });

        let (mut client, _) = connect_async(format!("ws://{}", gw_addr))
            .await
            .unwrap();

        client
            .send(WsMsg::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: "bye".into(),
            })))
            .await
            .unwrap();

        let got_close = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            close_rx,
        )
        .await
        .expect("timeout waiting for close propagation")
        .expect("channel dropped");

        assert!(got_close, "upstream did not receive close frame");
    }
}
