//! Eviction policies: LRU, LFU, TTL-based.

use crate::entry::Entry;
use crate::EvictionPolicyKind;

/// Trait for eviction strategies.
pub trait EvictionPolicy: Send + Sync {
    /// Given a set of candidate entries, return the key that should be evicted.
    /// Returns `None` if no eviction is needed.
    fn select_victim<'a>(
        &self,
        candidates: &[(&'a [u8], &Entry)],
    ) -> Option<Vec<u8>>;
}

/// Manages eviction for a shard.
pub struct EvictionManager {
    policy: Box<dyn EvictionPolicy>,
}

impl EvictionManager {
    pub fn new(kind: EvictionPolicyKind) -> Self {
        let policy: Box<dyn EvictionPolicy> = match kind {
            EvictionPolicyKind::Lru => Box::new(LruPolicy),
            EvictionPolicyKind::Lfu => Box::new(LfuPolicy),
            EvictionPolicyKind::Ttl => Box::new(TtlPolicy),
            EvictionPolicyKind::None => Box::new(NoPolicy),
        };
        Self { policy }
    }

    pub fn select_victim<'a>(
        &self,
        candidates: &[(&'a [u8], &Entry)],
    ) -> Option<Vec<u8>> {
        self.policy.select_victim(candidates)
    }
}

/// Evict the least recently accessed entry.
struct LruPolicy;

impl EvictionPolicy for LruPolicy {
    fn select_victim<'a>(
        &self,
        candidates: &[(&'a [u8], &Entry)],
    ) -> Option<Vec<u8>> {
        candidates
            .iter()
            .min_by_key(|(_, e)| e.metadata.last_accessed)
            .map(|(k, _)| k.to_vec())
    }
}

/// Evict the least frequently accessed entry.
struct LfuPolicy;

impl EvictionPolicy for LfuPolicy {
    fn select_victim<'a>(
        &self,
        candidates: &[(&'a [u8], &Entry)],
    ) -> Option<Vec<u8>> {
        candidates
            .iter()
            .min_by_key(|(_, e)| e.metadata.access_count)
            .map(|(k, _)| k.to_vec())
    }
}

/// Evict the entry closest to expiration.
struct TtlPolicy;

impl EvictionPolicy for TtlPolicy {
    fn select_victim<'a>(
        &self,
        candidates: &[(&'a [u8], &Entry)],
    ) -> Option<Vec<u8>> {
        candidates
            .iter()
            .filter(|(_, e)| e.metadata.expires_at.is_some())
            .min_by_key(|(_, e)| e.metadata.expires_at.unwrap())
            .map(|(k, _)| k.to_vec())
    }
}

/// No eviction: never evict.
struct NoPolicy;

impl EvictionPolicy for NoPolicy {
    fn select_victim<'a>(
        &self,
        _candidates: &[(&'a [u8], &Entry)],
    ) -> Option<Vec<u8>> {
        None
    }
}
