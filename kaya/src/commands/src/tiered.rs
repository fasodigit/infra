//! KAYA tiered-storage command handlers.
//!
//! Implements Redis-compatible MEMORY / OBJECT commands plus KAYA-specific
//! tiered-storage control commands:
//!
//! | Command                          | Description                                     |
//! |----------------------------------|-------------------------------------------------|
//! | `MEMORY USAGE key [SAMPLES n]`   | Returns tier location + approximate size        |
//! | `OBJECT FREQ key`                | Returns LFU access counter                      |
//! | `OBJECT IDLETIME key`            | Returns seconds since last access               |
//! | `DEBUG SET-ACTIVE-EXPIRE 0\|1`   | Enable/disable active TTL expiry                |
//! | `KAYA.TIERED.STATS`              | Returns full `TieredStats` as a map             |
//! | `KAYA.TIERED.PROMOTE key`        | Force-promote key from cold → hot               |
//! | `KAYA.TIERED.DEMOTE key`         | Force-demote key from hot → cold                |
//! | `KAYA.TIERED.POLICY GET`         | Return the current migration policy as a string |
//! | `KAYA.TIERED.POLICY SET <json>`  | Replace the migration policy at runtime         |
//!
//! ## Handler isolation
//!
//! This module is intentionally **not** wired into `commands/lib.rs`,
//! `router.rs`, or `handler.rs`. It exposes [`TieredCommandHandler`] which
//! the server layer (or integration tests) instantiate directly, passing an
//! `Arc<TieredStore>`.

use std::sync::Arc;

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use kaya_tiered::{Location, MigrationPolicy, TieredStore};
use tracing::warn;

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Handles all tiered-storage commands.
///
/// Bind to a shared [`TieredStore`] instance; the handler is `Clone + Send + Sync`.
pub struct TieredCommandHandler {
    store: Arc<TieredStore>,
    /// When false, background active-expire is suppressed (DEBUG SET-ACTIVE-EXPIRE 0).
    active_expire: std::sync::atomic::AtomicBool,
}

impl TieredCommandHandler {
    /// Create a new handler backed by `store`.
    pub fn new(store: Arc<TieredStore>) -> Self {
        Self {
            store,
            active_expire: std::sync::atomic::AtomicBool::new(true),
        }
    }

    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    /// Dispatch a single command. Returns a `Frame` on success, or an error
    /// frame on unknown command / wrong arity.
    ///
    /// **Async** because PROMOTE / DEMOTE and STATS touch the cold backend.
    pub async fn dispatch(&self, cmd: &Command) -> Frame {
        match cmd.name.to_ascii_uppercase().as_str() {
            "MEMORY" => self.cmd_memory(cmd),
            "OBJECT" => self.cmd_object(cmd),
            "DEBUG" => self.cmd_debug(cmd),
            "KAYA.TIERED.STATS" => self.cmd_tiered_stats().await,
            "KAYA.TIERED.PROMOTE" => self.cmd_tiered_promote(cmd).await,
            "KAYA.TIERED.DEMOTE" => self.cmd_tiered_demote(cmd).await,
            "KAYA.TIERED.POLICY" => self.cmd_tiered_policy(cmd).await,
            _ => Frame::Error(format!(
                "ERR unknown tiered command '{}'",
                cmd.name
            )),
        }
    }

    // -----------------------------------------------------------------------
    // MEMORY USAGE key [SAMPLES n]
    // -----------------------------------------------------------------------

    /// `MEMORY USAGE key [SAMPLES n]`
    ///
    /// Returns an integer (approximate bytes) or a simple string with tier
    /// location info. When the key does not exist returns Null.
    fn cmd_memory(&self, cmd: &Command) -> Frame {
        let sub = match cmd.args.first() {
            Some(b) => String::from_utf8_lossy(b).to_ascii_uppercase(),
            None => return Frame::Error("ERR MEMORY subcommand required".into()),
        };

        if sub != "USAGE" {
            return Frame::Error(format!("ERR MEMORY subcommand '{}' not supported by tiered handler", sub));
        }

        let key = match cmd.args.get(1) {
            Some(k) => k,
            None => return Frame::Error("ERR MEMORY USAGE requires key".into()),
        };

        let location_str = match self.store.location(key) {
            Some(Location::Hot) | Some(Location::HotDirty) => "hot",
            Some(Location::Cold) => "cold",
            None => return Frame::Null,
        };

        let size_hint = self.store.hot_store().get(key)
            .ok()
            .flatten()
            .map(|v| v.len() as i64)
            .unwrap_or(0);

        // Return a 2-element array: [location, size_bytes]
        Frame::Array(vec![
            Frame::BulkString(Bytes::from(location_str)),
            Frame::Integer(size_hint),
        ])
    }

    // -----------------------------------------------------------------------
    // OBJECT FREQ key / OBJECT IDLETIME key
    // -----------------------------------------------------------------------

    /// `OBJECT FREQ key` — returns LFU access count.
    /// `OBJECT IDLETIME key` — returns idle seconds.
    fn cmd_object(&self, cmd: &Command) -> Frame {
        let sub = match cmd.args.first() {
            Some(b) => String::from_utf8_lossy(b).to_ascii_uppercase(),
            None => return Frame::Error("ERR OBJECT subcommand required".into()),
        };

        let key = match cmd.args.get(1) {
            Some(k) => k,
            None => return Frame::Error(format!("ERR OBJECT {} requires key", sub)),
        };

        match sub.as_str() {
            "FREQ" => match self.store.access_count(key) {
                Some(count) => Frame::Integer(count as i64),
                None => Frame::Null,
            },
            "IDLETIME" => match self.store.idle_secs(key) {
                Some(secs) => Frame::Integer(secs as i64),
                None => Frame::Null,
            },
            other => Frame::Error(format!("ERR OBJECT subcommand '{}' not supported", other)),
        }
    }

