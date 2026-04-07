//! Raft request/response types for inter-node communication.

use serde::{Deserialize, Serialize};

/// A Raft log entry (replicated command).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftRequest {
    /// Set a key-value pair.
    Set {
        key: Vec<u8>,
        value: Vec<u8>,
        ttl_secs: Option<u64>,
    },
    /// Delete keys.
    Del { keys: Vec<Vec<u8>> },
    /// Forward a raw RESP3 command.
    RawCommand { name: String, args: Vec<Vec<u8>> },
}

/// Response from a Raft-replicated operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftResponse {
    Ok,
    Value(Option<Vec<u8>>),
    Integer(i64),
    Error(String),
}
