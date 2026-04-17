//! RESP3 client-side caching — server-side tracking table.
//!
//! Implements the tracking state machine described by the RESP3 client-side
//! caching specification (RESP3 spec, public). The server maintains, for every
//! connected client, either:
//!
//! * a *default* tracking set of keys the client has recently read, or
//! * a *broadcast* (BCAST) list of key prefixes the client subscribes to.
//!
//! When a write-type command mutates a key, [`TrackingTable::invalidate`]
//! resolves all impacted clients and pushes a `> invalidate <keys>` RESP3
//! frame on their outbound channel. Default-mode entries for the impacted keys
//! are removed from the table (the RESP3 spec permits that, since the client
//! has now been notified and must re-read the value before the server will
//! re-track it).
//!
//! Sovereignty: KAYA implements the public RESP3 tracking protocol directly;
//! no external reference server is required at runtime.

use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;
use parking_lot::Mutex;
use prometheus::{register_int_counter, register_int_gauge_vec, IntCounter, IntGaugeVec};
use smallvec::SmallVec;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::instrument;

use kaya_protocol::{Encoder, Frame};

/// Stable per-connection identifier used by the tracking table.
pub type ClientId = u64;

/// Upper bound on the number of tracked keys (default mode) a single client
/// may accumulate before the server force-flushes its tracking state.
pub const DEFAULT_MAX_TRACKED_KEYS_PER_CLIENT: usize = 100_000;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors emitted by the tracking subsystem.
#[derive(Debug, Error)]
pub enum TrackingError {
    /// The client is over its tracked-keys budget and has been force-flushed.
    #[error("tracking flushed for client {0}: memory limit reached")]
    MemoryLimit(ClientId),
}

// ---------------------------------------------------------------------------
// Tracking mode
// ---------------------------------------------------------------------------

/// Operational mode for a connection with respect to client-side caching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackingMode {
    /// Tracking disabled (default state on connect).
    Off,

    /// Default mode: the server remembers the exact keys the client reads and
    /// only notifies the client when those keys change.
    Default,

    /// BCAST mode: the server notifies the client for any key whose name
    /// starts with one of the configured prefixes, regardless of whether
    /// the client has read it.
    Bcast(SmallVec<[Bytes; 4]>),
}

impl TrackingMode {
    /// Short label used by Prometheus and `CLIENT TRACKINGINFO`.
    pub fn label(&self) -> &'static str {
        match self {
            TrackingMode::Off => "off",
            TrackingMode::Default => "default",
            TrackingMode::Bcast(_) => "bcast",
        }
    }

    /// Whether tracking is active in any form.
    pub fn is_on(&self) -> bool {
        !matches!(self, TrackingMode::Off)
    }
}

// ---------------------------------------------------------------------------
// Per-client options
// ---------------------------------------------------------------------------

/// Options that influence how invalidations are generated for a specific
/// client (OPTIN/OPTOUT/NOLOOP, NO-EVICT hint).
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    /// `CLIENT TRACKING ... OPTIN` — only track reads preceded by
    /// `CLIENT CACHING YES`.
    pub opt_in: bool,
    /// `CLIENT TRACKING ... OPTOUT` — track everything except reads
    /// preceded by `CLIENT CACHING NO`.
    pub opt_out: bool,
    /// `CLIENT TRACKING ... NOLOOP` — suppress invalidations produced by
    /// the client's own writes.
    pub no_loop: bool,
    /// `CLIENT NO-EVICT ON` — hint honoured by the store eviction layer.
    pub no_evict: bool,
    /// One-shot override toggled by `CLIENT CACHING YES|NO`. Consumed by
    /// the next read and reset after it.
    pub caching_override: Option<bool>,
    /// Optional redirect target — push frames are sent to this client id
    /// instead of the reading client (RESP3 OUT-OF-BAND redirection).
    pub redirect: Option<ClientId>,
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/// Prometheus metrics scoped to the tracking subsystem.
pub struct TrackingMetrics {
    pub clients_gauge: IntGaugeVec,
    pub keys_gauge: prometheus::IntGauge,
    pub prefixes_gauge: prometheus::IntGauge,
    pub invalidations_total: IntCounter,
    pub memory_evictions_total: IntCounter,
}

