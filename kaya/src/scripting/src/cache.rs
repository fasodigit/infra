//! Script cache: LRU cache mapping SHA -> compiled AST.

use std::collections::HashMap;

use parking_lot::RwLock;
use rhai::AST;

/// LRU-ish cache for compiled Rhai scripts, keyed by SHA-256 hash.
pub struct ScriptCache {
    entries: RwLock<HashMap<String, AST>>,
    max_size: usize,
}

impl ScriptCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_size,
        }
    }

    /// Compute a hex-encoded hash of the script source (using ahash for speed).
    pub fn sha(script: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        script.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Insert a compiled script. Returns the SHA.
    pub fn insert(&self, source: &str, ast: AST) -> String {
        let sha = Self::sha(source);
        let mut entries = self.entries.write();

        // Simple eviction: if over capacity, drop a random entry.
        if entries.len() >= self.max_size {
            if let Some(key) = entries.keys().next().cloned() {
                entries.remove(&key);
            }
        }

        entries.insert(sha.clone(), ast);
        sha
    }

    /// Get a compiled script by SHA.
    pub fn get(&self, sha: &str) -> Option<AST> {
        self.entries.read().get(sha).cloned()
    }

    /// Check if a script is cached.
    pub fn contains(&self, sha: &str) -> bool {
        self.entries.read().contains_key(sha)
    }

    /// Number of cached scripts.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Clear the cache.
    pub fn clear(&self) {
        self.entries.write().clear();
    }
}
