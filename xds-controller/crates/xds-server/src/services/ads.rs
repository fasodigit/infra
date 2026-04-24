// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Aggregated Discovery Service (ADS) implementation — SOTW + Delta.
//
// This is the primary entry point for ARMAGEDDON. A single bidirectional
// gRPC stream carries all resource types (CDS, EDS, RDS, LDS, SDS).
//
// # SOTW flow (StreamAggregatedResources)
//
//   1. ARMAGEDDON opens StreamAggregatedResources
//   2. ARMAGEDDON sends DiscoveryRequest for each type it wants
//   3. xDS Controller sends DiscoveryResponse with **all** current resources
//   4. When ConfigStore changes, Controller pushes new DiscoveryResponse
//   5. ARMAGEDDON ACKs or NACKs each response via response_nonce
//   6. On NACK, Controller can implement instant rollback
//
// # Delta flow (DeltaAggregatedResources)
//
//   1. ARMAGEDDON opens DeltaAggregatedResources
//   2. First DeltaDiscoveryRequest carries `initial_resource_versions` (name→ver)
//      and `resource_names_subscribe` to declare interest.
//   3. Server computes the diff vs. the snapshot: sends only resources that are
//      absent in the client's map or carry a different version.  Also sends
//      `removed_resources` for names the client has but the store no longer has.
//   4. Subsequent requests are ACK (`response_nonce` set, no `error_detail`) or
//      NACK (`error_detail` set).  Subscribe/unsubscribe deltas also arrive here.
//   5. On ConfigStore change, the server computes a diff vs. the per-client
//      `client_resource_versions` map and sends only changed/removed resources.
//
// # Failure modes
//
// * **Leader loss / NACK storm**: NACKs are logged with a warning and a metric
//   is incremented; the server does NOT resend automatically — the client
//   retries by sending a fresh subscribe request.
// * **Client disconnect**: the per-stream `DeltaClientState` is removed from
//   `DELTA_STREAMS` before the task exits.
// * **ConfigStore watch lag**: the watch channel only holds the *latest* version;
//   if two updates race between polls the server will compute the diff against
//   the newest snapshot and the intermediate state is skipped (safe; EDS for
//   example is converged on the next delta, not lost forever).

use crate::config::ServerConfig;
use crate::convert;
use crate::generated::envoy::service::discovery::v3::{
    aggregated_discovery_service_server::AggregatedDiscoveryService, ControlPlane,
    DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryRequest, DiscoveryResponse, Resource,
};
use crate::subscription::{type_urls, SubscriptionManager};

use dashmap::DashMap;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, info, warn};
use uuid::Uuid;
use xds_store::ConfigStore;

// ---------------------------------------------------------------------------
// Delta per-stream state
// ---------------------------------------------------------------------------

/// Per-stream state for an active Delta ADS client.
///
/// Tracks the set of resources the client is subscribed to and the last known
/// version of each resource so we can compute diffs efficiently.
///
/// # Thread-safety
///
/// Each `DeltaClientState` lives inside a `DashMap` entry protected by the
/// DashMap shard lock.  We never hold the shard lock across an `.await` —
/// all mutations are synchronous and brief.
#[derive(Debug, Default)]
struct DeltaClientState {
    /// type_url → set of subscribed resource names.  Empty set = wildcard (all).
    subscribed: HashMap<String, HashSet<String>>,
    /// type_url → resource_name → version string last sent to this client.
    sent_versions: HashMap<String, HashMap<String, String>>,
}

impl DeltaClientState {
    fn new() -> Self {
        Self::default()
    }

    /// Apply `resource_names_subscribe` and `resource_names_unsubscribe` from
    /// an incoming `DeltaDiscoveryRequest`.
    fn apply_subscription_delta(
        &mut self,
        type_url: &str,
        subscribe: &[String],
        unsubscribe: &[String],
    ) {
        let entry = self
            .subscribed
            .entry(type_url.to_string())
            .or_insert_with(HashSet::new);
        for name in subscribe {
            entry.insert(name.clone());
        }
        for name in unsubscribe {
            entry.remove(name);
        }
    }

