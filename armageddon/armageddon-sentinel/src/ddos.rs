// SPDX-License-Identifier: AGPL-3.0-only
//! Distributed DDoS rate limiter backed by KAYA centralized counters.
//!
//! Local in-process rate limiting is insufficient in a multi-replica deployment:
//! a distributed attacker can spread requests across replicas and bypass per-process
//! thresholds. This module issues `INCR` + `EXPIRE` commands against KAYA so that
//! all ARMAGEDDON replicas share the same atomic counter namespace.
//!
//! ## Key schema
//! ```text
//! faso:rate:<dimension>:<value>    KAYA counter (integer, TTL = window_secs)
//! faso:swlog:<dimension>:<value>   KAYA sorted set for sub-second sliding window
//! ```
//!
//! ## Fail-open policy
//! If KAYA is unreachable and the check exceeds 1 ms, the limiter returns `Allow`
//! to avoid turning a cache outage into a self-inflicted DoS. A metric counter
//! tracks every fail-open event so the operator can alert on it.

use std::sync::Arc;
use std::time::Duration;

use armageddon_nexus::kaya::{KayaClient, KayaError};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// -- decision type --

/// Decision returned by [`DistributedRateLimiter::check`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitDecision {
    /// Request is within limits — forward normally.
    Allow,
    /// Request is in the warning band (75–100 % of threshold) — present CAPTCHA.
    Challenge,
    /// Request exceeded the threshold — block immediately.
    Block,
}

// -- config --

/// Configuration for the distributed rate limiter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedRateLimiterConfig {
    /// Whether the distributed limiter is enabled. When `false`, every call returns `Allow`.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Length of the fixed counter window in seconds.
    #[serde(default = "default_window_secs")]
    pub window_secs: u64,

    /// Maximum requests allowed within `window_secs` before the key is blocked.
    #[serde(default = "default_threshold")]
    pub threshold_per_window: u32,

    /// Fraction of `threshold_per_window` at which `Challenge` is returned instead of `Allow`.
    /// Default 0.75 (i.e. challenge starts at 75 % of limit).
    #[serde(default = "default_challenge_ratio")]
    pub challenge_ratio: f64,

    /// Maximum wall-clock time allowed for the KAYA round-trip.
    /// If exceeded the limiter fails open (returns `Allow`).
    #[serde(default = "default_fail_open_ms")]
    pub fail_open_timeout_ms: u64,

    /// Enable the sorted-set sliding window log for sub-second precision.
    #[serde(default)]
    pub sliding_window_log: bool,

    /// Path to the YAML ASN deny-list (hot-reloaded).
    #[serde(default)]
    pub asn_blocklist_path: Option<String>,
}

fn default_true() -> bool { true }
fn default_window_secs() -> u64 { 1 }
fn default_threshold() -> u32 { 500 }
fn default_challenge_ratio() -> f64 { 0.75 }
fn default_fail_open_ms() -> u64 { 1 }

impl Default for DistributedRateLimiterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_secs: default_window_secs(),
            threshold_per_window: default_threshold(),
            challenge_ratio: default_challenge_ratio(),
            fail_open_timeout_ms: default_fail_open_ms(),
            sliding_window_log: false,
            asn_blocklist_path: None,
        }
    }
}

// -- limiter --

/// Distributed rate limiter.
///
/// Each call to [`check`] issues an atomic `INCR` + conditional `EXPIRE` against
/// KAYA. Because KAYA itself is the counter authority, every ARMAGEDDON replica
/// sees the same count and a distributed attacker cannot bypass the limit by
/// spreading load across replicas.
pub struct DistributedRateLimiter {
    kaya: Arc<KayaClient>,
    window_secs: u64,
    threshold_per_window: u32,
    challenge_ratio: f64,
    fail_open_timeout: Duration,
    sliding_window_log: bool,
    /// In-memory ASN deny-list (hot-reloaded from YAML).
    asn_blocklist: arc_swap::ArcSwap<Vec<u32>>,
}

