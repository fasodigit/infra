// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axum WebSocket handler for `/ws/sync/{producer_id}`.
//!
//! # Path
//! `/ws/sync/{producer_id}` — note: ARMAGEDDON's external route is
//! `/ws/terroir/sync` and proxies to the `terroir_mobile_bff` cluster after
//! stripping the `/ws/terroir/` prefix (and `/api/terroir/mobile-bff/` for
//! REST). This handler matches the path shape that ARMAGEDDON forwards.
//!
//! # Auth
//! `Sec-WebSocket-Protocol: bearer.<jwt>` — same pattern used elsewhere in
//! the FASO stack. The handler:
//!   1. Parses the protocol header, extracts the token after `bearer.`.
//!   2. Validates the JWT via `tenant_context::extract_from_jwt`.
//!   3. On success, accepts the upgrade and echoes the same protocol back
//!      (RFC 6455 requires the server to confirm the chosen sub-protocol).
//!   4. Registers the connection in the `WsRegistry`.
//!
//! # Lifecycle
//! - Inbound text frames are parsed as `WsFrame`. `Ping` → `Pong`,
//!   `YjsUpdate` → broadcast to other tenant clients. (Persist + merge will
//!   be wired to terroir-core gRPC in P1.E once a write-side polygon RPC
//!   exists; the proto today only has `GetParcelPolygon`.)
//! - Heartbeat: every `WS_HEARTBEAT_SECS` (30s) the writer task sends a
//!   `Message::Ping(vec![])`. If no inbound frame is seen within
//!   `WS_IDLE_TIMEOUT_SECS` (5min), the reader times out and closes.

use std::sync::Arc;

use axum::{
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header::SEC_WEBSOCKET_PROTOCOL},
    response::IntoResponse,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    dto::WsFrame,
    state::AppState,
    tenant_context::{TenantContext, extract_from_jwt},
    ws::registry::ConnId,
};

/// `GET /ws/sync/{producer_id}` — upgrade.
pub async fn ws_sync_handler(
    Path(producer_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Extract `bearer.<jwt>` from Sec-WebSocket-Protocol.
    let proto = headers
        .get(SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = proto
        .split(',')
        .map(str::trim)
        .find_map(|p| p.strip_prefix("bearer."));

    let Some(token) = token else {
        warn!(producer_id = %producer_id, "WS upgrade rejected: missing bearer.<jwt> in Sec-WebSocket-Protocol");
        return (StatusCode::UNAUTHORIZED, "missing bearer token").into_response();
    };

    let tenant = match extract_from_jwt(token, &state).await {
        Ok(t) => t,
        Err(e) => {
            warn!(producer_id = %producer_id, error = %e, "WS upgrade rejected: invalid JWT");
            return (StatusCode::UNAUTHORIZED, format!("invalid token: {e}")).into_response();
        }
    };

    info!(
        tenant = %tenant.slug,
        user_id = %tenant.user_id,
        producer_id = %producer_id,
        "WS upgrade accepted"
    );

    // Confirm the chosen sub-protocol on the response (RFC 6455 §1.9).
    let chosen_proto = format!("bearer.{token}");
    ws.protocols([chosen_proto])
        .on_upgrade(move |socket| handle_socket(socket, state, tenant, producer_id))
        .into_response()
}

/// Per-connection task — split the socket, spawn writer + heartbeat, run reader.
async fn handle_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    tenant: TenantContext,
    _producer_id: Uuid,
) {
    let conn_id: ConnId = state.ws_registry.next_conn_id();

    let (mut sender, mut receiver) = socket.split();

    // Outbound channel — registry pushes (broadcast or self) drain into this.
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    state
        .ws_registry
        .insert(&tenant.slug, &tenant.user_id, conn_id, tx);

    // Writer task — reads from rx, pushes to socket.
    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
        let _ = sender.close().await;
    });

    // Heartbeat task — push a Ping into our own outbound channel every 30s.
    let registry_hb = state.ws_registry.clone();
    let tenant_slug_hb = tenant.slug.clone();
    let user_id_hb = tenant.user_id.clone();
    let heartbeat_handle = tokio::spawn(async move {
        let mut tick =
            tokio::time::interval(std::time::Duration::from_secs(crate::WS_HEARTBEAT_SECS));
        tick.tick().await; // skip immediate
        loop {
            tick.tick().await;
            let alive = registry_hb.push_to(
                &tenant_slug_hb,
                &user_id_hb,
                conn_id,
                Message::Ping(Vec::<u8>::new().into()),
            );
            if !alive {
                break;
            }
        }
    });

    // Reader loop — bounded by WS_IDLE_TIMEOUT_SECS via tokio::time::timeout.
    let idle = std::time::Duration::from_secs(crate::WS_IDLE_TIMEOUT_SECS);
    loop {
        let next = tokio::time::timeout(idle, receiver.next()).await;
        let msg = match next {
            Ok(Some(Ok(m))) => m,
            Ok(Some(Err(e))) => {
                debug!(error = %e, "WS reader: stream error");
                break;
            }
            Ok(None) => {
                debug!("WS reader: stream closed");
                break;
            }
            Err(_) => {
                debug!(
                    tenant = %tenant.slug,
                    user_id = %tenant.user_id,
                    conn_id,
                    "WS reader: idle timeout"
                );
                break;
            }
        };

        match msg {
            Message::Text(txt) => {
                handle_text_frame(&state, &tenant, conn_id, &txt).await;
            }
            Message::Binary(_) => {
                debug!("ignoring binary WS frame (text-only protocol in P1.D)");
            }
            Message::Ping(payload) => {
                debug!(payload_len = payload.len(), "received Ping");
            }
            Message::Pong(_) => {
                debug!("received Pong");
            }
            Message::Close(_) => {
                debug!("received Close");
                break;
            }
        }
    }

    // Cleanup.
    state
        .ws_registry
        .remove(&tenant.slug, &tenant.user_id, conn_id);
    heartbeat_handle.abort();
    let _ = writer_handle.await;
    info!(
        tenant = %tenant.slug,
        user_id = %tenant.user_id,
        conn_id,
        "WS connection closed"
    );
}