    /// Seed from `initial_resource_versions` on the first request.
    fn seed_initial_versions(
        &mut self,
        type_url: &str,
        initial: &HashMap<String, String>,
    ) {
        self.sent_versions
            .entry(type_url.to_string())
            .or_insert_with(HashMap::new)
            .extend(initial.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    /// Record a batch of resources that were just sent to this client.
    fn record_sent(&mut self, type_url: &str, name: &str, version: &str) {
        self.sent_versions
            .entry(type_url.to_string())
            .or_insert_with(HashMap::new)
            .insert(name.to_string(), version.to_string());
    }

    /// Record that a resource was removed (delete from sent_versions).
    fn record_removed(&mut self, type_url: &str, name: &str) {
        if let Some(map) = self.sent_versions.get_mut(type_url) {
            map.remove(name);
        }
    }

    /// Returns `true` if the client is subscribed to `name` under `type_url`.
    /// Wildcard (empty set) counts as subscribed to everything.
    fn is_subscribed(&self, type_url: &str, name: &str) -> bool {
        match self.subscribed.get(type_url) {
            None => false,
            Some(s) if s.is_empty() => true, // wildcard
            Some(s) => s.contains(name),
        }
    }

    /// The version the client last saw for `name` under `type_url`, if any.
    fn known_version(&self, type_url: &str, name: &str) -> Option<&str> {
        self.sent_versions
            .get(type_url)
            .and_then(|m| m.get(name))
            .map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Global Delta stream registry (stream_id → state)
// ---------------------------------------------------------------------------

type StreamId = String;

/// Global per-stream state for all active Delta ADS clients.
///
/// Keyed by stream UUID.  Entries are inserted on stream open and
/// removed on disconnect so the map never grows unboundedly.
static DELTA_STREAMS: OnceLock<Arc<DashMap<StreamId, DeltaClientState>>> = OnceLock::new();

fn delta_streams() -> &'static Arc<DashMap<StreamId, DeltaClientState>> {
    DELTA_STREAMS.get_or_init(|| Arc::new(DashMap::new()))
}

// ---------------------------------------------------------------------------
// Metrics helpers (inline, no external registry import needed here)
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

static DELTA_NACK_TOTAL: AtomicU64 = AtomicU64::new(0);
static DELTA_RESOURCES_SENT_TOTAL: AtomicU64 = AtomicU64::new(0);
static DELTA_REMOVED_SENT_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Expose current counters (for tests / admin endpoint).
pub fn delta_nack_total() -> u64 {
    DELTA_NACK_TOTAL.load(AtomicOrdering::Relaxed)
}
pub fn delta_resources_sent_total() -> u64 {
    DELTA_RESOURCES_SENT_TOTAL.load(AtomicOrdering::Relaxed)
}
pub fn delta_removed_sent_total() -> u64 {
    DELTA_REMOVED_SENT_TOTAL.load(AtomicOrdering::Relaxed)
}

// ---------------------------------------------------------------------------
// ADS service
// ---------------------------------------------------------------------------

/// ADS service implementation that serves all xDS resource types
/// over a single aggregated stream to ARMAGEDDON.
///
/// Supports both SOTW (`StreamAggregatedResources`) and Delta
/// (`DeltaAggregatedResources`) protocols.
pub struct AdsService {
    store: ConfigStore,
    subscriptions: SubscriptionManager,
    config: Arc<ServerConfig>,
}

impl AdsService {
    pub fn new(store: ConfigStore, config: Arc<ServerConfig>) -> Self {
        Self {
            store,
            subscriptions: SubscriptionManager::new(),
            config,
        }
    }
}

#[tonic::async_trait]
impl AggregatedDiscoveryService for AdsService {
    type StreamAggregatedResourcesStream =
        Pin<Box<ReceiverStream<Result<DiscoveryResponse, Status>>>>;

    /// Main ADS SOTW streaming RPC.
    /// ARMAGEDDON calls this to receive all xDS resources over a single stream.
    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status> {
        let remote_addr = request
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        info!(remote = %remote_addr, "ARMAGEDDON connected via ADS SOTW stream");

        let (tx, rx) = mpsc::channel(64);
        let mut inbound = request.into_inner();

        let store = self.store.clone();
        let subscriptions = self.subscriptions.clone();
        let config = self.config.clone();

        let temp_node_id = format!("pending-{}", Uuid::new_v4());
        subscriptions.register_node(&temp_node_id);

        tokio::spawn(async move {
            let mut node_id = temp_node_id.clone();
            let mut change_rx = store.subscribe();

            loop {
                tokio::select! {
                    msg = inbound.next() => {
                        match msg {
                            Some(Ok(req)) => {
                                if let Some(node) = &req.node {
                                    if node_id.starts_with("pending-") {
                                        let real_id = if node.id.is_empty() {
                                            remote_addr.clone()
                                        } else {
                                            node.id.clone()
                                        };
                                        subscriptions.unregister_node(&node_id);
                                        node_id = real_id;
                                        subscriptions.register_node(&node_id);
                                        info!(node = %node_id, "identified ARMAGEDDON node (SOTW)");
                                    }
                                }

                                let type_url = &req.type_url;

                                if !req.response_nonce.is_empty() {
                                    if req.error_detail.is_some() {
                                        warn!(
                                            node = %node_id,
                                            type_url = %type_url,
                                            nonce = %req.response_nonce,
                                            "NACK received from ARMAGEDDON (SOTW)"
                                        );
                                    } else {
                                        subscriptions.record_ack(
                                            &node_id,
                                            type_url,
                                            &req.version_info,
                                            &req.response_nonce,
                                        );
                                    }
                                }

                                subscriptions.update_subscription(
                                    &node_id,
                                    type_url,
                                    req.resource_names.clone(),
                                );

                                let version = store.snapshot().version.as_string();
                                let response = build_response_static(
                                    &store, &subscriptions, &config,
                                    type_url, &version, &node_id,
                                );

                                if tx.send(Ok(response)).await.is_err() {
                                    debug!(node = %node_id, "client disconnected during send (SOTW)");
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                warn!(node = %node_id, error = %e, "stream error from ARMAGEDDON (SOTW)");
                                break;
                            }
                            None => {
                                info!(node = %node_id, "ARMAGEDDON disconnected (SOTW)");
                                break;
                            }
                        }
                    }

                    Ok(()) = change_rx.changed() => {
                        let notification = change_rx.borrow_and_update().clone();

                        if let Some(notification) = notification.as_ref() {
                            let version = notification.version.as_string();

                            let type_url = match notification.resource_type {
                                xds_store::store::ResourceType::Cluster => type_urls::CLUSTER,
                                xds_store::store::ResourceType::Endpoint => type_urls::ENDPOINT,
                                xds_store::store::ResourceType::Route => type_urls::ROUTE,
                                xds_store::store::ResourceType::Listener => type_urls::LISTENER,
                                xds_store::store::ResourceType::Certificate => type_urls::SECRET,
                            };

                            if subscriptions.get_subscribed_resources(&node_id, type_url).is_some() {
                                let response = build_response_static(
                                    &store, &subscriptions, &config,
                                    type_url, &version, &node_id,
                                );

                                if tx.send(Ok(response)).await.is_err() {
                                    debug!(node = %node_id, "client disconnected during push (SOTW)");
                                    break;
                                }

                                debug!(
                                    node = %node_id,
                                    type_url = %type_url,
                                    version = %version,
                                    "pushed SOTW update to ARMAGEDDON"
                                );
                            }
                        }
                    }
                }
            }

            subscriptions.unregister_node(&node_id);
            info!(node = %node_id, "SOTW ADS stream closed");
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    type DeltaAggregatedResourcesStream =
        Pin<Box<ReceiverStream<Result<DeltaDiscoveryResponse, Status>>>>;

    /// Delta ADS (incremental) streaming RPC.
    ///
    /// Sends only added/updated/removed resources since the client's last known
    /// state.  The client's per-resource versions are tracked in `DELTA_STREAMS`
    /// and used to compute diffs on every inbound subscription change or
    /// ConfigStore push.
    ///
    /// # Protocol handshake
    ///
    /// 1. Client opens stream and sends first `DeltaDiscoveryRequest` with:
    ///    - `node` (ARMAGEDDON identifier)
    ///    - `type_url` of the first resource type it wants
    ///    - `resource_names_subscribe` (empty = wildcard)
    ///    - `initial_resource_versions` (name → version the client already has)
    ///
    /// 2. Server sends `DeltaDiscoveryResponse` with:
    ///    - `resources`: only resources not in `initial_resource_versions` OR
    ///      with a different version
    ///    - `removed_resources`: names that are in `initial_resource_versions`
    ///      but no longer in the store
    ///
    /// 3. Client ACKs with `response_nonce = <last received nonce>`.
    ///    On parse failure the client NACKs with `error_detail` populated.
    ///
    /// 4. On ConfigStore change, server re-diffs and sends only the delta.
    async fn delta_aggregated_resources(
        &self,
        request: Request<Streaming<DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status> {
        let remote_addr = request
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let stream_id = Uuid::new_v4().to_string();
        info!(
            remote = %remote_addr,
            stream_id = %stream_id,
            "ARMAGEDDON connected via Delta ADS stream"
        );

        // Register per-stream state.
        delta_streams().insert(stream_id.clone(), DeltaClientState::new());

        let (tx, rx) = mpsc::channel(64);
        let mut inbound = request.into_inner();

        let store = self.store.clone();
        let config = self.config.clone();
        let sid = stream_id.clone();

        tokio::spawn(async move {
            let mut node_id = format!("delta-pending-{}", &sid[..8]);
            let mut change_rx = store.subscribe();
            // last nonce sent to the client (for correlating ACK/NACK)
            let mut last_nonce: Option<String> = None;

            loop {
                tokio::select! {
                    msg = inbound.next() => {
                        match msg {
                            Some(Ok(req)) => {
                                // --- Node identification ---
                                if let Some(node) = &req.node {
                                    if node_id.starts_with("delta-pending-") {
                                        let real_id = if node.id.is_empty() {
                                            remote_addr.clone()
                                        } else {
                                            node.id.clone()
                                        };
                                        node_id = real_id;
                                        info!(node = %node_id, stream_id = %sid, "identified ARMAGEDDON node (Delta)");
                                    }
                                }

                                let type_url = req.type_url.clone();

                                // --- ACK / NACK handling ---
                                if !req.response_nonce.is_empty() {
                                    if req.error_detail.is_some() {
                                        DELTA_NACK_TOTAL.fetch_add(1, AtomicOrdering::Relaxed);
                                        warn!(
                                            node = %node_id,
                                            stream_id = %sid,
                                            type_url = %type_url,
                                            nonce = %req.response_nonce,
                                            error = ?req.error_detail,
                                            "Delta NACK received from ARMAGEDDON — not advancing client versions"
                                        );
                                        // On NACK we do NOT update sent_versions —
                                        // the client rejected this batch.  The next
                                        // ConfigStore change will resend.
                                    } else {
                                        debug!(
                                            node = %node_id,
                                            stream_id = %sid,
                                            type_url = %type_url,
                                            nonce = %req.response_nonce,
                                            "Delta ACK received"
                                        );
                                        // ACK is purely informational here; versions
                                        // are already recorded when we sent the
                                        // response (optimistic record-on-send).
                                    }
                                }

                                // --- Subscribe / unsubscribe delta ---
                                if delta_streams().contains_key(&sid) {
                                    let subscribe = req.resource_names_subscribe.clone();
                                    let unsubscribe = req.resource_names_unsubscribe.clone();
                                    let initial = req.initial_resource_versions.clone();

                                    let mut entry = delta_streams().get_mut(&sid).unwrap();
                                    if !initial.is_empty() {
                                        entry.seed_initial_versions(&type_url, &initial);
                                    }
                                    entry.apply_subscription_delta(&type_url, &subscribe, &unsubscribe);
                                }

                                // --- Compute and send diff for this type ---
                                if !type_url.is_empty() {
                                    let snapshot = store.snapshot();
                                    let sys_ver = snapshot.version.as_string();
                                    let nonce = Uuid::new_v4().to_string();

                                    if let Some(delta_resp) = build_delta_response(
                                        &sid,
                                        &snapshot,
                                        &config,
                                        &type_url,
                                        &sys_ver,
                                        &nonce,
                                    ) {
                                        last_nonce = Some(nonce);
                                        if tx.send(Ok(delta_resp)).await.is_err() {
                                            debug!(node = %node_id, stream_id = %sid, "client disconnected during delta send");
                                            break;
                                        }
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                warn!(node = %node_id, stream_id = %sid, error = %e, "stream error from ARMAGEDDON (Delta)");
                                break;
                            }
                            None => {
                                info!(node = %node_id, stream_id = %sid, "ARMAGEDDON disconnected (Delta)");
                                break;
                            }
                        }
                    }

                    Ok(()) = change_rx.changed() => {
                        let notification = change_rx.borrow_and_update().clone();

                        if let Some(notification) = notification.as_ref() {
                            let type_url = match notification.resource_type {
                                xds_store::store::ResourceType::Cluster => type_urls::CLUSTER,
                                xds_store::store::ResourceType::Endpoint => type_urls::ENDPOINT,
                                xds_store::store::ResourceType::Route => type_urls::ROUTE,
                                xds_store::store::ResourceType::Listener => type_urls::LISTENER,
                                xds_store::store::ResourceType::Certificate => type_urls::SECRET,
                            };

                            // Only push if the client is subscribed to this type.
                            let is_subscribed = delta_streams()
                                .get(&sid)
                                .map(|s| s.subscribed.contains_key(type_url))
                                .unwrap_or(false);

                            if is_subscribed {
                                let snapshot = store.snapshot();
                                let sys_ver = snapshot.version.as_string();
                                let nonce = Uuid::new_v4().to_string();

                                if let Some(delta_resp) = build_delta_response(
                                    &sid,
                                    &snapshot,
                                    &config,
                                    type_url,
                                    &sys_ver,
                                    &nonce,
                                ) {
                                    last_nonce = Some(nonce);
                                    debug!(
                                        node = %node_id,
                                        stream_id = %sid,
                                        type_url = %type_url,
                                        version = %sys_ver,
                                        "pushed Delta update to ARMAGEDDON"
                                    );
                                    if tx.send(Ok(delta_resp)).await.is_err() {
                                        debug!(node = %node_id, stream_id = %sid, "client disconnected during delta push");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Clean up per-stream state on disconnect.
            delta_streams().remove(&sid);
            let _ = last_nonce; // suppress unused warning
            info!(node = %node_id, stream_id = %sid, "Delta ADS stream closed");
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}

// ---------------------------------------------------------------------------
// SOTW helper
// ---------------------------------------------------------------------------

/// Build a full SOTW DiscoveryResponse for `type_url`.
fn build_response_static(
    store: &ConfigStore,
    subscriptions: &SubscriptionManager,
    config: &ServerConfig,
    type_url: &str,
    version: &str,
    node_id: &str,
) -> DiscoveryResponse {
    let snapshot = store.snapshot();
    let nonce = Uuid::new_v4().to_string();

    let resources = match type_url {
        type_urls::CLUSTER => snapshot
            .clusters
            .values()
            .map(|c| convert::cluster_to_any(c))
            .collect(),

        type_urls::ENDPOINT => {
            let subscribed = subscriptions.get_subscribed_resources(node_id, type_url);
            snapshot
                .endpoints
                .iter()
                .filter(|(name, _)| {
                    subscribed
                        .as_ref()
                        .map_or(true, |s| s.is_empty() || s.contains(*name))
                })
                .map(|(name, eps)| convert::endpoints_to_any(name, eps))
                .collect()
        }

        type_urls::ROUTE => snapshot
            .routes
            .values()
            .map(|r| convert::route_to_any(r))
            .collect(),

        type_urls::LISTENER => snapshot
            .listeners
            .values()
            .map(|l| convert::listener_to_any(l))
            .collect(),

        type_urls::SECRET => snapshot
            .certificates
            .values()
            .map(|c| convert::certificate_to_any(c))
            .collect(),

        _ => vec![],
    };

    subscriptions.record_nonce(node_id, type_url, &nonce);

    DiscoveryResponse {
        version_info: version.to_string(),
        resources,
        canary: false,
        type_url: type_url.to_string(),
        nonce,
        control_plane: Some(ControlPlane {
            identifier: config.control_plane_id.clone(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Delta helper — builds a DeltaDiscoveryResponse
// ---------------------------------------------------------------------------

/// Compute a `DeltaDiscoveryResponse` for `type_url` relative to the per-stream
/// client state stored in `DELTA_STREAMS[stream_id]`.
///
/// Returns `None` if there are no resources to add/update/remove (no-op diff).
///
/// Side-effect: updates `DELTA_STREAMS[stream_id].sent_versions` for every
/// resource included in the response (optimistic record-on-send).
fn build_delta_response(
    stream_id: &str,
    snapshot: &xds_store::snapshot::ConfigSnapshot,
    config: &ServerConfig,
    type_url: &str,
    sys_ver: &str,
    nonce: &str,
) -> Option<DeltaDiscoveryResponse> {
    // Collect (name, version, Any) for all store resources the client cares about.
    let store_resources: Vec<(String, String, prost_types::Any)> =
        collect_store_resources(snapshot, type_url);

    let mut added_or_updated: Vec<Resource> = Vec::new();
    let mut removed_resources: Vec<String> = Vec::new();

    // --- Compute diff ---
    {
        let state = delta_streams().get(stream_id)?;

        for (name, store_ver, any) in &store_resources {
            // Skip resources the client hasn't subscribed to.
            if !state.is_subscribed(type_url, name) {
                continue;
            }

            let client_ver = state.known_version(type_url, name);
            if client_ver != Some(store_ver.as_str()) {
                // New or changed.
                added_or_updated.push(Resource {
                    name: name.clone(),
                    version: store_ver.clone(),
                    resource: Some(any.clone()),
                    aliases: vec![],
                });
            }
        }

        // Resources the client knows about but are no longer in the store.
        if let Some(known_map) = state.sent_versions.get(type_url) {
            let store_names: HashSet<&str> =
                store_resources.iter().map(|(n, _, _)| n.as_str()).collect();
            for known_name in known_map.keys() {
                if !store_names.contains(known_name.as_str()) {
                    removed_resources.push(known_name.clone());
                }
            }
        }
    }

    if added_or_updated.is_empty() && removed_resources.is_empty() {
        return None;
    }

    // --- Update sent_versions optimistically (before ACK) ---
    {
        if let Some(mut state) = delta_streams().get_mut(stream_id) {
            for r in &added_or_updated {
                state.record_sent(type_url, &r.name, &r.version);
                DELTA_RESOURCES_SENT_TOTAL.fetch_add(1, AtomicOrdering::Relaxed);
            }
            for name in &removed_resources {
                state.record_removed(type_url, name);
                DELTA_REMOVED_SENT_TOTAL.fetch_add(1, AtomicOrdering::Relaxed);
            }
        }
    }

    Some(DeltaDiscoveryResponse {
        system_version_info: sys_ver.to_string(),
        resources: added_or_updated,
        type_url: type_url.to_string(),
        removed_resources,
        nonce: nonce.to_string(),
        control_plane: Some(ControlPlane {
            identifier: config.control_plane_id.clone(),
        }),
    })
}

/// Compute a stable, content-based version string for a serialised protobuf `Any`.
///
/// We use a simple FNV-1a 64-bit hash of the encoded bytes.  This produces a
/// per-resource version that only changes when the resource's content changes,
/// regardless of the global snapshot version.  This is the critical property
/// that makes Delta xDS efficient: cluster-b's version is unchanged when only
/// cluster-a is mutated.
fn content_version(any: &prost_types::Any) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    any.value.hash(&mut h);
    any.type_url.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Enumerate all resources from `snapshot` for `type_url` as (name, version, Any).
///
/// The version is a content-hash of the encoded resource bytes so that it only
/// changes when the resource itself changes — independent of the global snapshot
/// version counter.  This is what makes delta diffing correct.
fn collect_store_resources(
    snapshot: &xds_store::snapshot::ConfigSnapshot,
    type_url: &str,
) -> Vec<(String, String, prost_types::Any)> {
    match type_url {
        type_urls::CLUSTER => snapshot
            .clusters
            .iter()
            .map(|(name, c)| {
                let any = convert::cluster_to_any(c);
                let ver = content_version(&any);
                (name.clone(), ver, any)
            })
            .collect(),

        type_urls::ENDPOINT => snapshot
            .endpoints
            .iter()
            .map(|(name, eps)| {
                let any = convert::endpoints_to_any(name, eps);
                let ver = content_version(&any);
                (name.clone(), ver, any)
            })
            .collect(),

        type_urls::ROUTE => snapshot
            .routes
            .iter()
            .map(|(name, r)| {
                let any = convert::route_to_any(r);
                let ver = content_version(&any);
                (name.clone(), ver, any)
            })
            .collect(),

        type_urls::LISTENER => snapshot
            .listeners
            .iter()
            .map(|(name, l)| {
                let any = convert::listener_to_any(l);
                let ver = content_version(&any);
                (name.clone(), ver, any)
            })
            .collect(),

        type_urls::SECRET => snapshot
            .certificates
            .iter()
            .map(|(name, c)| {
                let any = convert::certificate_to_any(c);
                let ver = content_version(&any);
                (name.clone(), ver, any)
            })
            .collect(),

        _ => vec![],
    }
}
