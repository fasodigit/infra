// Subscription tracking for connected ARMAGEDDON instances.
//
// Each ARMAGEDDON instance maintains subscriptions to specific resource
// types and resource names. The SubscriptionManager tracks these and
// determines which responses to push when the configuration changes.

use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info};

/// xDS resource type URLs.
pub mod type_urls {
    pub const CLUSTER: &str = "type.googleapis.com/envoy.config.cluster.v3.Cluster";
    pub const ENDPOINT: &str =
        "type.googleapis.com/envoy.config.endpoint.v3.ClusterLoadAssignment";
    pub const ROUTE: &str = "type.googleapis.com/envoy.config.route.v3.RouteConfiguration";
    pub const LISTENER: &str = "type.googleapis.com/envoy.config.listener.v3.Listener";
    pub const SECRET: &str =
        "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.Secret";
}

/// Unique identifier for a connected ARMAGEDDON instance.
pub type NodeId = String;

/// Tracks which resources each connected ARMAGEDDON instance is subscribed to.
#[derive(Debug, Clone)]
pub struct SubscriptionManager {
    /// node_id -> type_url -> subscribed resource names (empty = wildcard)
    subscriptions: Arc<DashMap<NodeId, HashMap<String, HashSet<String>>>>,

    /// node_id -> type_url -> last acknowledged version
    acked_versions: Arc<DashMap<NodeId, HashMap<String, String>>>,

    /// node_id -> type_url -> last sent nonce
    pending_nonces: Arc<DashMap<NodeId, HashMap<String, String>>>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(DashMap::new()),
            acked_versions: Arc::new(DashMap::new()),
            pending_nonces: Arc::new(DashMap::new()),
        }
    }

    /// Register a new ARMAGEDDON connection.
    pub fn register_node(&self, node_id: &str) {
        info!(node = %node_id, "registering ARMAGEDDON node");
        self.subscriptions
            .insert(node_id.to_string(), HashMap::new());
        self.acked_versions
            .insert(node_id.to_string(), HashMap::new());
        self.pending_nonces
            .insert(node_id.to_string(), HashMap::new());
    }

    /// Unregister a disconnected ARMAGEDDON instance.
    pub fn unregister_node(&self, node_id: &str) {
        info!(node = %node_id, "unregistering ARMAGEDDON node");
        self.subscriptions.remove(node_id);
        self.acked_versions.remove(node_id);
        self.pending_nonces.remove(node_id);
    }

    /// Update subscriptions from a DiscoveryRequest.
    pub fn update_subscription(
        &self,
        node_id: &str,
        type_url: &str,
        resource_names: Vec<String>,
    ) {
        debug!(
            node = %node_id,
            type_url = %type_url,
            resources = resource_names.len(),
            "updating subscription"
        );

        if let Some(mut subs) = self.subscriptions.get_mut(node_id) {
            let names: HashSet<String> = resource_names.into_iter().collect();
            subs.insert(type_url.to_string(), names);
        }
    }

    /// Record an ACK from ARMAGEDDON.
    pub fn record_ack(&self, node_id: &str, type_url: &str, version: &str, nonce: &str) {
        debug!(
            node = %node_id,
            type_url = %type_url,
            version = %version,
            nonce = %nonce,
            "ACK received"
        );

        if let Some(mut versions) = self.acked_versions.get_mut(node_id) {
            versions.insert(type_url.to_string(), version.to_string());
        }

        // Clear the pending nonce since it was ACKed
        if let Some(mut nonces) = self.pending_nonces.get_mut(node_id) {
            if nonces.get(type_url).map_or(false, |n| n == nonce) {
                nonces.remove(type_url);
            }
        }
    }

    /// Record a sent nonce so we can correlate ACK/NACK.
    pub fn record_nonce(&self, node_id: &str, type_url: &str, nonce: &str) {
        if let Some(mut nonces) = self.pending_nonces.get_mut(node_id) {
            nonces.insert(type_url.to_string(), nonce.to_string());
        }
    }

    /// Check if a node has a pending (un-ACKed) response for a type.
    pub fn has_pending(&self, node_id: &str, type_url: &str) -> bool {
        self.pending_nonces
            .get(node_id)
            .map_or(false, |nonces| nonces.contains_key(type_url))
    }

    /// Get all subscribed resource names for a node and type.
    /// Empty set means wildcard subscription (all resources).
    pub fn get_subscribed_resources(
        &self,
        node_id: &str,
        type_url: &str,
    ) -> Option<HashSet<String>> {
        self.subscriptions
            .get(node_id)
            .and_then(|subs| subs.get(type_url).cloned())
    }

    /// Get all node IDs that are subscribed to a given type.
    pub fn subscribers_for_type(&self, type_url: &str) -> Vec<NodeId> {
        self.subscriptions
            .iter()
            .filter_map(|entry| {
                if entry.value().contains_key(type_url) {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the number of connected nodes.
    pub fn connected_count(&self) -> usize {
        self.subscriptions.len()
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}