impl TrackingMetrics {
    /// Register or fetch the tracking metrics from the default registry.
    ///
    /// Re-registration is tolerated (useful for tests and for multiple
    /// [`TrackingTable`] instances in a single process).
    pub fn new() -> Self {
        fn reg_counter(name: &str, help: &str) -> IntCounter {
            match register_int_counter!(name, help) {
                Ok(c) => c,
                Err(prometheus::Error::AlreadyReg) => {
                    let mfs = prometheus::default_registry().gather();
                    for mf in mfs {
                        if mf.get_name() == name {
                            if let Some(_m) = mf.get_metric().first() {
                                // Re-build via a detached counter; never fails.
                                return prometheus::IntCounter::new(name, help)
                                    .expect("counter rebuild");
                            }
                        }
                    }
                    prometheus::IntCounter::new(name, help).expect("counter rebuild")
                }
                Err(e) => panic!("prometheus register error: {e}"),
            }
        }

        fn reg_gauge(name: &str, help: &str) -> prometheus::IntGauge {
            match prometheus::register_int_gauge!(name, help) {
                Ok(g) => g,
                Err(prometheus::Error::AlreadyReg) => {
                    prometheus::IntGauge::new(name, help).expect("gauge rebuild")
                }
                Err(e) => panic!("prometheus register error: {e}"),
            }
        }

        fn reg_gauge_vec(name: &str, help: &str, labels: &[&str]) -> IntGaugeVec {
            match register_int_gauge_vec!(name, help, labels) {
                Ok(g) => g,
                Err(prometheus::Error::AlreadyReg) => {
                    let opts = prometheus::Opts::new(name, help);
                    IntGaugeVec::new(opts, labels).expect("gauge vec rebuild")
                }
                Err(e) => panic!("prometheus register error: {e}"),
            }
        }

        Self {
            clients_gauge: reg_gauge_vec(
                "kaya_tracking_clients_gauge",
                "Number of connected clients with tracking enabled, by mode",
                &["mode"],
            ),
            keys_gauge: reg_gauge(
                "kaya_tracking_keys_gauge",
                "Total number of (client, key) tracking entries in default mode",
            ),
            prefixes_gauge: reg_gauge(
                "kaya_tracking_prefixes_gauge",
                "Total number of (client, prefix) subscriptions in BCAST mode",
            ),
            invalidations_total: reg_counter(
                "kaya_tracking_invalidations_sent_total",
                "Cumulative number of RESP3 invalidate PUSH frames sent to clients",
            ),
            memory_evictions_total: reg_counter(
                "kaya_tracking_memory_evictions_total",
                "Clients force-flushed from tracking because they exceeded the key budget",
            ),
        }
    }
}

impl Default for TrackingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Invalidation event
// ---------------------------------------------------------------------------

/// Event posted by write-type commands on the store side to request
/// invalidation notifications to be propagated to tracking clients.
///
/// `origin` is the id of the client performing the write — used to honour
/// `NOLOOP`.
#[derive(Debug, Clone)]
pub struct InvalidationEvent {
    pub keys: Vec<Bytes>,
    pub origin: Option<ClientId>,
}

// ---------------------------------------------------------------------------
// TrackingTable
// ---------------------------------------------------------------------------

/// Per-client tracking state stored inside the table. Wrapped in a dedicated
/// struct so we can take a single lock per client while keeping the top-level
/// `DashMap` optimized for concurrent client lookups.
#[derive(Debug)]
struct ClientState {
    mode: TrackingMode,
    options: ClientOptions,
    tracked_keys: usize,
}

impl ClientState {
    fn new(mode: TrackingMode, options: ClientOptions) -> Self {
        Self {
            mode,
            options,
            tracked_keys: 0,
        }
    }
}

