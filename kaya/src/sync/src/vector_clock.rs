//! Vector clock for conflict resolution in distributed sync.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A vector clock mapping node IDs to logical timestamps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorClock {
    pub clocks: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment the clock for a given node.
    pub fn increment(&mut self, node_id: &str) {
        let counter = self.clocks.entry(node_id.to_string()).or_insert(0);
        *counter += 1;
    }

    /// Get the logical time for a node.
    pub fn get(&self, node_id: &str) -> u64 {
        self.clocks.get(node_id).copied().unwrap_or(0)
    }

    /// Merge with another vector clock (take max of each).
    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &ts) in &other.clocks {
            let entry = self.clocks.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(ts);
        }
    }

    /// Check if `self` happened before `other`.
    pub fn happened_before(&self, other: &VectorClock) -> bool {
        let mut at_least_one_less = false;

        for (node, &ts) in &self.clocks {
            let other_ts = other.get(node);
            if ts > other_ts {
                return false;
            }
            if ts < other_ts {
                at_least_one_less = true;
            }
        }

        // Check nodes in other but not in self.
        for (node, &ts) in &other.clocks {
            if !self.clocks.contains_key(node) && ts > 0 {
                at_least_one_less = true;
            }
        }

        at_least_one_less
    }

    /// Check if two clocks are concurrent (neither happened before the other).
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happened_before(other) && !other.happened_before(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_clock_ordering() {
        let mut a = VectorClock::new();
        a.increment("node1");
        a.increment("node1");

        let mut b = a.clone();
        b.increment("node1");

        assert!(a.happened_before(&b));
        assert!(!b.happened_before(&a));
    }

    #[test]
    fn vector_clock_concurrent() {
        let mut a = VectorClock::new();
        a.increment("node1");

        let mut b = VectorClock::new();
        b.increment("node2");

        assert!(a.is_concurrent(&b));
    }
}
