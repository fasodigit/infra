// SPDX-License-Identifier: AGPL-3.0-or-later
//! In-process registry of connected WebSocket clients.
//!
//! Keyed by `tenant_slug` first (top-level mutex), then `user_id` (a single
//! user may have multiple devices: phone + tablet). Each connection holds
//! an unbounded `tokio::sync::mpsc::UnboundedSender<Message>` so that the
//! broadcast path is non-blocking from the producer side. The receive side
//! lives inside the per-connection task in `handler.rs`.
//!
//! Single-process registry only. For multi-replica deployments behind
//! ARMAGEDDON, broadcast across pods will use KAYA pub/sub
//! (`terroir:mobile:ws:broadcast:{tenant}`) — wired in P1.E.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::Message;
use parking_lot::{Mutex, RwLock};
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

/// Unique handle for a single WebSocket connection (one per upgrade).
pub type ConnId = u64;

/// Sender side of a per-connection mpsc channel — outbound frames to client.
pub type ConnSender = UnboundedSender<Message>;

#[derive(Default)]
struct TenantBucket {
    /// `user_id` → list of (conn_id, sender). A user may have multiple devices.
    by_user: HashMap<String, Vec<(ConnId, ConnSender)>>,
}

/// Registry of active WebSocket clients, indexed by tenant slug.
///
/// Cloning is cheap (Arc).
#[derive(Default)]
pub struct WsRegistry {
    inner: RwLock<HashMap<String, TenantBucket>>,
    next_id: Mutex<u64>,
}

impl WsRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Allocate a fresh connection id.
    pub fn next_conn_id(&self) -> ConnId {
        let mut g = self.next_id.lock();
        *g += 1;
        *g
    }

    /// Insert a new connection for `(tenant_slug, user_id)`.
    pub fn insert(&self, tenant_slug: &str, user_id: &str, conn_id: ConnId, tx: ConnSender) {
        let mut guard = self.inner.write();
        let bucket = guard.entry(tenant_slug.to_owned()).or_default();
        bucket
            .by_user
            .entry(user_id.to_owned())
            .or_default()
            .push((conn_id, tx));
        debug!(
            tenant = tenant_slug,
            user_id, conn_id, "registered WebSocket"
        );
    }

    /// Remove a connection by id.
    pub fn remove(&self, tenant_slug: &str, user_id: &str, conn_id: ConnId) {
        let mut guard = self.inner.write();
        if let Some(bucket) = guard.get_mut(tenant_slug)
            && let Some(conns) = bucket.by_user.get_mut(user_id)
        {
            conns.retain(|(id, _)| *id != conn_id);
            if conns.is_empty() {
                bucket.by_user.remove(user_id);
            }
        }
        debug!(
            tenant = tenant_slug,
            user_id, conn_id, "unregistered WebSocket"
        );
    }

    /// Broadcast a message to every client of the same tenant **except** the
    /// originator (`exclude_conn_id`). Errors are silently dropped — a closed
    /// receiver simply means the client disconnected mid-broadcast.
    pub fn broadcast(&self, tenant_slug: &str, exclude_conn_id: Option<ConnId>, msg: Message) {
        let guard = self.inner.read();
        let Some(bucket) = guard.get(tenant_slug) else {
            return;
        };
        for conns in bucket.by_user.values() {
            for (id, tx) in conns {
                if Some(*id) == exclude_conn_id {
                    continue;
                }
                let _ = tx.send(msg.clone());
            }
        }
    }

    /// Push a message to a specific connection.
    /// Returns `true` if delivered (channel still open), `false` otherwise.
    pub fn push_to(&self, tenant_slug: &str, user_id: &str, conn_id: ConnId, msg: Message) -> bool {
        let guard = self.inner.read();
        let Some(bucket) = guard.get(tenant_slug) else {
            return false;
        };
        let Some(conns) = bucket.by_user.get(user_id) else {
            return false;
        };
        for (id, tx) in conns {
            if *id == conn_id {
                return tx.send(msg).is_ok();
            }
        }
        false
    }

    /// For tests / metrics — count of total connections in a tenant.
    pub fn tenant_size(&self, tenant_slug: &str) -> usize {
        self.inner
            .read()
            .get(tenant_slug)
            .map(|b| b.by_user.values().map(Vec::len).sum())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn insert_and_remove() {
        let r = WsRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        r.insert("t_pilot", "user-1", 1, tx);
        assert_eq!(r.tenant_size("t_pilot"), 1);
        r.remove("t_pilot", "user-1", 1);
        assert_eq!(r.tenant_size("t_pilot"), 0);
    }
}