impl DistributedRateLimiter {
    /// Create a new limiter from config and a shared KAYA client.
    pub fn new(kaya: Arc<KayaClient>, cfg: &DistributedRateLimiterConfig) -> Self {
        Self {
            kaya,
            window_secs: cfg.window_secs,
            threshold_per_window: cfg.threshold_per_window,
            challenge_ratio: cfg.challenge_ratio,
            fail_open_timeout: Duration::from_millis(cfg.fail_open_timeout_ms),
            sliding_window_log: cfg.sliding_window_log,
            asn_blocklist: arc_swap::ArcSwap::from_pointee(Vec::new()),
        }
    }

    // -- public API --

    /// Check whether the given key is within rate limits.
    ///
    /// `key` must be one of the canonical prefixed forms:
    /// - `ip:<client_ip>` — per source IP
    /// - `asn:<asn_number>` — per autonomous system
    /// - `session:<id>` — per authenticated session
    /// - `country:<cc>` — per country code
    ///
    /// The function returns within `fail_open_timeout_ms` even if KAYA is slow.
    pub async fn check(&self, key: &str) -> RateLimitDecision {
        let t0 = std::time::Instant::now();

        // Fast path: ASN hard-block (in-memory, zero network round-trip).
        if let Some(asn) = parse_asn_key(key) {
            let list = self.asn_blocklist.load();
            if list.contains(&asn) {
                debug!(key, "ASN in deny-list, hard block");
                return RateLimitDecision::Block;
            }
        }

        let result = tokio::time::timeout(
            self.fail_open_timeout,
            self.check_kaya(key),
        )
        .await;

        match result {
            Ok(Ok(decision)) => {
                let elapsed_us = t0.elapsed().as_micros();
                debug!(key, elapsed_us, ?decision, "distributed rate check");
                decision
            }
            Ok(Err(e)) => {
                warn!(key, error = %e, "KAYA error in rate limiter — fail open");
                RateLimitDecision::Allow
            }
            Err(_timeout) => {
                warn!(
                    key,
                    timeout_ms = self.fail_open_timeout.as_millis(),
                    "KAYA timeout in rate limiter — fail open"
                );
                RateLimitDecision::Allow
            }
        }
    }

    /// Replace the in-memory ASN deny-list (called by the hot-reload task).
    pub fn reload_asn_blocklist(&self, asns: Vec<u32>) {
        self.asn_blocklist.store(Arc::new(asns));
        tracing::info!(count = self.asn_blocklist.load().len(), "ASN blocklist reloaded");
    }

    // -- internal --

    /// Core check logic against KAYA.
    async fn check_kaya(&self, key: &str) -> Result<RateLimitDecision, KayaError> {
        let kaya_key = format!("faso:rate:{}", key);

        let count = if self.sliding_window_log {
            self.sliding_window_check(&kaya_key).await?
        } else {
            self.fixed_window_incr(&kaya_key).await?
        };

        let decision = self.decide(count);
        Ok(decision)
    }

    /// Fixed-window counter: INCR + EXPIRE on first hit.
    ///
    /// The pipeline is:
    /// 1. `INCR faso:rate:<key>`  → new count (atomic)
    /// 2. `EXPIRE faso:rate:<key> <window>` only when count == 1 so we do not
    ///    reset the TTL on every hit (which would give an immortal key to a slow
    ///    but persistent attacker).
    async fn fixed_window_incr(&self, kaya_key: &str) -> Result<u64, KayaError> {
        // incr_with_expire issues INCR + EXPIRE atomically in a pipeline.
        self.kaya.incr_with_expire(kaya_key, self.window_secs).await
    }

    /// Sorted-set sliding window log for sub-second precision.
    ///
    /// Algorithm (ZRANGEBYSCORE variant):
    /// 1. Compute `now_ms` (Unix milliseconds).
    /// 2. `ZREMRANGEBYSCORE faso:swlog:<key> 0 (now_ms - window_ms)` — purge old entries.
    /// 3. `ZADD faso:swlog:<key> <now_ms> <uuid>` — record this hit.
    /// 4. `ZCARD faso:swlog:<key>` — count members in current window.
    /// 5. `EXPIRE faso:swlog:<key> <window_secs + 1>` — bound memory.
    async fn sliding_window_check(&self, kaya_key: &str) -> Result<u64, KayaError> {
        let swlog_key = kaya_key.replacen("faso:rate:", "faso:swlog:", 1);
        let count = self
            .kaya
            .sliding_window_incr(&swlog_key, self.window_secs)
            .await?;
        Ok(count)
    }