    // -----------------------------------------------------------------------
    // DEBUG SET-ACTIVE-EXPIRE 0|1
    // -----------------------------------------------------------------------

    fn cmd_debug(&self, cmd: &Command) -> Frame {
        let sub = match cmd.args.first() {
            Some(b) => String::from_utf8_lossy(b).to_ascii_uppercase(),
            None => return Frame::Error("ERR DEBUG subcommand required".into()),
        };

        if sub != "SET-ACTIVE-EXPIRE" {
            return Frame::Error(format!("ERR DEBUG '{}' not supported by tiered handler", sub));
        }

        let flag = match cmd.args.get(1) {
            Some(b) => match b.as_ref() {
                b"0" => false,
                b"1" => true,
                _ => return Frame::Error("ERR value must be 0 or 1".into()),
            },
            None => return Frame::Error("ERR DEBUG SET-ACTIVE-EXPIRE requires 0|1".into()),
        };

        self.active_expire.store(flag, std::sync::atomic::Ordering::Relaxed);
        Frame::SimpleString("OK".into())
    }

    // -----------------------------------------------------------------------
    // KAYA.TIERED.STATS
    // -----------------------------------------------------------------------

    async fn cmd_tiered_stats(&self) -> Frame {
        let stats = self.store.stats().await;
        Frame::Array(vec![
            Frame::BulkString(Bytes::from_static(b"hot_keys")),
            Frame::Integer(stats.hot_keys as i64),
            Frame::BulkString(Bytes::from_static(b"cold_keys")),
            Frame::Integer(stats.cold_keys as i64),
            Frame::BulkString(Bytes::from_static(b"hot_bytes")),
            Frame::Integer(stats.hot_bytes as i64),
            Frame::BulkString(Bytes::from_static(b"cold_bytes")),
            Frame::Integer(stats.cold_bytes as i64),
            Frame::BulkString(Bytes::from_static(b"migrations_total")),
            Frame::Integer(stats.migrations_total as i64),
            Frame::BulkString(Bytes::from_static(b"promotions_total")),
            Frame::Integer(stats.promotions_total as i64),
        ])
    }

    // -----------------------------------------------------------------------
    // KAYA.TIERED.PROMOTE key
    // -----------------------------------------------------------------------

    async fn cmd_tiered_promote(&self, cmd: &Command) -> Frame {
        let key = match cmd.args.first() {
            Some(k) => k.clone(),
            None => return Frame::Error("ERR KAYA.TIERED.PROMOTE requires key".into()),
        };

        match self.store.force_promote(&key).await {
            Ok(()) => Frame::SimpleString("OK".into()),
            Err(e) => {
                warn!("KAYA.TIERED.PROMOTE error: {}", e);
                Frame::Error(format!("ERR {}", e))
            }
        }
    }

    // -----------------------------------------------------------------------
    // KAYA.TIERED.DEMOTE key
    // -----------------------------------------------------------------------

    async fn cmd_tiered_demote(&self, cmd: &Command) -> Frame {
        let key = match cmd.args.first() {
            Some(k) => k.clone(),
            None => return Frame::Error("ERR KAYA.TIERED.DEMOTE requires key".into()),
        };

        match self.store.force_demote(&key).await {
            Ok(()) => Frame::SimpleString("OK".into()),
            Err(e) => {
                warn!("KAYA.TIERED.DEMOTE error: {}", e);
                Frame::Error(format!("ERR {}", e))
            }
        }
    }

    // -----------------------------------------------------------------------
    // KAYA.TIERED.POLICY GET|SET [json]
    // -----------------------------------------------------------------------

    async fn cmd_tiered_policy(&self, cmd: &Command) -> Frame {
        let sub = match cmd.args.first() {
            Some(b) => String::from_utf8_lossy(b).to_ascii_uppercase(),
            None => return Frame::Error("ERR KAYA.TIERED.POLICY requires GET or SET".into()),
        };

        match sub.as_str() {
            "GET" => {
                let policy = self.store.get_policy();
                let desc = policy_description(&policy);
                Frame::BulkString(Bytes::from(desc))
            }
            "SET" => {
                let json_bytes = match cmd.args.get(1) {
                    Some(b) => b,
                    None => return Frame::Error("ERR KAYA.TIERED.POLICY SET requires JSON policy".into()),
                };

                let json_str = match std::str::from_utf8(json_bytes) {
                    Ok(s) => s,
                    Err(_) => return Frame::Error("ERR policy must be valid UTF-8 JSON".into()),
                };

                match serde_json::from_str::<MigrationPolicy>(json_str) {
                    Ok(new_policy) => {
                        self.store.set_policy(new_policy);
                        Frame::SimpleString("OK".into())
                    }
                    Err(e) => Frame::Error(format!("ERR invalid policy JSON: {}", e)),
                }
            }
            other => Frame::Error(format!("ERR KAYA.TIERED.POLICY unknown subcommand '{}'", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn policy_description(policy: &MigrationPolicy) -> String {
    match policy {
        MigrationPolicy::LfuCold { min_idle_secs, max_hot_mem_bytes, migrate_ratio } => {
            format!(
                "lfu_cold min_idle={}s max_hot={}B ratio={}",
                min_idle_secs, max_hot_mem_bytes, migrate_ratio
            )
        }
        MigrationPolicy::TtlCold { hot_ttl_secs } => {
            format!("ttl_cold hot_ttl={}s", hot_ttl_secs)
        }
        MigrationPolicy::Manual => "manual".into(),
    }
}
