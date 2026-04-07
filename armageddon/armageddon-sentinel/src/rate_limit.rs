//! Sliding window rate limiter with periodic cleanup.
//!
//! Tracks request counts per key (IP, JWT sub, etc.) using a fixed-window
//! approximation of a sliding window. Expired entries are cleaned up periodically.

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Entry in the sliding window.
struct WindowEntry {
    /// Count in the current window.
    current_count: u64,
    /// Count in the previous window (for sliding approximation).
    previous_count: u64,
    /// When the current window started.
    window_start: Instant,
}

/// Sliding window rate limiter.
///
/// Uses a fixed-window with sliding window approximation:
/// effective_count = previous_count * (1 - elapsed_fraction) + current_count
pub struct SlidingWindowLimiter {
    window_secs: u64,
    max_requests: u64,
    entries: Arc<DashMap<String, WindowEntry>>,
}

impl SlidingWindowLimiter {
    pub fn new(window_secs: u64, max_requests: u64) -> Self {
        Self {
            window_secs,
            max_requests,
            entries: Arc::new(DashMap::new()),
        }
    }

    /// Check if a request should be allowed for the given key.
    /// Returns true if allowed, false if rate limited.
    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        let mut entry = self.entries.entry(key.to_string()).or_insert(WindowEntry {
            current_count: 0,
            previous_count: 0,
            window_start: now,
        });

        let elapsed = now.duration_since(entry.window_start);

        // Check if we need to rotate the window
        if elapsed >= window {
            // How many full windows have passed?
            if elapsed >= window * 2 {
                // More than 2 windows: reset everything
                entry.previous_count = 0;
                entry.current_count = 0;
            } else {
                // Exactly 1 window passed: rotate
                entry.previous_count = entry.current_count;
                entry.current_count = 0;
            }
            entry.window_start = now;
        }

        // Compute sliding window estimate
        let elapsed_fraction = elapsed.as_secs_f64() / self.window_secs as f64;
        let elapsed_fraction = elapsed_fraction.min(1.0);
        let estimated_count = (entry.previous_count as f64 * (1.0 - elapsed_fraction))
            + entry.current_count as f64;

        if estimated_count < self.max_requests as f64 {
            entry.current_count += 1;
            true
        } else {
            false
        }
    }

    /// Get the current estimated count for a key.
    pub fn current_count(&self, key: &str) -> u64 {
        self.entries.get(key).map_or(0, |e| e.current_count)
    }

    /// Clean up expired entries. Call periodically to prevent memory leaks.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let max_age = Duration::from_secs(self.window_secs * 2);

        self.entries.retain(|_, entry| {
            now.duration_since(entry.window_start) < max_age
        });
    }

    /// Start a background cleanup task that runs every `interval` seconds.
    pub fn start_cleanup_task(self: &Arc<Self>, interval_secs: u64) -> tokio::task::JoinHandle<()> {
        let limiter = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                ticker.tick().await;
                let before = limiter.entries.len();
                limiter.cleanup();
                let after = limiter.entries.len();
                if before != after {
                    tracing::debug!(
                        "rate limiter cleanup: {} -> {} entries",
                        before,
                        after
                    );
                }
            }
        })
    }

    /// Number of tracked keys.
    pub fn tracked_keys(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_rate_limiting() {
        let limiter = SlidingWindowLimiter::new(60, 5);

        // First 5 should be allowed
        for _ in 0..5 {
            assert!(limiter.allow("test-ip"));
        }
        // 6th should be denied
        assert!(!limiter.allow("test-ip"));
    }

    #[test]
    fn test_different_keys_independent() {
        let limiter = SlidingWindowLimiter::new(60, 2);

        assert!(limiter.allow("ip-1"));
        assert!(limiter.allow("ip-1"));
        assert!(!limiter.allow("ip-1"));

        // Different key should still be allowed
        assert!(limiter.allow("ip-2"));
        assert!(limiter.allow("ip-2"));
        assert!(!limiter.allow("ip-2"));
    }

    #[test]
    fn test_cleanup() {
        let limiter = SlidingWindowLimiter::new(1, 100); // 1 second window

        limiter.allow("key-1");
        limiter.allow("key-2");
        assert_eq!(limiter.tracked_keys(), 2);

        // Sleep past 2 windows
        std::thread::sleep(Duration::from_millis(2100));

        limiter.cleanup();
        assert_eq!(limiter.tracked_keys(), 0);
    }
}