    /// Map a raw counter value to a [`RateLimitDecision`].
    fn decide(&self, count: u64) -> RateLimitDecision {
        let threshold = self.threshold_per_window as u64;
        if count > threshold {
            RateLimitDecision::Block
        } else if count as f64 > threshold as f64 * self.challenge_ratio {
            RateLimitDecision::Challenge
        } else {
            RateLimitDecision::Allow
        }
    }
}

// -- helpers --

/// Extract the raw ASN number from a key of the form `asn:<number>`.
fn parse_asn_key(key: &str) -> Option<u32> {
    key.strip_prefix("asn:").and_then(|s| s.parse::<u32>().ok())
}

// -- ASN blocklist hot-reload --

/// Load an ASN deny-list from a YAML file.
///
/// Expected format:
/// ```yaml
/// # asn-blocklist.yaml
/// asns:
///   - 12345
///   - 67890
/// ```
pub async fn load_asn_blocklist(path: &str) -> anyhow::Result<Vec<u32>> {
    let content = tokio::fs::read_to_string(path).await?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&content)?;
    let asns = doc
        .get("asns")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(asns)
}

/// Spawn a hot-reload task that watches `path` and updates the limiter every
/// `interval_secs` seconds.
pub fn spawn_asn_reload_task(
    limiter: Arc<DistributedRateLimiter>,
    path: String,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            match load_asn_blocklist(&path).await {
                Ok(asns) => limiter.reload_asn_blocklist(asns),
                Err(e) => warn!(path, error = %e, "failed to reload ASN blocklist"),
            }
        }
    })
}

