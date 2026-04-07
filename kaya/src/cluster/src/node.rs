//! Node identity and state.

use serde::{Deserialize, Serialize};

/// Unique identifier for a cluster node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// Node is joining the cluster.
    Joining,
    /// Node is active and serving requests.
    Active,
    /// Node is being drained for removal.
    Draining,
    /// Node is down / unreachable.
    Down,
}

/// Information about a cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub addr: String,
    pub state: NodeState,
}
