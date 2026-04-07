//! Value types stored in KAYA.

use bytes::Bytes;
use std::collections::BTreeSet;

/// The different value types KAYA can store for a key.
#[derive(Debug, Clone)]
pub enum KayaValue {
    /// Simple string/binary value (result of SET).
    String(Bytes),
    /// Set of unique members (result of SADD).
    Set(BTreeSet<Bytes>),
    /// Sorted set with scores.
    SortedSet(Vec<(f64, Bytes)>),
    /// List (LPUSH/RPUSH).
    List(Vec<Bytes>),
    /// Hash map (HSET/HGET).
    Hash(Vec<(Bytes, Bytes)>),
}

impl KayaValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Set(_) => "set",
            Self::SortedSet(_) => "zset",
            Self::List(_) => "list",
            Self::Hash(_) => "hash",
        }
    }
}