/// The central state structure for RESP3 client-side caching.
///
/// `TrackingTable` is designed to be shared as `Arc<TrackingTable>` between
/// network handlers (which mutate it) and writer paths (which call
/// [`TrackingTable::invalidate`]).
pub struct TrackingTable {
    /// Default-mode reverse index: key -> clients that currently track it.
    keys_to_clients: DashMap<Bytes, SmallVec<[ClientId; 4]>>,
    /// BCAST reverse index: prefix -> subscribers.
    prefix_clients: DashMap<Bytes, SmallVec<[ClientId; 4]>>,
    /// Per-client state (mode, options, counter).
    clients: DashMap<ClientId, Arc<Mutex<ClientState>>>,
    /// Hard cap: max number of tracked keys for a single client in Default mode.
    max_tracked_keys_per_client: usize,
    /// Prometheus metrics.
    metrics: TrackingMetrics,
}

impl std::fmt::Debug for TrackingTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackingTable")
            .field("clients", &self.clients.len())
            .field("max_tracked_keys_per_client", &self.max_tracked_keys_per_client)
            .finish_non_exhaustive()
    }
}

impl TrackingTable {
    /// Build a new table with the default memory limit.
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_MAX_TRACKED_KEYS_PER_CLIENT)
    }

    /// Build a new table with a custom per-client tracked-keys budget.
    pub fn with_limit(max_tracked_keys_per_client: usize) -> Self {
        Self {
            keys_to_clients: DashMap::new(),
            prefix_clients: DashMap::new(),
            clients: DashMap::new(),
            max_tracked_keys_per_client,
            metrics: TrackingMetrics::new(),
        }
    }

    /// Enable default-mode tracking for `client`.
    #[instrument(level = "debug", skip(self))]
    pub fn enable_default(&self, client: ClientId) {
        self.enable_default_with_options(client, ClientOptions::default());
    }

    /// Variant of [`enable_default`] that also stores per-client options.
    #[instrument(level = "debug", skip(self, options))]
    pub fn enable_default_with_options(&self, client: ClientId, options: ClientOptions) {
        self.disable(client);
        self.clients.insert(
            client,
            Arc::new(Mutex::new(ClientState::new(TrackingMode::Default, options))),
        );
        self.refresh_gauges();
    }

    /// Enable BCAST tracking for `client` with the given prefixes.
    ///
    /// An empty prefix list subscribes the client to *all* keyspace changes.
    #[instrument(level = "debug", skip(self, prefixes))]
    pub fn enable_bcast(&self, client: ClientId, prefixes: Vec<Bytes>) {
        self.enable_bcast_with_options(client, prefixes, ClientOptions::default());
    }

    /// Variant of [`enable_bcast`] that also stores per-client options.
    #[instrument(level = "debug", skip(self, prefixes, options))]
    pub fn enable_bcast_with_options(
        &self,
        client: ClientId,
        prefixes: Vec<Bytes>,
        options: ClientOptions,
    ) {
        self.disable(client);

        let subs: SmallVec<[Bytes; 4]> = if prefixes.is_empty() {
            // Empty prefix = match everything.
            SmallVec::from_vec(vec![Bytes::new()])
        } else {
            SmallVec::from_iter(prefixes.into_iter())
        };

        for p in subs.iter() {
            self.prefix_clients
                .entry(p.clone())
                .or_default()
                .push(client);
        }

        self.clients.insert(
            client,
            Arc::new(Mutex::new(ClientState::new(
                TrackingMode::Bcast(subs),
                options,
            ))),
        );
        self.refresh_gauges();
    }

    /// Disable tracking for `client` and release all per-client state.
    #[instrument(level = "debug", skip(self))]
    pub fn disable(&self, client: ClientId) {
        if let Some((_, state)) = self.clients.remove(&client) {
            let mode = state.lock().mode.clone();
            match mode {
                TrackingMode::Default => {
                    self.drop_all_keys_for(client);
                }
                TrackingMode::Bcast(prefixes) => {
                    for p in prefixes.iter() {
                        self.drop_prefix_for(p, client);
                    }
                }
                TrackingMode::Off => {}
            }
        }
        self.refresh_gauges();
    }

    /// Record that `client` has read `key` (Default mode only).
    ///
    /// If the per-client budget is exceeded, the client is force-flushed,
    /// a `TRACKINGINFO flush-limit-reached` warning is conceptually
    /// delivered by the caller via [`TrackingTable::take_flushed_clients`]
    /// or through the returned [`TrackingError`].
    #[instrument(level = "trace", skip(self, key))]
    pub fn track_read(&self, client: ClientId, key: &[u8]) -> Result<(), TrackingError> {
        let state_arc = match self.clients.get(&client) {
            Some(s) => s.clone(),
            None => return Ok(()),
        };

        let mut state = state_arc.lock();
        if !matches!(state.mode, TrackingMode::Default) {
            return Ok(());
        }

        // Honour OPTIN/OPTOUT.
        if state.options.opt_in {
            match state.options.caching_override.take() {
                Some(true) => {}
                _ => return Ok(()),
            }
        } else if state.options.opt_out {
            match state.options.caching_override.take() {
                Some(false) => return Ok(()),
                _ => {}
            }
        }

        if state.tracked_keys >= self.max_tracked_keys_per_client {
            state.mode = TrackingMode::Off;
            state.tracked_keys = 0;
            drop(state);
            self.drop_all_keys_for(client);
            self.clients.remove(&client);
            self.metrics.memory_evictions_total.inc();
            self.refresh_gauges();
            return Err(TrackingError::MemoryLimit(client));
        }

        let key_bytes = Bytes::copy_from_slice(key);
        let mut clients = self.keys_to_clients.entry(key_bytes).or_default();
        if !clients.contains(&client) {
            clients.push(client);
            state.tracked_keys += 1;
            self.metrics.keys_gauge.inc();
        }

        Ok(())
    }

    /// Toggle `CLIENT CACHING YES|NO` one-shot override for the next read.
    pub fn set_caching_override(&self, client: ClientId, value: bool) {
        if let Some(state_arc) = self.clients.get(&client) {
            let mut state = state_arc.lock();
            state.options.caching_override = Some(value);
        }
    }

    /// Toggle `CLIENT NO-EVICT ON|OFF` for a given client.
    pub fn set_no_evict(&self, client: ClientId, value: bool) {
        if let Some(state_arc) = self.clients.get(&client) {
            let mut state = state_arc.lock();
            state.options.no_evict = value;
        }
    }

    /// Retrieve a snapshot of the tracking info for the given client, for
    /// `CLIENT TRACKINGINFO`.
    pub fn info(&self, client: ClientId) -> TrackingInfo {
        match self.clients.get(&client) {
            Some(state) => {
                let st = state.lock();
                TrackingInfo {
                    mode: st.mode.clone(),
                    tracked_keys: st.tracked_keys,
                    options: st.options.clone(),
                }
            }
            None => TrackingInfo {
                mode: TrackingMode::Off,
                tracked_keys: 0,
                options: ClientOptions::default(),
            },
        }
    }

    /// Current mode for `client`, or [`TrackingMode::Off`] if unknown.
    pub fn mode(&self, client: ClientId) -> TrackingMode {
        self.clients
            .get(&client)
            .map(|s| s.lock().mode.clone())
            .unwrap_or(TrackingMode::Off)
    }

    /// Broadcast invalidations for a batch of keys.
    ///
    /// For each key, all clients currently tracking it (default mode) *plus*
    /// all clients whose BCAST prefixes match the key, receive a
    /// `> invalidate [keys]` push frame.
    ///
    /// `origin` is honored for `NOLOOP`: clients whose `no_loop` option is set
    /// do not receive invalidations caused by their own writes.
    #[instrument(level = "debug", skip(self, keys, senders))]
    pub async fn invalidate(
        &self,
        keys: &[&[u8]],
        origin: Option<ClientId>,
        senders: &DashMap<ClientId, mpsc::Sender<Frame>>,
    ) {
        if keys.is_empty() {
            return;
        }

        // Build per-recipient lists of keys to invalidate.
        let mut batches: ahash_map::AHashMap<ClientId, Vec<Bytes>> =
            ahash_map::AHashMap::default();

        for &key in keys {
            // Default-mode recipients.
            let key_bytes = Bytes::copy_from_slice(key);
            if let Some((_, subs)) = self.keys_to_clients.remove(&key_bytes) {
                for cid in subs.iter() {
                    if Self::should_skip(*cid, origin, &self.clients) {
                        continue;
                    }
                    batches
                        .entry(*cid)
                        .or_default()
                        .push(key_bytes.clone());
                }
                // Decrement the counter for each impacted client in Default mode.
                for cid in subs.iter() {
                    if let Some(state_arc) = self.clients.get(cid) {
                        let mut st = state_arc.lock();
                        if st.tracked_keys > 0 {
                            st.tracked_keys -= 1;
                        }
                    }
                }
                self.metrics.keys_gauge.sub(subs.len() as i64);
            }

            // BCAST recipients.
            for item in self.prefix_clients.iter() {
                let prefix = item.key();
                if key.starts_with(prefix) {
                    for cid in item.value().iter() {
                        if Self::should_skip(*cid, origin, &self.clients) {
                            continue;
                        }
                        batches
                            .entry(*cid)
                            .or_default()
                            .push(key_bytes.clone());
                    }
                }
            }
        }

        // Send one PUSH frame per recipient with the full batch.
        for (cid, keys) in batches.into_iter() {
            if let Some(sender) = senders.get(&cid) {
                let key_refs: Vec<&[u8]> = keys.iter().map(|b: &Bytes| b.as_ref()).collect();
                let frame = invalidate_frame(&key_refs);
                if sender.send(frame).await.is_ok() {
                    self.metrics.invalidations_total.inc();
                }
            }
        }

        self.refresh_gauges();
    }

    /// Cheap check for NOLOOP.
    fn should_skip(
        client: ClientId,
        origin: Option<ClientId>,
        clients: &DashMap<ClientId, Arc<Mutex<ClientState>>>,
    ) -> bool {
        let Some(origin) = origin else {
            return false;
        };
        if client != origin {
            return false;
        }
        clients
            .get(&client)
            .map(|s| s.lock().options.no_loop)
            .unwrap_or(false)
    }

    fn drop_all_keys_for(&self, client: ClientId) {
        let mut removed = 0i64;
        self.keys_to_clients.retain(|_, subs| {
            let before = subs.len();
            subs.retain(|c| *c != client);
            removed += (before - subs.len()) as i64;
            !subs.is_empty()
        });
        if removed > 0 {
            self.metrics.keys_gauge.sub(removed);
        }
    }

    fn drop_prefix_for(&self, prefix: &Bytes, client: ClientId) {
        if let Some(mut subs) = self.prefix_clients.get_mut(prefix) {
            subs.retain(|c| *c != client);
        }
        self.prefix_clients.retain(|_, subs| !subs.is_empty());
    }

    /// Refresh the clients/prefixes gauges from the authoritative maps.
    fn refresh_gauges(&self) {
        let mut default = 0i64;
        let mut bcast = 0i64;
        for item in self.clients.iter() {
            match item.value().lock().mode {
                TrackingMode::Default => default += 1,
                TrackingMode::Bcast(_) => bcast += 1,
                TrackingMode::Off => {}
            }
        }
        self.metrics
            .clients_gauge
            .with_label_values(&["default"])
            .set(default);
        self.metrics
            .clients_gauge
            .with_label_values(&["bcast"])
            .set(bcast);

        let prefix_count: i64 = self
            .prefix_clients
            .iter()
            .map(|i| i.value().len() as i64)
            .sum();
        self.metrics.prefixes_gauge.set(prefix_count);
    }

    /// Test/introspection helper: returns the number of clients currently
    /// enrolled in any form of tracking.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Test/introspection helper: number of (key, client) default entries.
    pub fn tracked_keys_count(&self) -> usize {
        self.keys_to_clients
            .iter()
            .map(|i| i.value().len())
            .sum()
    }
}

