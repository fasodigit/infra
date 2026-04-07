//! Consistent hash ring for shard-to-node routing.

use std::collections::BTreeMap;

use crate::node::NodeId;

/// Consistent hash ring with virtual nodes.
pub struct HashRing {
    ring: BTreeMap<u64, NodeId>,
    virtual_nodes: u32,
}

impl HashRing {
    pub fn new(virtual_nodes: u32) -> Self {
        Self {
            ring: BTreeMap::new(),
            virtual_nodes,
        }
    }

    /// Add a node with `virtual_nodes` number of virtual entries.
    pub fn add_node(&mut self, node_id: &NodeId) {
        for i in 0..self.virtual_nodes {
            let key = format!("{}#{}", node_id.0, i);
            let hash = self.hash(key.as_bytes());
            self.ring.insert(hash, node_id.clone());
        }
    }

    /// Remove all virtual entries for a node.
    pub fn remove_node(&mut self, node_id: &NodeId) {
        self.ring.retain(|_, v| v != node_id);
    }

    /// Find the node responsible for a key.
    pub fn get_node(&self, key: &[u8]) -> Option<NodeId> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = self.hash(key);
        // Find the first node at or after the hash.
        self.ring
            .range(hash..)
            .next()
            .or_else(|| self.ring.iter().next())
            .map(|(_, node)| node.clone())
    }

    /// Number of real nodes (distinct node IDs).
    pub fn node_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for node in self.ring.values() {
            seen.insert(node.0.clone());
        }
        seen.len()
    }

    fn hash(&self, data: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        data.hash(&mut hasher);
        hasher.finish()
    }
}

impl std::fmt::Debug for HashRing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashRing")
            .field("entries", &self.ring.len())
            .field("nodes", &self.node_count())
            .finish()
    }
}
