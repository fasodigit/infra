//! KAYA Cluster: Raft consensus per shard, consistent hashing, rebalancing.

pub mod hash_ring;
pub mod node;
pub mod raft_types;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use hash_ring::HashRing;
pub use node::{NodeId, NodeInfo, NodeState};
pub use raft_types::{RaftRequest, RaftResponse};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ClusterError {
    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("not leader for shard {0}")]
    NotLeader(u64),

    #[error("raft error: {0}")]
    Raft(String),

    #[error("rebalance in progress")]
    RebalanceInProgress,

    #[error("cluster not initialized")]
    NotInitialized,

    #[error("cluster error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub enabled: bool,
    pub node_id: String,
    pub seeds: Vec<String>,
    pub raft_heartbeat_ms: u64,
    pub raft_election_timeout_ms: u64,
    pub virtual_nodes: u32,
    pub replication_factor: u32,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: String::new(),
            seeds: Vec::new(),
            raft_heartbeat_ms: 150,
            raft_election_timeout_ms: 1000,
            virtual_nodes: 256,
            replication_factor: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Cluster Manager
// ---------------------------------------------------------------------------

/// Manages cluster topology and shard assignment.
pub struct ClusterManager {
    config: ClusterConfig,
    /// This node's info.
    local_node: NodeInfo,
    /// Known nodes in the cluster.
    nodes: parking_lot::RwLock<HashMap<NodeId, NodeInfo>>,
    /// Consistent hash ring for shard -> node routing.
    ring: parking_lot::RwLock<HashRing>,
}

impl ClusterManager {
    pub fn new(config: ClusterConfig) -> Self {
        let node_id = if config.node_id.is_empty() {
            uuid::Uuid::now_v7().to_string()
        } else {
            config.node_id.clone()
        };

        let local_node = NodeInfo {
            id: NodeId(node_id.clone()),
            addr: String::new(),
            state: NodeState::Joining,
        };

        let ring = HashRing::new(config.virtual_nodes);

        Self {
            config,
            local_node,
            nodes: parking_lot::RwLock::new(HashMap::new()),
            ring: parking_lot::RwLock::new(ring),
        }
    }

    /// Get the node responsible for a given key.
    pub fn route_key(&self, key: &[u8]) -> Option<NodeId> {
        let ring = self.ring.read();
        ring.get_node(key)
    }

    /// Is this key owned by the local node?
    pub fn is_local(&self, key: &[u8]) -> bool {
        self.route_key(key)
            .map(|n| n == self.local_node.id)
            .unwrap_or(true) // if no cluster, everything is local
    }

    /// Add a node to the cluster.
    pub fn add_node(&self, node: NodeInfo) {
        let id = node.id.clone();
        self.ring.write().add_node(&id);
        self.nodes.write().insert(id, node);
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&self, id: &NodeId) {
        self.ring.write().remove_node(id);
        self.nodes.write().remove(id);
    }

    /// List all known nodes.
    pub fn list_nodes(&self) -> Vec<NodeInfo> {
        self.nodes.read().values().cloned().collect()
    }

    /// Local node info.
    pub fn local_node(&self) -> &NodeInfo {
        &self.local_node
    }

    pub fn config(&self) -> &ClusterConfig {
        &self.config
    }
}
