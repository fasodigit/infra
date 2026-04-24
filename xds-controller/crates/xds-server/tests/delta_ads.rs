// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Integration tests for the Delta ADS (DeltaAggregatedResources) server handler.
//!
//! # Test matrix
//!
//! 1. `delta_initial_subscribe_sends_known_resources`
//!    — client subscribes (wildcard) with empty initial_resource_versions
//!    → server returns both clusters in the first DeltaDiscoveryResponse.
//!
//! 2. `delta_subsequent_update_sends_only_changed`
//!    — client ACKs initial response, server updates one cluster
//!    → only that cluster appears in the follow-up delta.
//!
//! 3. `delta_removed_resource_sent`
//!    — server removes a cluster after the client has ACK'd it
//!    → `removed_resources` contains the deleted name only.
//!
//! 4. `delta_ack_nack_handled`
//!    — client NACKs → counter increments, stream stays alive,
//!    server sends fresh delta on next store mutation.
//!
//! # Architecture
//!
//! Each test binds an `AdsService` on a random loopback port via tonic's
//! `Server::builder`.  A thin client built on `tonic::client::Grpc` drives
//! the `DeltaAggregatedResources` bidirectional stream directly, avoiding
//! the need for a generated client stub in this crate.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use http::uri::PathAndQuery;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt as _;
use tonic::codec::ProstCodec;
use tonic::transport::Server;
use tonic::Request;

use xds_server::ServerConfig;
use xds_server::generated::envoy::service::discovery::v3::{
    aggregated_discovery_service_server::AggregatedDiscoveryServiceServer,
    DeltaDiscoveryRequest, DeltaDiscoveryResponse, Node,
};
use xds_server::generated::google::rpc::Status as GrpcStatus;
use xds_store::{
    ConfigStore,
    model::{ClusterEntry, DiscoveryType, LbPolicy},
};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

const CLUSTER_URL: &str = "type.googleapis.com/envoy.config.cluster.v3.Cluster";

fn make_cluster(name: &str) -> ClusterEntry {
    ClusterEntry {
        name: name.to_string(),
        discovery_type: DiscoveryType::Static,
        lb_policy: LbPolicy::RoundRobin,
        connect_timeout_ms: 250,
        health_check: None,
        circuit_breaker: None,
        spiffe_id: None,
        metadata: HashMap::new(),
        updated_at: chrono::Utc::now(),
    }
}

fn subscribe(nonce: &str) -> DeltaDiscoveryRequest {
    DeltaDiscoveryRequest {
        node: Some(Node {
            id: "test-armageddon".to_string(),
            cluster: "test".to_string(),
            ..Default::default()
        }),
        type_url: CLUSTER_URL.to_string(),
        resource_names_subscribe: vec![],   // empty = wildcard
        resource_names_unsubscribe: vec![],
        initial_resource_versions: HashMap::new(),
        response_nonce: nonce.to_string(),
        error_detail: None,
    }
}

fn ack(nonce: &str) -> DeltaDiscoveryRequest {
    DeltaDiscoveryRequest {
        node: None,
        type_url: CLUSTER_URL.to_string(),
        resource_names_subscribe: vec![],
        resource_names_unsubscribe: vec![],
        initial_resource_versions: HashMap::new(),
        response_nonce: nonce.to_string(),
        error_detail: None,
    }
}

fn nack(nonce: &str, msg: &str) -> DeltaDiscoveryRequest {
    DeltaDiscoveryRequest {
        node: None,
        type_url: CLUSTER_URL.to_string(),
        resource_names_subscribe: vec![],
        resource_names_unsubscribe: vec![],
        initial_resource_versions: HashMap::new(),
        response_nonce: nonce.to_string(),
        error_detail: Some(GrpcStatus {
            code: 3,
            message: msg.to_string(),
            details: vec![],
        }),
    }
}

// ---------------------------------------------------------------------------
// Embedded server helper
// ---------------------------------------------------------------------------

async fn serve(store: ConfigStore) -> String {
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();

    let config = Arc::new(ServerConfig::default());
    let svc = xds_server::services::ads::AdsService::new(store, config);

    tokio::spawn(async move {
        Server::builder()
            .add_service(AggregatedDiscoveryServiceServer::new(svc))
            .serve_with_incoming(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .ok();
    });

    tokio::time::sleep(Duration::from_millis(40)).await;
    format!("http://{}", local_addr)
}

// ---------------------------------------------------------------------------
// Delta stream helper — uses tonic::client::Grpc directly
// ---------------------------------------------------------------------------

const DELTA_PATH: &str = "/envoy.service.discovery.v3.AggregatedDiscoveryService/DeltaAggregatedResources";

async fn open_delta(
    addr: &str,
) -> (
    mpsc::Sender<DeltaDiscoveryRequest>,
    tonic::Streaming<DeltaDiscoveryResponse>,
) {
    let channel = tonic::transport::Channel::from_shared(addr.to_string())
        .unwrap()
        .connect()
        .await
        .unwrap();

    let (tx, rx) = mpsc::channel::<DeltaDiscoveryRequest>(32);
    let outbound = ReceiverStream::new(rx);

    let mut grpc = tonic::client::Grpc::new(channel);
    grpc.ready().await.expect("channel ready");

    let codec: ProstCodec<DeltaDiscoveryRequest, DeltaDiscoveryResponse> = ProstCodec::default();
    let path = PathAndQuery::from_static(DELTA_PATH);

    let resp = grpc
        .streaming(Request::new(outbound), path, codec)
        .await
        .expect("delta streaming call succeeded");

    (tx, resp.into_inner())
}

// ---------------------------------------------------------------------------
// Test 1: delta_initial_subscribe_sends_known_resources
// ---------------------------------------------------------------------------

/// Wildcard subscribe with empty initial_resource_versions → server sends both clusters.
#[tokio::test]
async fn delta_initial_subscribe_sends_known_resources() {
    let store = ConfigStore::new();
    store.set_cluster(make_cluster("payments-v1")).unwrap();
    store.set_cluster(make_cluster("auth-svc")).unwrap();

    let addr = serve(store).await;
    let (tx, mut rx) = open_delta(&addr).await;

    tx.send(subscribe("")).await.unwrap();

    let resp = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .expect("response within 3s")
        .expect("stream open")
        .expect("no gRPC error");

    assert_eq!(resp.type_url, CLUSTER_URL);
    assert_eq!(resp.removed_resources.len(), 0, "no removals on initial subscribe");
    assert_eq!(
        resp.resources.len(),
        2,
        "initial subscribe must return both clusters; got: {:?}",
        resp.resources.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    let names: Vec<&str> = resp.resources.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"payments-v1"), "payments-v1 must be present");
    assert!(names.contains(&"auth-svc"), "auth-svc must be present");
}