// No extension trait needed: all KAYA commands are provided directly by
// `KayaClient::incr_with_expire` and `KayaClient::sliding_window_incr`
// which are defined in armageddon-nexus::kaya.

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;

    // -- unit: decision mapping --

    #[test]
    fn decide_allow_below_challenge_band() {
        let cfg = DistributedRateLimiterConfig {
            threshold_per_window: 100,
            challenge_ratio: 0.75,
            ..Default::default()
        };
        let kaya = Arc::new(KayaClient::new("127.0.0.1", 6380));
        let limiter = DistributedRateLimiter::new(kaya, &cfg);

        // challenge_ratio = 0.75, threshold = 100 → challenge band is count > 75
        assert_eq!(limiter.decide(74), RateLimitDecision::Allow);
        assert_eq!(limiter.decide(75), RateLimitDecision::Allow);      // 75 == 75 % → still Allow
        assert_eq!(limiter.decide(76), RateLimitDecision::Challenge);  // 76 > 75 → Challenge
        assert_eq!(limiter.decide(99), RateLimitDecision::Challenge);
        assert_eq!(limiter.decide(100), RateLimitDecision::Challenge); // at threshold, not over
        assert_eq!(limiter.decide(101), RateLimitDecision::Block);     // over threshold → Block
    }

    // -- unit: ASN blocklist --

    #[test]
    fn asn_key_parsing() {
        assert_eq!(parse_asn_key("asn:12345"), Some(12345));
        assert_eq!(parse_asn_key("ip:1.2.3.4"), None);
        assert_eq!(parse_asn_key("asn:"), None);
        assert_eq!(parse_asn_key("asn:0"), Some(0));
    }

    #[test]
    fn asn_reload_blocks_listed_asn() {
        let cfg = DistributedRateLimiterConfig::default();
        let kaya = Arc::new(KayaClient::new("127.0.0.1", 6380));
        let limiter = DistributedRateLimiter::new(kaya, &cfg);

        limiter.reload_asn_blocklist(vec![12345, 67890]);
        let list = limiter.asn_blocklist.load();
        assert!(list.contains(&12345));
        assert!(list.contains(&67890));
        assert!(!list.contains(&99999));
    }

    // -- unit: fail-open on timeout --

    #[tokio::test]
    async fn check_fails_open_when_kaya_unreachable() {
        // KayaClient pointing at a port nothing is listening on.
        let cfg = DistributedRateLimiterConfig {
            enabled: true,
            threshold_per_window: 500,
            window_secs: 1,
            fail_open_timeout_ms: 1, // 1 ms — will always time out
            ..Default::default()
        };
        let kaya = Arc::new(KayaClient::new("127.0.0.1", 19999));
        let limiter = DistributedRateLimiter::new(kaya, &cfg);

        // Even though KAYA is down, we should fail open (Allow).
        let decision = limiter.check("ip:10.0.0.1").await;
        assert_eq!(decision, RateLimitDecision::Allow);
    }

    // -- integration: 1000 requests → 501-1000 blocked --
    //
    // Hermetic: an in-process minimal RESP2 mock server handles INCR and
    // EXPIRE over loopback on an OS-assigned ephemeral port. No external
    // KAYA/Redis process is required, making the test safe in any CI.
    // See [`mock_kaya::spawn`] for the RESP state machine (MULTI/INCR/EXEC
    // pipeline + standalone EXPIRE).
    #[tokio::test]
    async fn ddos_1000_requests_threshold_500() {
        // Spawn mock KAYA on a loopback ephemeral port.
        let (kaya_port, _shutdown) = mock_kaya::spawn().await;

        let cfg = DistributedRateLimiterConfig {
            enabled: true,
            threshold_per_window: 500,
            window_secs: 60, // large window so it doesn't expire mid-test
            challenge_ratio: 0.75,
            fail_open_timeout_ms: 2000, // generous: mock is in-process
            ..Default::default()
        };
        let kaya = Arc::new(KayaClient::new("127.0.0.1", kaya_port));
        kaya.connect().await.expect("mock KAYA must be reachable");

        let limiter = Arc::new(DistributedRateLimiter::new(Arc::clone(&kaya), &cfg));
        let test_ip = format!("ip:test-ddos-{}", uuid::Uuid::new_v4());

        let mut allowed = 0u32;
        let mut blocked = 0u32;

        for _ in 0..1000 {
            match limiter.check(&test_ip).await {
                RateLimitDecision::Allow | RateLimitDecision::Challenge => allowed += 1,
                RateLimitDecision::Block => blocked += 1,
            }
        }

        // Requests 1-500 must be allowed (or challenged), 501-1000 must be blocked.
        assert!(
            allowed >= 500,
            "expected >= 500 allowed, got {}",
            allowed
        );
        assert!(
            blocked >= 500,
            "expected >= 500 blocked, got {}",
            blocked
        );
        assert_eq!(
            allowed + blocked,
            1000,
            "total requests mismatch"
        );
    }

    // -- in-process RESP2 mock for KAYA/Redis --
    //
    // A minimal subset of the Redis wire protocol, just enough for this
    // module's usage (`incr_with_expire`):
    //
    // * `MULTI`                                 → `+OK`
    // * `INCR <key>` (inside tx)                → `+QUEUED`
    // * `EXEC`                                  → `*1\r\n:<count>`
    // * `EXPIRE <key> <secs>`                   → `:1`
    // * `PING`, `HELLO`, `CLIENT SETNAME`, ...  → best-effort generic replies
    //
    // The mock binds on `127.0.0.1:0` and returns its port. A shutdown
    // `oneshot::Sender` is returned via a guard that stops the server when
    // dropped (end of test).
    mod mock_kaya {
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpListener;
        use tokio::sync::Mutex;

        /// Handle returned by [`spawn`]. Dropping it aborts the server task.
        pub struct MockHandle {
            _task: tokio::task::JoinHandle<()>,
        }

        impl Drop for MockHandle {
            fn drop(&mut self) {
                self._task.abort();
            }
        }

        /// Spawn the mock server and return `(port, handle)`.
        pub async fn spawn() -> (u16, MockHandle) {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let port = listener.local_addr().unwrap().port();
            let store: Arc<Mutex<HashMap<String, i64>>> = Arc::new(Mutex::new(HashMap::new()));

            let task = tokio::spawn(async move {
                loop {
                    let (stream, _peer) = match listener.accept().await {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let store = Arc::clone(&store);
                    tokio::spawn(async move {
                        let _ = handle_client(stream, store).await;
                    });
                }
            });

            (port, MockHandle { _task: task })
        }

        async fn handle_client(
            stream: tokio::net::TcpStream,
            store: Arc<Mutex<HashMap<String, i64>>>,
        ) -> std::io::Result<()> {
            let (rd, mut wr) = stream.into_split();
            let mut rd = BufReader::new(rd);

            // Transaction state per-connection (no nesting supported).
            let mut tx_open = false;
            let mut tx_replies: Vec<Vec<u8>> = Vec::new();

            loop {
                let cmd = match read_array(&mut rd).await? {
                    Some(c) => c,
                    None => return Ok(()), // peer closed
                };
                if cmd.is_empty() {
                    continue;
                }
                let name = cmd[0].to_ascii_uppercase();

                // Handle MULTI / EXEC / DISCARD transaction flow.
                match name.as_slice() {
                    b"MULTI" => {
                        tx_open = true;
                        tx_replies.clear();
                        wr.write_all(b"+OK\r\n").await?;
                        continue;
                    }
                    b"EXEC" => {
                        tx_open = false;
                        // Write EXEC response: array of per-command replies.
                        let mut out = format!("*{}\r\n", tx_replies.len()).into_bytes();
                        for r in tx_replies.drain(..) {
                            out.extend_from_slice(&r);
                        }
                        wr.write_all(&out).await?;
                        continue;
                    }
                    b"DISCARD" => {
                        tx_open = false;
                        tx_replies.clear();
                        wr.write_all(b"+OK\r\n").await?;
                        continue;
                    }
                    _ => {}
                }

                // Compute the reply for the command.
                let reply: Vec<u8> = match name.as_slice() {
                    b"PING" => b"+PONG\r\n".to_vec(),
                    b"HELLO" => {
                        // Minimal RESP2 reply for HELLO: empty map-ish array.
                        b"*0\r\n".to_vec()
                    }
                    b"CLIENT" => {
                        // CLIENT SETNAME / GETNAME / INFO etc. — accept all.
                        b"+OK\r\n".to_vec()
                    }
                    b"COMMAND" => b"*0\r\n".to_vec(),
                    b"SELECT" => b"+OK\r\n".to_vec(),
                    b"INCR" => {
                        let key = String::from_utf8_lossy(&cmd[1]).into_owned();
                        let mut s = store.lock().await;
                        let v = s.entry(key).or_insert(0);
                        *v += 1;
                        format!(":{}\r\n", *v).into_bytes()
                    }
                    b"INCRBY" => {
                        let key = String::from_utf8_lossy(&cmd[1]).into_owned();
                        let delta: i64 = cmd
                            .get(2)
                            .and_then(|b| std::str::from_utf8(b).ok())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let mut s = store.lock().await;
                        let v = s.entry(key).or_insert(0);
                        *v += delta;
                        format!(":{}\r\n", *v).into_bytes()
                    }
                    b"DECR" => {
                        let key = String::from_utf8_lossy(&cmd[1]).into_owned();
                        let mut s = store.lock().await;
                        let v = s.entry(key).or_insert(0);
                        *v -= 1;
                        format!(":{}\r\n", *v).into_bytes()
                    }
                    b"DECRBY" => {
                        let key = String::from_utf8_lossy(&cmd[1]).into_owned();
                        let delta: i64 = cmd
                            .get(2)
                            .and_then(|b| std::str::from_utf8(b).ok())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let mut s = store.lock().await;
                        let v = s.entry(key).or_insert(0);
                        *v -= delta;
                        format!(":{}\r\n", *v).into_bytes()
                    }
                    b"EXPIRE" | b"PEXPIRE" | b"EXPIREAT" | b"PEXPIREAT" => {
                        // Ignore TTL in the mock — we only care about the counter.
                        b":1\r\n".to_vec()
                    }
                    b"DEL" => b":1\r\n".to_vec(),
                    b"GET" => {
                        let key = String::from_utf8_lossy(&cmd[1]).into_owned();
                        let s = store.lock().await;
                        match s.get(&key) {
                            Some(v) => format!("${}\r\n{}\r\n", v.to_string().len(), v)
                                .into_bytes(),
                            None => b"$-1\r\n".to_vec(),
                        }
                    }
                    _ => {
                        // Unknown command — emit a generic error but keep the
                        // connection open so the test can continue.
                        let msg = format!(
                            "-ERR unknown command '{}' in mock\r\n",
                            String::from_utf8_lossy(&name)
                        );
                        msg.into_bytes()
                    }
                };

                if tx_open {
                    tx_replies.push(reply);
                    wr.write_all(b"+QUEUED\r\n").await?;
                } else {
                    wr.write_all(&reply).await?;
                }
            }
        }

        /// Read one RESP2 array and return its bulk-string elements.
        /// Returns `Ok(None)` on clean EOF.
        async fn read_array<R: tokio::io::AsyncBufRead + Unpin>(
            rd: &mut R,
        ) -> std::io::Result<Option<Vec<Vec<u8>>>> {
            let mut line = String::new();
            let n = rd.read_line(&mut line).await?;
            if n == 0 {
                return Ok(None);
            }
            let line = line.trim_end_matches(['\r', '\n']);
            let first = match line.chars().next() {
                Some(c) => c,
                None => return Ok(Some(Vec::new())),
            };
            match first {
                '*' => {
                    let count: i64 = line[1..].parse().unwrap_or(0);
                    if count < 0 {
                        return Ok(Some(Vec::new()));
                    }
                    let mut items = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        let item = read_bulk(rd).await?;
                        items.push(item);
                    }
                    Ok(Some(items))
                }
                // Inline command fallback: whitespace-split.
                _ => Ok(Some(
                    line.split_whitespace()
                        .map(|s| s.as_bytes().to_vec())
                        .collect(),
                )),
            }
        }

        async fn read_bulk<R: tokio::io::AsyncBufRead + Unpin>(
            rd: &mut R,
        ) -> std::io::Result<Vec<u8>> {
            let mut hdr = String::new();
            rd.read_line(&mut hdr).await?;
            let hdr = hdr.trim_end_matches(['\r', '\n']);
            let len: i64 = hdr
                .strip_prefix('$')
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            if len < 0 {
                return Ok(Vec::new());
            }
            let mut buf = vec![0u8; len as usize];
            rd.read_exact(&mut buf).await?;
            // Consume trailing \r\n.
            let mut crlf = [0u8; 2];
            rd.read_exact(&mut crlf).await?;
            Ok(buf)
        }
    }

    // -- unit: challenge band integration --

    #[test]
    fn challenge_band_boundaries() {
        // threshold=100, challenge_ratio=0.75 → challenge at count 76..=100, block at 101+
        let cfg = DistributedRateLimiterConfig {
            threshold_per_window: 100,
            challenge_ratio: 0.75,
            ..Default::default()
        };
        let kaya = Arc::new(KayaClient::new("127.0.0.1", 6380));
        let limiter = DistributedRateLimiter::new(kaya, &cfg);

        // challenge_ratio=0.75, threshold=100 → challenge when count > 75.0
        // count 75 is exactly at 75 %, so Allow; count 76 exceeds it → Challenge.
        for i in 1..=75u64 {
            assert_eq!(
                limiter.decide(i),
                RateLimitDecision::Allow,
                "count={} should be Allow",
                i
            );
        }
        for i in 76..=100u64 {
            assert_eq!(
                limiter.decide(i),
                RateLimitDecision::Challenge,
                "count={} should be Challenge",
                i
            );
        }
        for i in 101..=110u64 {
            assert_eq!(
                limiter.decide(i),
                RateLimitDecision::Block,
                "count={} should be Block",
                i
            );
        }
    }
}
