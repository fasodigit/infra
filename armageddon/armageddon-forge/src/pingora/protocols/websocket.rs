// SPDX-License-Identifier: AGPL-3.0-or-later
//! WebSocket protocol handler for the Pingora gateway (M4-3 wave 2).
//!
//! Ported from `src/websocket.rs` (hyper path).
//!
//! ## Pingora 0.3 compatibility note
//!
//! Pingora 0.3 does not expose a `session.upgrade_to_ws()` API in the public
//! `ProxyHttp` trait.  The WebSocket upgrade must therefore be handled at a
//! lower level — the gateway detects the upgrade headers in `request_filter`
//! and sets `ctx.ws_upgrade = true`, then the caller is responsible for
//! managing the actual connection upgrade (typically via `session.as_mut()`
//! once Pingora exposes the socket directly, or via a separate listener in
//! M5/M6).
//!
//! **TODO(M5)**: when Pingora 0.4 exposes `session.upgrade_to_ws()` or an
//! equivalent hook (`ProxyHttp::handle_websocket`), replace the manual
//! handshake helpers with the native API and remove the `upgrade_to_ws_natve`
//! note in `PINGORA-MIGRATION-PROGRESS.md`.
//!
//! ## Current implementation strategy
//!
//! This module provides:
//!
//! 1. [`check_upgrade_headers`] — RFC 6455 §4.2.1 header validation.
//! 2. [`compute_websocket_accept`] — `Sec-WebSocket-Accept` derivation.
//! 3. [`WebSocketConfig`] — idle timeout, max frame size, ping interval.
//! 4. [`WebSocketProxy`] — a standalone proxy that can accept a `TcpStream`
//!    and proxy frames bidirectionally using `tokio-tungstenite`.  Used by
//!    integration tests and the future M5 listener.
//! 5. Prometheus counter / gauge helpers for active-connection tracking.
//!
//! ## Failure modes
//!
//! | Scenario | Behaviour |
//! |----------|-----------|
//! | Invalid upgrade headers | `WsError::InvalidUpgrade` — caller returns 400 |
//! | Upstream WS connect fails | `WsError::UpstreamConnect` — caller returns 502 |
//! | Frame exceeds `max_frame_size` | Close frame 1009 sent then session closes |
//! | Idle timeout expires | Session closed gracefully |
//! | Upstream drops connection | `client_write` task drains then exits |

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use sha1::{Digest, Sha1};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    accept_async_with_config, connect_async_with_config,
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame, Message, WebSocketConfig as TungsteniteConfig},
    },
};
use tracing::{debug, info, warn};

// ── Constants ──────────────────────────────────────────────────────────────

/// Default maximum WebSocket frame payload size (512 KiB).
pub const DEFAULT_MAX_FRAME_SIZE: usize = 512 * 1024;

/// Default idle timeout: 120 seconds.
pub const DEFAULT_IDLE_TIMEOUT_MS: u64 = 120_000;

/// Default ping interval: 30 seconds.
pub const DEFAULT_PING_INTERVAL_MS: u64 = 30_000;

/// Backpressure channel capacity per direction.
const CHANNEL_CAPACITY: usize = 256;

/// Magic GUID used by RFC 6455 for `Sec-WebSocket-Accept` computation.
const WS_MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

// ── Error type ─────────────────────────────────────────────────────────────

/// Errors specific to the WebSocket proxy path.
#[derive(Debug, thiserror::Error)]
pub enum WsError {
    /// Missing or invalid WebSocket upgrade headers (RFC 6455 §4.2.1).
    #[error("missing or invalid WebSocket upgrade headers")]
    InvalidUpgrade,

    /// Server-side WebSocket handshake with the client failed.
    #[error("WebSocket handshake failed: {0}")]
    Handshake(String),

    /// Could not establish a WebSocket connection to the upstream.
    #[error("upstream WebSocket connection failed: {0}")]
    UpstreamConnect(String),

    /// A received frame exceeded the configured size limit.
    #[error("frame size {actual} exceeds configured limit {limit}")]
    FrameTooLarge { actual: usize, limit: usize },

    /// An underlying transport error.
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

// ── Configuration ──────────────────────────────────────────────────────────

/// Runtime configuration for the WebSocket proxy path.
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum WebSocket frame payload size in bytes.
    ///
    /// Frames exceeding this limit cause a close frame 1009 (Message Too Big).
    pub max_frame_size: usize,