/// Parse + dispatch one inbound text frame.
async fn handle_text_frame(
    state: &Arc<AppState>,
    tenant: &TenantContext,
    conn_id: ConnId,
    txt: &str,
) {
    let frame: WsFrame = match serde_json::from_str(txt) {
        Ok(f) => f,
        Err(e) => {
            push_self(
                state,
                tenant,
                conn_id,
                WsFrame::Error {
                    code: "bad_frame".into(),
                    message: format!("JSON parse: {e}"),
                },
            );
            return;
        }
    };

    match frame {
        WsFrame::Ping => {
            push_self(state, tenant, conn_id, WsFrame::Pong);
        }
        WsFrame::Pong => { /* heartbeat ack — nothing to do */ }
        WsFrame::YjsUpdate {
            parcel_id,
            yjs_delta,
        } => {
            if let Err(e) = base64::engine::general_purpose::STANDARD.decode(yjs_delta.as_bytes()) {
                push_self(
                    state,
                    tenant,
                    conn_id,
                    WsFrame::Error {
                        code: "bad_delta".into(),
                        message: format!("base64: {e}"),
                    },
                );
                return;
            }
            // P1.E: merge via terroir-core gRPC + persist.
            // For P1.D we re-broadcast to the rest of the tenant so other
            // devices see the in-flight delta — the merge / persist will be
            // wired when terroir-core exposes a write-side polygon RPC.
            let payload = serde_json::to_string(&WsFrame::YjsUpdate {
                parcel_id,
                yjs_delta,
            })
            .unwrap_or_default();
            state
                .ws_registry
                .broadcast(&tenant.slug, Some(conn_id), Message::Text(payload.into()));
        }
        WsFrame::Error { .. } => {
            // Clients SHOULD NOT send error frames upstream — just ignore.
        }
    }
}

fn push_self(state: &Arc<AppState>, tenant: &TenantContext, conn_id: ConnId, frame: WsFrame) {
    let payload = serde_json::to_string(&frame).unwrap_or_default();
    state.ws_registry.push_to(
        &tenant.slug,
        &tenant.user_id,
        conn_id,
        Message::Text(payload.into()),
    );
}