impl Default for TrackingTable {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TrackingInfo (return type of CLIENT TRACKINGINFO)
// ---------------------------------------------------------------------------

/// Snapshot of a client's tracking configuration, returned by
/// `CLIENT TRACKINGINFO`.
#[derive(Debug, Clone)]
pub struct TrackingInfo {
    pub mode: TrackingMode,
    pub tracked_keys: usize,
    pub options: ClientOptions,
}

// ---------------------------------------------------------------------------
// Frame helpers
// ---------------------------------------------------------------------------

/// Build a RESP3 `> invalidate <keys>` PUSH frame.
///
/// Wire shape:
///
/// ```text
/// >2\r\n
/// $10\r\ninvalidate\r\n
/// *N\r\n
/// $L\r\n<key1>\r\n
/// ...
/// ```
pub fn invalidate_frame(keys: &[&[u8]]) -> Frame {
    let key_frames: Vec<Frame> = keys
        .iter()
        .map(|k| Frame::BulkString(Bytes::copy_from_slice(k)))
        .collect();
    Frame::Push(vec![
        Frame::BulkString(Bytes::from_static(b"invalidate")),
        Frame::Array(key_frames),
    ])
}

/// Encode an invalidation PUSH frame directly to bytes. Convenience for the
/// protocol-layer helper promised in the public API surface.
pub fn encode_invalidate(keys: &[&[u8]]) -> bytes::Bytes {
    let frame = invalidate_frame(keys);
    let mut buf = bytes::BytesMut::new();
    Encoder::encode(&frame, &mut buf);
    buf.freeze()
}

// ---------------------------------------------------------------------------
// Tiny wrapper around ahash for a HashMap — avoids pulling std HashMap
// with RandomState while we're already using ahash in the workspace.
// ---------------------------------------------------------------------------

mod ahash_map {
    use std::collections::HashMap;
    pub type AHashMap<K, V> = HashMap<K, V, ahash::RandomState>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dashmap::DashMap;
    use tokio::sync::mpsc;