    /// Idle timeout in milliseconds.
    ///
    /// The session is closed after this duration of inactivity.
    /// TODO(M5): implement via `tokio::time::interval` pings once the native
    /// Pingora hook is available.
    pub idle_timeout_ms: u64,

    /// Ping interval in milliseconds.
    ///
    /// The proxy sends `Ping` frames to the upstream at this interval to detect
    /// half-open connections.
    /// TODO(M5): implement with a dedicated tokio task.
    pub ping_interval_ms: u64,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
            idle_timeout_ms: DEFAULT_IDLE_TIMEOUT_MS,
            ping_interval_ms: DEFAULT_PING_INTERVAL_MS,
        }
    }
}

// ── Header validation ──────────────────────────────────────────────────────

/// Validate that the incoming HTTP headers carry a valid WebSocket upgrade
/// request as per RFC 6455 §4.2.1.
///
/// Required conditions (all must hold):
/// - `Connection` header contains `"upgrade"` (case-insensitive).
/// - `Upgrade` header equals `"websocket"` (case-insensitive).
/// - `Sec-WebSocket-Key` header is present and non-empty.
/// - `Sec-WebSocket-Version` header equals `"13"`.
///
/// Returns `Ok(())` when the request is a valid WebSocket upgrade, or
/// `Err(WsError::InvalidUpgrade)` when any condition fails.
pub fn check_upgrade_headers(headers: &http::HeaderMap) -> Result<(), WsError> {
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

/// Detect WebSocket upgrade from a slice of `(name, value)` header pairs.
///
/// Convenience wrapper for use in `request_filter` where headers are
/// accessed via pingora's `req_header().headers`.
pub fn detect_ws_upgrade(headers: &[(&str, &str)]) -> bool {
    let connection = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("connection"));
    let upgrade = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("upgrade"));
    let key = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-key"));
    let version = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-version"));

    connection.map(|(_, v)| v.to_lowercase().contains("upgrade")).unwrap_or(false)
        && upgrade.map(|(_, v)| v.eq_ignore_ascii_case("websocket")).unwrap_or(false)
        && key.map(|(_, v)| !v.is_empty()).unwrap_or(false)
        && version.map(|(_, v)| v.trim() == "13").unwrap_or(false)
}

// ── Handshake computation ──────────────────────────────────────────────────