// ---------------------------------------------------------------------------
// Test 2: delta_subsequent_update_sends_only_changed
// ---------------------------------------------------------------------------

/// After ACK, server updates one cluster → only that cluster in the delta push.
#[tokio::test]
async fn delta_subsequent_update_sends_only_changed() {
    let store = ConfigStore::new();
    store.set_cluster(make_cluster("cluster-a")).unwrap();
    store.set_cluster(make_cluster("cluster-b")).unwrap();

    let addr = serve(store.clone()).await;
    let (tx, mut rx) = open_delta(&addr).await;

    tx.send(subscribe("")).await.unwrap();

    let initial = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(initial.resources.len(), 2, "initial must have 2 resources");

    // ACK initial response.
    tx.send(ack(&initial.nonce)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Update only cluster-a.
    let mut updated = make_cluster("cluster-a");
    updated.connect_timeout_ms = 999;
    store.set_cluster(updated).unwrap();

    let delta = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .expect("delta push within 3s")
        .unwrap()
        .unwrap();

    assert_eq!(
        delta.resources.len(),
        1,
        "only changed cluster must be in delta; got: {:?}",
        delta.resources.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
    assert_eq!(delta.resources[0].name, "cluster-a", "changed cluster must be cluster-a");
    assert_eq!(delta.removed_resources.len(), 0, "no removals expected");
}

// ---------------------------------------------------------------------------
// Test 3: delta_removed_resource_sent
// ---------------------------------------------------------------------------

/// Server removes a cluster → `removed_resources` contains only that name.
#[tokio::test]
async fn delta_removed_resource_sent() {
    let store = ConfigStore::new();
    store.set_cluster(make_cluster("ephemeral")).unwrap();
    store.set_cluster(make_cluster("permanent")).unwrap();

    let addr = serve(store.clone()).await;
    let (tx, mut rx) = open_delta(&addr).await;

    tx.send(subscribe("")).await.unwrap();

    let initial = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(initial.resources.len(), 2);

    tx.send(ack(&initial.nonce)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Remove ephemeral cluster.
    store.remove_cluster("ephemeral").unwrap();

    let removal = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .expect("removal delta within 3s")
        .unwrap()
        .unwrap();

    assert!(
        removal.removed_resources.contains(&"ephemeral".to_string()),
        "'ephemeral' must be in removed_resources; got: {:?}",
        removal.removed_resources
    );
    // 'permanent' must NOT be re-sent in the same delta.
    assert!(
        !removal.resources.iter().any(|r| r.name == "permanent"),
        "'permanent' must not appear in resources for a removal-only delta"
    );
}

// ---------------------------------------------------------------------------
// Test 4: delta_ack_nack_handled
// ---------------------------------------------------------------------------

/// Client NACKs → counter increments, stream alive, server sends after next mutation.
#[tokio::test]
async fn delta_ack_nack_handled() {
    let store = ConfigStore::new();
    store.set_cluster(make_cluster("nack-cluster")).unwrap();

    let addr = serve(store.clone()).await;
    let (tx, mut rx) = open_delta(&addr).await;

    tx.send(subscribe("")).await.unwrap();

    let initial = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!initial.resources.is_empty(), "must receive at least one resource");

    let nack_before = xds_server::services::ads::delta_nack_total();

    // Send NACK.
    tx.send(nack(&initial.nonce, "simulated parse error")).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let nack_after = xds_server::services::ads::delta_nack_total();
    assert!(
        nack_after > nack_before,
        "NACK counter must increment (before={nack_before}, after={nack_after})"
    );

    // Trigger a store change — stream must still be alive.
    store.set_cluster(make_cluster("extra-cluster")).unwrap();

    let follow_up = tokio::time::timeout(Duration::from_secs(3), rx.next())
        .await
        .expect("follow-up delta within 3s after NACK")
        .unwrap()
        .unwrap();

    assert_eq!(follow_up.type_url, CLUSTER_URL);
    assert!(
        !follow_up.resources.is_empty(),
        "server must send resources after NACK + store update"
    );
}