    fn new_senders() -> (DashMap<ClientId, mpsc::Sender<Frame>>, Vec<mpsc::Receiver<Frame>>) {
        (DashMap::new(), Vec::new())
    }

    fn add_client(
        senders: &DashMap<ClientId, mpsc::Sender<Frame>>,
        id: ClientId,
    ) -> mpsc::Receiver<Frame> {
        let (tx, rx) = mpsc::channel(16);
        senders.insert(id, tx);
        rx
    }

    #[tokio::test]
    async fn track_read_then_invalidate_notifies_client() {
        let table = TrackingTable::new();
        let senders = DashMap::new();
        let mut rx = add_client(&senders, 1);

        table.enable_default(1);
        table.track_read(1, b"my:app:user:42").unwrap();

        table
            .invalidate(&[b"my:app:user:42"], None, &senders)
            .await;

        let frame = rx.recv().await.expect("expected push frame");
        match frame {
            Frame::Push(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], Frame::BulkString(b) if b.as_ref() == b"invalidate"));
                match &parts[1] {
                    Frame::Array(keys) => {
                        assert_eq!(keys.len(), 1);
                        match &keys[0] {
                            Frame::BulkString(b) => assert_eq!(b.as_ref(), b"my:app:user:42"),
                            other => panic!("unexpected key frame: {other:?}"),
                        }
                    }
                    other => panic!("expected array, got {other:?}"),
                }
            }
            other => panic!("expected push, got {other:?}"),
        }

        // After invalidation the key is dropped from the default table.
        assert_eq!(table.tracked_keys_count(), 0);
    }

    #[tokio::test]
    async fn bcast_prefix_matches_all_keys_with_prefix() {
        let table = TrackingTable::new();
        let senders = DashMap::new();
        let mut rx = add_client(&senders, 7);

        table.enable_bcast(7, vec![Bytes::from_static(b"my:app:")]);

        // No track_read — that's the whole point of BCAST.
        table
            .invalidate(
                &[b"my:app:user:42", b"my:app:cart:9", b"other:ignored"],
                None,
                &senders,
            )
            .await;

        let frame = rx.recv().await.expect("expected push frame");
        match frame {
            Frame::Push(parts) => match &parts[1] {
                Frame::Array(keys) => {
                    let decoded: Vec<&[u8]> = keys
                        .iter()
                        .filter_map(|f| match f {
                            Frame::BulkString(b) => Some(b.as_ref()),
                            _ => None,
                        })
                        .collect();
                    assert!(decoded.contains(&b"my:app:user:42".as_ref()));
                    assert!(decoded.contains(&b"my:app:cart:9".as_ref()));
                    assert!(!decoded.contains(&b"other:ignored".as_ref()));
                }
                other => panic!("expected array, got {other:?}"),
            },
            other => panic!("expected push, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn opt_out_client_not_notified() {
        let table = TrackingTable::new();
        let senders = DashMap::new();
        let mut rx = add_client(&senders, 3);

        let options = ClientOptions {
            opt_out: true,
            ..Default::default()
        };
        table.enable_default_with_options(3, options);

        // CLIENT CACHING NO — next read is NOT tracked.
        table.set_caching_override(3, false);
        table.track_read(3, b"k1").unwrap();

        table.invalidate(&[b"k1"], None, &senders).await;

        // Channel should be empty.
        let result = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err(), "no invalidation expected for opted-out read");
    }

    #[tokio::test]
    async fn memory_limit_flushes_client() {
        let table = TrackingTable::with_limit(3);
        let senders = DashMap::new();
        let _rx = add_client(&senders, 9);

        table.enable_default(9);
        table.track_read(9, b"a").unwrap();
        table.track_read(9, b"b").unwrap();
        table.track_read(9, b"c").unwrap();

        // 4th read must overflow and flush.
        let result = table.track_read(9, b"d");
        assert!(matches!(result, Err(TrackingError::MemoryLimit(9))));
        assert_eq!(table.mode(9), TrackingMode::Off);
        assert_eq!(table.tracked_keys_count(), 0);
    }

    #[tokio::test]
    async fn encode_invalidate_produces_resp3_push() {
        let bytes = encode_invalidate(&[b"k1", b"k2"]);
        let as_str = std::str::from_utf8(&bytes).unwrap();
        assert!(as_str.starts_with(">2\r\n"));
        assert!(as_str.contains("$10\r\ninvalidate\r\n"));
        assert!(as_str.contains("*2\r\n"));
        assert!(as_str.contains("$2\r\nk1\r\n"));
        assert!(as_str.contains("$2\r\nk2\r\n"));
    }

    #[tokio::test]
    async fn noloop_suppresses_self_invalidation() {
        let table = TrackingTable::new();
        let senders = DashMap::new();
        let mut rx = add_client(&senders, 5);

        let options = ClientOptions {
            no_loop: true,
            ..Default::default()
        };
        table.enable_default_with_options(5, options);
        table.track_read(5, b"k").unwrap();

        // Client 5 writes to its own tracked key.
        table.invalidate(&[b"k"], Some(5), &senders).await;

        let result = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await;
        assert!(result.is_err(), "NOLOOP should suppress self-invalidation");
    }

    #[tokio::test]
    async fn disable_drops_all_state() {
        let table = TrackingTable::new();
        let senders = DashMap::new();
        let _rx = add_client(&senders, 1);

        table.enable_default(1);
        table.track_read(1, b"k1").unwrap();
        table.track_read(1, b"k2").unwrap();

        table.disable(1);

        assert_eq!(table.client_count(), 0);
        assert_eq!(table.tracked_keys_count(), 0);
    }
}