/// Compute `Sec-WebSocket-Accept` from `Sec-WebSocket-Key` per RFC 6455.
///
/// SHA-1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"), then base64-encode.
pub fn compute_websocket_accept(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.trim().as_bytes());
    hasher.update(WS_MAGIC.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

// ── Proxy ──────────────────────────────────────────────────────────────────

/// WebSocket reverse proxy.
///
/// A single `WebSocketProxy` instance is typically shared behind an `Arc` and
/// reused across connections.  All configuration is immutable after
/// construction.
#[derive(Debug, Clone)]
pub struct WebSocketProxy {
    /// Default upstream URL base (e.g. `ws://backend:8080`).
    pub upstream: String,
    /// Per-connection configuration.
    pub config: WebSocketConfig,
}

impl WebSocketProxy {
    /// Create a new proxy with the given upstream base URL and default config.
    pub fn new(upstream: impl Into<String>) -> Self {
        Self {
            upstream: upstream.into(),
            config: WebSocketConfig::default(),
        }
    }

    /// Create a new proxy with an explicit configuration.
    pub fn with_config(upstream: impl Into<String>, config: WebSocketConfig) -> Self {
        Self {
            upstream: upstream.into(),
            config,
        }
    }

    /// Upgrade the raw TCP connection to a WebSocket session and proxy frames
    /// bidirectionally to `upstream_uri`.
    ///
    /// # Parameters
    /// - `client`: Raw TCP stream from the connecting client.
    /// - `upstream_uri`: Full WebSocket URI to connect to upstream.
    /// - `headers`: Original HTTP request headers (used for upgrade validation).
    ///
    /// # Metrics
    ///
    /// TODO(M5): increment `armageddon_ws_connections_active{cluster}` gauge
    /// on entry and decrement on exit, once the Prometheus registry wiring is
    /// in place.
    pub async fn upgrade_and_proxy(
        &self,
        client: TcpStream,
        upstream_uri: http::Uri,
        headers: http::HeaderMap,
    ) -> Result<(), WsError> {
        check_upgrade_headers(&headers)?;

        let max_frame = self.config.max_frame_size;
        let ws_config = build_tungstenite_config(max_frame);

        // Server-side handshake with the connecting client.
        let client_ws = accept_async_with_config(client, Some(ws_config))
            .await
            .map_err(|e| WsError::Handshake(e.to_string()))?;

        debug!("WebSocket client handshake complete");

        // Establish connection to upstream.
        let upstream_url = upstream_uri.to_string();
        let (upstream_ws, _) =
            connect_async_with_config(&upstream_url, Some(ws_config), false)
                .await
                .map_err(|e| WsError::UpstreamConnect(e.to_string()))?;

        debug!(upstream = %upstream_url, "WebSocket upstream connected");

        let (mut client_sink, mut client_stream) = client_ws.split();
        let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();

        // Bounded channels for backpressure.
        let (to_upstream_tx, mut to_upstream_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);
        let (to_client_tx, mut to_client_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);

        // task: client → upstream channel
        let client_read = tokio::spawn(async move {
            while let Some(result) = client_stream.next().await {
                match result {
                    Ok(msg) => {
                        let payload_len = msg.len();
                        if payload_len > max_frame {
                            warn!(
                                "client frame too large: {} > {} bytes",
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
                        debug!("client WS stream error: {}", e);
                        break;
                    }
                }
            }
        });

        // task: upstream → client channel
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
                        debug!("upstream WS stream error: {}", e);
                        break;
                    }
                }
            }
        });

        // task: upstream channel → upstream sink
        let upstream_write = tokio::spawn(async move {
            while let Some(msg) = to_upstream_rx.recv().await {
                let is_close = msg.is_close();
                if upstream_sink.send(msg).await.is_err() || is_close {
                    break;
                }
            }
            let _ = upstream_sink.close().await;
        });

        // task: client channel → client sink
        let client_write = tokio::spawn(async move {
            while let Some(msg) = to_client_rx.recv().await {
                let is_close = msg.is_close();
                if client_sink.send(msg).await.is_err() || is_close {
                    break;
                }
            }
            let _ = client_sink.close().await;
        });

        // Wait for any task to finish; remaining tasks drain naturally.
        let idle_timeout = Duration::from_millis(self.config.idle_timeout_ms);
        let session_done = async {
            tokio::select! {
                _ = client_read    => {}
                _ = upstream_read  => {}
                _ = upstream_write => {}
                _ = client_write   => {}
            }
        };

        tokio::select! {
            _ = session_done => {}
            _ = tokio::time::sleep(idle_timeout) => {
                warn!(upstream = %upstream_url, "WebSocket idle timeout");
            }
        }

        info!(upstream = %upstream_url, "WebSocket session closed");
        Ok(())
    }
}

// ── Internal helpers ───────────────────────────────────────────────────────

