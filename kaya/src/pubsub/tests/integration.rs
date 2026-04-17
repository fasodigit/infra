//! Integration tests for the KAYA Pub/Sub protocol.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use kaya_pubsub::{Pattern, PubSubBroker, ShardedPubSub, DEFAULT_SUBSCRIBER_CAPACITY};
use tokio::sync::mpsc;
use tokio::time::timeout;

fn ch(s: &str) -> Bytes {
    Bytes::from(s.to_owned())
}

#[tokio::test]
async fn subscribe_publish_roundtrip() {
    let broker = Arc::new(PubSubBroker::new());
    let mut sub = broker.subscribe_bounded(ch("events"), 16);

    let delivered = broker.publish(b"events", Bytes::from_static(b"hello")).await;
    assert_eq!(delivered, 1);

    let msg = timeout(Duration::from_secs(1), sub.receiver.recv())
        .await
        .expect("timed out")
        .expect("channel closed");
    assert_eq!(msg.channel.as_ref(), b"events");
    assert_eq!(msg.payload.as_ref(), b"hello");
    assert!(msg.pattern.is_none());
}

#[tokio::test]
async fn multi_subscriber_broadcast() {
    let broker = Arc::new(PubSubBroker::new());
    let mut subs: Vec<_> = (0..5)
        .map(|_| broker.subscribe_bounded(ch("news"), 16))
        .collect();

    let delivered = broker.publish(b"news", Bytes::from_static(b"breaking")).await;
    assert_eq!(delivered, 5);

    for sub in subs.iter_mut() {
        let msg = timeout(Duration::from_secs(1), sub.receiver.recv())
            .await
            .expect("timed out")
            .expect("channel closed");
        assert_eq!(msg.payload.as_ref(), b"breaking");
    }
}

#[tokio::test]
async fn unsubscribe_stops_delivery() {
    let broker = Arc::new(PubSubBroker::new());
    let sub = broker.subscribe_bounded(ch("c1"), 16);
    let sub_id = sub.id;
    let mut receiver = sub.receiver;

    broker.unsubscribe(sub_id);
    let delivered = broker.publish(b"c1", Bytes::from_static(b"payload")).await;
    assert_eq!(delivered, 0, "no subscribers after unsubscribe");
    assert_eq!(broker.subscriber_count(b"c1"), 0);
    assert_eq!(broker.channel_count(), 0);

    // Confirm no pending message: either timeout elapses, or the channel
    // has closed with no pending messages — never a delivery.
    match timeout(Duration::from_millis(50), receiver.recv()).await {
        Err(_) => {} // timeout: OK
        Ok(None) => {} // channel closed with no pending messages: OK
        Ok(Some(msg)) => panic!("no delivery expected after unsubscribe, got {msg:?}"),
    }
}

#[tokio::test]
async fn pattern_matches_glob_star() {
    let broker = Arc::new(PubSubBroker::new());
    let pattern = Pattern::compile(b"news.*").expect("compile pattern");
    let mut sub = broker
        .psubscribe_bounded(pattern, 16)
        .expect("psubscribe");

    let delivered = broker
        .publish(b"news.sport", Bytes::from_static(b"goal"))
        .await;
    assert_eq!(delivered, 1);

    let msg = timeout(Duration::from_secs(1), sub.receiver.recv())
        .await
        .expect("timed out")
        .expect("channel closed");
    assert_eq!(msg.channel.as_ref(), b"news.sport");
    assert!(msg.pattern.is_some());
    // RESP3 Pub/Sub parity: '*' spans '.'.
    let delivered2 = broker
        .publish(b"news.sport.football", Bytes::from_static(b"match"))
        .await;
    assert_eq!(delivered2, 1);
}

#[tokio::test]
async fn pattern_does_not_match_different_prefix() {
    let broker = Arc::new(PubSubBroker::new());
    let pattern = Pattern::compile(b"news.*").expect("compile pattern");
    let mut sub = broker
        .psubscribe_bounded(pattern, 16)
        .expect("psubscribe");

    let delivered = broker
        .publish(b"weather.today", Bytes::from_static(b"sunny"))
        .await;
    assert_eq!(delivered, 0);

    match timeout(Duration::from_millis(50), sub.receiver.recv()).await {
        Err(_) => {}
        Ok(None) => {}
        Ok(Some(msg)) => panic!("no delivery expected for non-matching channel, got {msg:?}"),
    }
}

#[tokio::test]
async fn sharded_publish_routes_consistently() {
    let sharded = ShardedPubSub::new(8);

    // Subscribe to the same channel via SSUBSCRIBE: it must land on the
    // shard that SPUBLISH targets.
    let channel = ch("orders.EU");
    let mut sub = sharded.ssubscribe_bounded(channel.clone(), 16);

    let delivered = sharded
        .spublish(b"orders.EU", Bytes::from_static(b"ord-1"))
        .await;
    assert_eq!(delivered, 1, "publish must reach the shard subscriber");

    let msg = timeout(Duration::from_secs(1), sub.receiver.recv())
        .await
        .expect("timed out")
        .expect("channel closed");
    assert_eq!(msg.payload.as_ref(), b"ord-1");

    // Same key must hash to the same shard on every call.
    let idx_a = sharded.shard_index(b"orders.EU");
    let idx_b = sharded.shard_index(b"orders.EU");
    assert_eq!(idx_a, idx_b);
}

#[tokio::test]
async fn backpressure_drops_instead_of_blocking() {
    let broker = Arc::new(PubSubBroker::new());

    // Slow subscriber with a tiny capacity and never reads.
    let (slow_tx, _slow_rx) = mpsc::channel(1);
    let _slow_id = broker.subscribe(ch("fast"), slow_tx);

    // Fast subscriber with ample capacity.
    let (fast_tx, mut fast_rx) = mpsc::channel(DEFAULT_SUBSCRIBER_CAPACITY);
    let _fast_id = broker.subscribe(ch("fast"), fast_tx);

    // Publish more than the slow subscriber's capacity.
    let total: u64 = 50;
    for i in 0..total {
        let payload = Bytes::from(format!("msg-{i}"));
        // publish should never block/hang even though slow is full.
        let _ = timeout(Duration::from_millis(100), broker.publish(b"fast", payload))
            .await
            .expect("publish should not block on slow subscriber");
    }

    // Fast subscriber must have received every message.
    let mut received: u64 = 0;
    while let Ok(Some(_)) = timeout(Duration::from_millis(50), fast_rx.recv()).await {
        received += 1;
        if received == total {
            break;
        }
    }
    assert_eq!(received, total, "fast subscriber received every message");
}