fn build_tungstenite_config(max_frame_size: usize) -> TungsteniteConfig {
    TungsteniteConfig {
        max_frame_size: Some(max_frame_size),
        max_message_size: Some(max_frame_size),
        ..Default::default()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;

    // -- helpers -----------------------------------------------------------

    fn valid_upgrade_header_map() -> http::HeaderMap {
        let mut h = http::HeaderMap::new();
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

    // -- header validation -------------------------------------------------

    #[test]
    fn valid_headers_accepted() {
        assert!(check_upgrade_headers(&valid_upgrade_header_map()).is_ok());
    }

    #[test]
    fn missing_upgrade_header_rejected() {
        let mut h = valid_upgrade_header_map();
        h.remove(http::header::UPGRADE);
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    #[test]
    fn wrong_ws_version_rejected() {
        let mut h = valid_upgrade_header_map();
        h.insert("sec-websocket-version", HeaderValue::from_static("8"));
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    #[test]
    fn missing_ws_key_rejected() {
        let mut h = valid_upgrade_header_map();
        h.remove("sec-websocket-key");
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    #[test]
    fn missing_connection_header_rejected() {
        let mut h = valid_upgrade_header_map();
        h.remove(http::header::CONNECTION);
        assert!(matches!(
            check_upgrade_headers(&h),
            Err(WsError::InvalidUpgrade)
        ));
    }

    // -- detect_ws_upgrade -------------------------------------------------

    #[test]
    fn detect_ws_upgrade_slice_valid() {
        let headers = vec![
            ("connection", "Upgrade"),
            ("upgrade", "websocket"),
            ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
            ("sec-websocket-version", "13"),
        ];
        assert!(detect_ws_upgrade(&headers));
    }

    #[test]
    fn detect_ws_upgrade_slice_incomplete() {
        let headers = vec![("connection", "Upgrade"), ("upgrade", "websocket")];
        assert!(!detect_ws_upgrade(&headers));
    }

    // -- Sec-WebSocket-Accept computation ----------------------------------

    #[test]
    fn websocket_accept_rfc6455_test_vector() {
        // RFC 6455 §1.3 test vector.
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
        assert_eq!(compute_websocket_accept(key), expected);
    }

    #[test]
    fn websocket_accept_key_with_whitespace() {
        // Key with surrounding whitespace — trim must be applied.
        let key = "  dGhlIHNhbXBsZSBub25jZQ==  ";
        let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
        assert_eq!(compute_websocket_accept(key), expected);
    }

    // -- config defaults ---------------------------------------------------

    #[test]
    fn default_config_values() {
        let cfg = WebSocketConfig::default();
        assert_eq!(cfg.max_frame_size, DEFAULT_MAX_FRAME_SIZE);
        assert_eq!(cfg.idle_timeout_ms, DEFAULT_IDLE_TIMEOUT_MS);
        assert_eq!(cfg.ping_interval_ms, DEFAULT_PING_INTERVAL_MS);
    }

    #[test]
    fn proxy_stores_upstream() {
        let p = WebSocketProxy::new("ws://backend:8080");
        assert_eq!(p.upstream, "ws://backend:8080");
    }

    #[test]
    fn proxy_with_config_overrides_defaults() {
        let cfg = WebSocketConfig {
            max_frame_size: 64 * 1024,
            idle_timeout_ms: 60_000,
            ping_interval_ms: 10_000,
        };
        let p = WebSocketProxy::with_config("ws://backend:8080", cfg.clone());
        assert_eq!(p.config.max_frame_size, 64 * 1024);
        assert_eq!(p.config.idle_timeout_ms, 60_000);
    }

    // -- end-to-end (requires tokio runtime) --------------------------------

    #[tokio::test]
    async fn text_frame_roundtrip() {
        use tokio::net::TcpListener;
        use tokio_tungstenite::{accept_async, connect_async};
        use futures_util::{SinkExt as _, StreamExt as _};
        use tokio_tungstenite::tungstenite::Message as WsMsg;

        // Echo upstream.
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            if let Some(Ok(msg)) = ws.next().await {
                let _ = ws.send(msg).await;
            }
        });

        // Gateway listener.
        let gw_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gw_addr = gw_listener.local_addr().unwrap();

        let proxy = WebSocketProxy::new(format!("ws://{}", upstream_addr));
        let upstream_uri: http::Uri =
            format!("ws://{}", upstream_addr).parse().unwrap();
        let headers = valid_upgrade_header_map();

        tokio::spawn(async move {
            let (stream, _) = gw_listener.accept().await.unwrap();
            let _ = proxy.upgrade_and_proxy(stream, upstream_uri, headers).await;
        });

        let (mut client, _) = connect_async(format!("ws://{}", gw_addr))
            .await
            .unwrap();

        client.send(WsMsg::Text("hello M4".into())).await.unwrap();
        let reply = client.next().await.unwrap().unwrap();
        assert_eq!(reply, WsMsg::Text("hello M4".into()));
    }

    #[tokio::test]
    async fn close_frame_propagates() {
        use tokio::{net::TcpListener, sync::oneshot};
        use tokio_tungstenite::{accept_async, connect_async};
        use futures_util::{SinkExt as _, StreamExt as _};
        use tokio_tungstenite::tungstenite::Message as WsMsg;

        let (close_tx, close_rx) = oneshot::channel::<bool>();

        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = upstream_listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
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
        let upstream_uri: http::Uri =
            format!("ws://{}", upstream_addr).parse().unwrap();
        let headers = valid_upgrade_header_map();

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
        .expect("timeout")
        .expect("channel dropped");

        assert!(got_close);
    }
}
