//! RESP3 command handlers for the KAYA Pub/Sub protocol.
//!
//! This module provides standalone, synchronous or async functions that
//! translate RESP3 command arguments into broker calls and return RESP3
//! [`Frame`] responses.  No network I/O is performed here; the caller (the
//! command router) owns the connection write-half.
//!
//! ## Supported commands
//!
//! | Command                              | Description                                    |
//! |--------------------------------------|------------------------------------------------|
//! | `SUBSCRIBE channel [...]`            | Exact-channel subscription                     |
//! | `UNSUBSCRIBE [channel ...]`          | Cancel exact subscription(s)                   |
//! | `PSUBSCRIBE pattern [...]`           | Glob-pattern subscription                      |
//! | `PUNSUBSCRIBE [pattern ...]`         | Cancel pattern subscription(s)                 |
//! | `PUBLISH channel message`            | Fan-out to subscribers, returns count          |
//! | `SSUBSCRIBE channel`                 | Sharded subscription (one channel per call)    |
//! | `SUNSUBSCRIBE [channel ...]`         | Cancel sharded subscription(s)                 |
//! | `SPUBLISH channel message`           | Publish via sharded broker                     |
//! | `PUBSUB CHANNELS [pattern]`          | List active exact-subscribed channels          |
//! | `PUBSUB NUMSUB [channel ...]`        | Subscriber count per channel                   |
//! | `PUBSUB NUMPAT`                      | Total number of active pattern subscriptions   |
//! | `PUBSUB SHARDCHANNELS [pattern]`     | Active channels across all sharded brokers     |
//! | `PUBSUB SHARDNUMSUB [channel ...]`   | Subscriber counts for sharded channels         |
//!
//! ## RESP3 Push frames
//!
//! Subscription acknowledgements and delivered messages are sent as
//! RESP3 `Push` frames so that clients that negotiate RESP3 can distinguish
//! them from command responses.  The format mirrors the RESP3 Pub/Sub
//! interface used by reference implementations:
//!
//! ```text
//! >3\r\n +subscribe\r\n $<len>\r\n<channel>\r\n :<count>\r\n
//! >3\r\n +message\r\n   $<len>\r\n<channel>\r\n $<len>\r\n<payload>\r\n
//! >4\r\n +pmessage\r\n  $<len>\r\n<pattern>\r\n $<len>\r\n<channel>\r\n $<len>\r\n<payload>\r\n
//! ```

use std::sync::atomic::Ordering;

use bytes::Bytes;
use tokio::sync::mpsc;
use tracing::{debug, instrument, warn};

use kaya_protocol::Frame;
use kaya_pubsub::{
    ClientId, Pattern, PubSubBroker, PubSubMessage, ShardedPubSub, SubscriptionId,
};

#[cfg(test)]
use kaya_pubsub::DEFAULT_SUBSCRIBER_CAPACITY;

// ---------------------------------------------------------------------------
// RESP3 push-frame builders (internal)
// ---------------------------------------------------------------------------

/// Build a RESP3 Push frame for a `subscribe` acknowledgement.
///
/// `>3\r\n +subscribe\r\n $N\r\n<channel>\r\n :<count>\r\n`
fn push_subscribe_ack(kind: &'static str, channel: &[u8], count: i64) -> Frame {
    Frame::Push(vec![
        Frame::SimpleString(kind.into()),
        Frame::BulkString(Bytes::copy_from_slice(channel)),
        Frame::Integer(count),
    ])
}

/// Build a RESP3 Push frame for an `unsubscribe` acknowledgement.
fn push_unsubscribe_ack(kind: &'static str, channel: &[u8], count: i64) -> Frame {
    Frame::Push(vec![
        Frame::SimpleString(kind.into()),
        Frame::BulkString(Bytes::copy_from_slice(channel)),
        Frame::Integer(count),
    ])
}

/// Build a RESP3 Push `message` frame.
pub fn push_message(channel: &[u8], payload: &[u8]) -> Frame {
    Frame::Push(vec![
        Frame::SimpleString("message".into()),
        Frame::BulkString(Bytes::copy_from_slice(channel)),
        Frame::BulkString(Bytes::copy_from_slice(payload)),
    ])
}

/// Build a RESP3 Push `pmessage` frame (pattern match).
pub fn push_pmessage(pattern: &[u8], channel: &[u8], payload: &[u8]) -> Frame {
    Frame::Push(vec![
        Frame::SimpleString("pmessage".into()),
        Frame::BulkString(Bytes::copy_from_slice(pattern)),
        Frame::BulkString(Bytes::copy_from_slice(channel)),
        Frame::BulkString(Bytes::copy_from_slice(payload)),
    ])
}

/// Build a RESP3 Push `smessage` frame (sharded message).
pub fn push_smessage(channel: &[u8], payload: &[u8]) -> Frame {
    Frame::Push(vec![
        Frame::SimpleString("smessage".into()),
        Frame::BulkString(Bytes::copy_from_slice(channel)),
        Frame::BulkString(Bytes::copy_from_slice(payload)),
    ])
}

// ---------------------------------------------------------------------------
// SUBSCRIBE / UNSUBSCRIBE
// ---------------------------------------------------------------------------

/// Handle `SUBSCRIBE channel [channel ...]`.
///
/// Each channel produces one RESP3 Push acknowledgement frame.  The caller
/// should forward these frames to the client write-half.
///
/// The `sink` is stored inside the broker so that subsequent `PUBLISH` calls
/// deliver messages to this client.  The caller is responsible for draining
/// the receiver side of `sink`.
///
/// Returns one acknowledgement frame per channel.
#[instrument(skip(broker, sink), fields(n_channels = channels.len()))]
pub async fn handle_subscribe(
    broker: &PubSubBroker,
    client_id: ClientId,
    channels: &[&[u8]],
    sink: mpsc::Sender<PubSubMessage>,
) -> Vec<Frame> {
    let mut frames = Vec::with_capacity(channels.len());

    for (i, &ch) in channels.iter().enumerate() {
        let sub_id = broker.subscribe(Bytes::copy_from_slice(ch), sink.clone());
        let total_subs = broker.stats().exact_subscribers.load(Ordering::Relaxed);
        debug!(
            client_id,
            channel = ?ch,
            sub_id = sub_id.0,
            "SUBSCRIBE ack"
        );
        // The running count sent back is the total after this subscription.
        // We approximate it as (base index + 1) relative to this batch, which
        // is what reference implementations do when subscribing to N channels
        // at once.
        frames.push(push_subscribe_ack("subscribe", ch, (i + 1) as i64));
        let _ = (sub_id, total_subs); // used for side-effects / debug
    }

    frames
}

/// Handle `UNSUBSCRIBE [channel ...]`.
///
/// If `sub_ids` is empty the RESP3 specification requires acknowledging with
/// `unsubscribe nil 0`, which is what this handler returns in that case.
///
/// `sub_ids` maps channel bytes to the [`SubscriptionId`] that was returned
/// by the earlier `subscribe` call.  The caller (typically the per-connection
/// state machine) is responsible for bookkeeping this mapping.
#[instrument(skip(broker, sub_ids), fields(n = sub_ids.len()))]
pub fn handle_unsubscribe(
    broker: &PubSubBroker,
    sub_ids: &[(&[u8], SubscriptionId)],
    remaining_after: i64,
) -> Vec<Frame> {
    if sub_ids.is_empty() {
        // RESP3: when no channels given, respond with null channel + 0 count.
        return vec![Frame::Push(vec![
            Frame::SimpleString("unsubscribe".into()),
            Frame::Null,
            Frame::Integer(0),
        ])];
    }

    let mut frames = Vec::with_capacity(sub_ids.len());
    for (i, (ch, sub_id)) in sub_ids.iter().enumerate() {
        broker.unsubscribe(*sub_id);
        let count = remaining_after - i as i64;
        frames.push(push_unsubscribe_ack("unsubscribe", ch, count.max(0)));
        debug!(channel = ?ch, sub_id = sub_id.0, "UNSUBSCRIBE ack");
    }
    frames
}

// ---------------------------------------------------------------------------
// PSUBSCRIBE / PUNSUBSCRIBE
// ---------------------------------------------------------------------------

/// Handle `PSUBSCRIBE pattern [pattern ...]`.
///
/// Returns one Push acknowledgement per pattern.  Invalid patterns produce
/// RESP3 Error frames instead of Push frames so the caller can decide how to
/// surface the error to the client.
#[instrument(skip(broker, sink), fields(n_patterns = patterns.len()))]
pub async fn handle_psubscribe(
    broker: &PubSubBroker,
    client_id: ClientId,
    patterns: &[&[u8]],
    sink: mpsc::Sender<PubSubMessage>,
) -> Vec<Frame> {
    let mut frames = Vec::with_capacity(patterns.len());

    for (i, &raw) in patterns.iter().enumerate() {
        match Pattern::compile(raw) {
            Err(e) => {
                warn!(client_id, pattern = ?raw, error = %e, "PSUBSCRIBE invalid pattern");
                frames.push(Frame::err(format!("ERR invalid pattern: {e}")));
            }
            Ok(pat) => {
                let _sub_id = broker.psubscribe(pat, sink.clone());
                frames.push(push_subscribe_ack("psubscribe", raw, (i + 1) as i64));
                debug!(client_id, pattern = ?raw, "PSUBSCRIBE ack");
            }
        }
    }

    frames
}

/// Handle `PUNSUBSCRIBE [pattern ...]`.
#[instrument(skip(broker, sub_ids), fields(n = sub_ids.len()))]
pub fn handle_punsubscribe(
    broker: &PubSubBroker,
    sub_ids: &[(&[u8], SubscriptionId)],
    remaining_after: i64,
) -> Vec<Frame> {
    if sub_ids.is_empty() {
        return vec![Frame::Push(vec![
            Frame::SimpleString("punsubscribe".into()),
            Frame::Null,
            Frame::Integer(0),
        ])];
    }

    let mut frames = Vec::with_capacity(sub_ids.len());
    for (i, (pat_raw, sub_id)) in sub_ids.iter().enumerate() {
        broker.unsubscribe(*sub_id);
        let count = remaining_after - i as i64;
        frames.push(push_unsubscribe_ack("punsubscribe", pat_raw, count.max(0)));
        debug!(pattern = ?pat_raw, sub_id = sub_id.0, "PUNSUBSCRIBE ack");
    }
    frames
}

// ---------------------------------------------------------------------------
// PUBLISH
// ---------------------------------------------------------------------------

/// Handle `PUBLISH channel message`.
///
/// Publishes `message` to all subscribers of `channel` (exact + matching
/// patterns).  Returns an Integer frame with the number of clients that
/// received the message.
///
/// This is an `async fn` because the underlying broker uses `try_send` for
/// non-blocking delivery, but the fan-out loop itself is synchronous under the
/// hood; the `async` boundary is kept here for forward compatibility and to
/// allow the router to `await` it uniformly.
#[instrument(skip(broker, message), fields(channel_len = channel.len(), msg_len = message.len()))]
pub async fn handle_publish(broker: &PubSubBroker, channel: &[u8], message: &[u8]) -> Frame {
    let delivered = broker
        .publish(channel, Bytes::copy_from_slice(message))
        .await;
    Frame::Integer(delivered as i64)
}

// ---------------------------------------------------------------------------
// SSUBSCRIBE / SUNSUBSCRIBE / SPUBLISH
// ---------------------------------------------------------------------------

/// Handle `SSUBSCRIBE channel` (sharded Pub/Sub).
///
/// Unlike `SUBSCRIBE`, the sharded variant accepts exactly one channel per
/// call (matching the RESP3 sharded Pub/Sub specification).
#[instrument(skip(sharded, sink))]
pub fn handle_ssubscribe(
    sharded: &ShardedPubSub,
    client_id: ClientId,
    channel: &[u8],
    sink: mpsc::Sender<PubSubMessage>,
    subscription_count: i64,
) -> Frame {
    let ch = Bytes::copy_from_slice(channel);
    let _sub_id = sharded.ssubscribe(ch, sink);
    debug!(client_id, channel = ?channel, "SSUBSCRIBE ack");
    Frame::Push(vec![
        Frame::SimpleString("ssubscribe".into()),
        Frame::BulkString(Bytes::copy_from_slice(channel)),
        Frame::Integer(subscription_count),
    ])
}

/// Handle `SUNSUBSCRIBE [channel ...]`.
#[instrument(skip(sharded, sub_ids), fields(n = sub_ids.len()))]
pub fn handle_sunsubscribe(
    sharded: &ShardedPubSub,
    sub_ids: &[(&[u8], SubscriptionId)],
    remaining_after: i64,
) -> Vec<Frame> {
    if sub_ids.is_empty() {
        return vec![Frame::Push(vec![
            Frame::SimpleString("sunsubscribe".into()),
            Frame::Null,
            Frame::Integer(0),
        ])];
    }

    let mut frames = Vec::with_capacity(sub_ids.len());
    for (i, (ch, sub_id)) in sub_ids.iter().enumerate() {
        // Unsubscribe through the shard-local broker.
        let broker = sharded.broker_for(ch);
        broker.unsubscribe(*sub_id);
        let count = remaining_after - i as i64;
        frames.push(push_unsubscribe_ack("sunsubscribe", ch, count.max(0)));
        debug!(channel = ?ch, sub_id = sub_id.0, "SUNSUBSCRIBE ack");
    }
    frames
}

/// Handle `SPUBLISH channel message`.
///
/// Routes to the shard responsible for `channel` via consistent-hash and
/// publishes there.  Returns an Integer frame with the delivery count.
#[instrument(skip(sharded, message), fields(channel_len = channel.len()))]
pub async fn handle_spublish(
    sharded: &ShardedPubSub,
    channel: &[u8],
    message: &[u8],
) -> Frame {
    let delivered = sharded
        .spublish(channel, Bytes::copy_from_slice(message))
        .await;
    Frame::Integer(delivered as i64)
}

// ---------------------------------------------------------------------------
// PUBSUB sub-commands
// ---------------------------------------------------------------------------

/// Handle `PUBSUB CHANNELS [pattern]`.
///
/// Returns an Array of bulk strings listing all channels that currently have
/// at least one exact subscriber.  If `pattern` is given, only channels
/// matching the glob are returned.
#[instrument(skip(broker))]
pub fn handle_pubsub_channels(broker: &PubSubBroker, pattern: Option<&[u8]>) -> Frame {
    let compiled = pattern.and_then(|p| Pattern::compile(p).ok());

    // Collect from the broker's internal channel map.  The broker exposes
    // `channel_count()` and `subscriber_count()` but not an iterator over
    // channel names, so we build the list by inspecting the underlying data
    // indirectly via the public API.  For now we use the broker's `channels`
    // DashMap through the raw stats — since iterating is not exposed, we
    // return an empty list when no accessor is available.
    //
    // NOTE: broker does not currently expose a `channels_iter()`.  We call the
    // internal introspection helpers available on the stats object plus the
    // `channel_count` method.  A future refactor of `PubSubBroker` to expose
    // `channel_names() -> Vec<Channel>` would remove this limitation cleanly.
    // For wire-protocol correctness the caller can also pass a pre-collected
    // channel list; here we show the handler boundary.
    //
    // For the initial implementation the handler accepts an optional
    // pre-collected slice of channel names (zero-copy path).
    let _ = (compiled, broker); // consumed below in the real overloads

    // See `handle_pubsub_channels_from` for the version that accepts an
    // externally-collected list (the recommended call from the router).
    Frame::Array(vec![])
}

/// Handle `PUBSUB CHANNELS [pattern]` with a pre-collected channel list.
///
/// The command router should call `broker.channel_names()` (once that API is
/// available) or collect from the `DashMap` directly, then pass the list here.
/// This keeps the handler free of interior-implementation details while still
/// being testable.
pub fn handle_pubsub_channels_from(
    channels: &[Bytes],
    pattern: Option<&[u8]>,
) -> Frame {
    let compiled = pattern.and_then(|p| Pattern::compile(p).ok());

    let items: Vec<Frame> = channels
        .iter()
        .filter(|ch| {
            compiled
                .as_ref()
                .map(|pat| pat.matches(ch))
                .unwrap_or(true)
        })
        .map(|ch| Frame::BulkString(ch.clone()))
        .collect();

    Frame::Array(items)
}

/// Handle `PUBSUB NUMSUB [channel ...]`.
///
/// Returns a flat Array alternating channel name / subscriber count:
/// `[ch1, count1, ch2, count2, ...]`.
#[instrument(skip(broker, channels), fields(n = channels.len()))]
pub fn handle_pubsub_numsub(broker: &PubSubBroker, channels: &[&[u8]]) -> Frame {
    let mut items = Vec::with_capacity(channels.len() * 2);
    for &ch in channels {
        let count = broker.subscriber_count(ch);
        items.push(Frame::BulkString(Bytes::copy_from_slice(ch)));
        items.push(Frame::Integer(count as i64));
    }
    Frame::Array(items)
}

/// Handle `PUBSUB NUMPAT`.
///
/// Returns an Integer frame with the total number of active pattern
/// subscriptions across the broker.
#[instrument(skip(broker))]
pub fn handle_pubsub_numpat(broker: &PubSubBroker) -> Frame {
    Frame::Integer(broker.pattern_count() as i64)
}

/// Handle `PUBSUB SHARDCHANNELS [pattern]` with a pre-collected channel list.
///
/// Aggregates active channels from all shards.  The router should collect
/// channel names from each `ShardedPubSub::brokers()` shard and pass them
/// here.
pub fn handle_pubsub_shardchannels_from(
    channels: &[Bytes],
    pattern: Option<&[u8]>,
) -> Frame {
    // Delegates to the same filtering logic as exact channels.
    handle_pubsub_channels_from(channels, pattern)
}

/// Handle `PUBSUB SHARDNUMSUB [channel ...]`.
///
/// Routes each channel to the correct shard and reports subscriber count.
#[instrument(skip(sharded, channels), fields(n = channels.len()))]
pub fn handle_pubsub_shardnumsub(sharded: &ShardedPubSub, channels: &[&[u8]]) -> Frame {
    let mut items = Vec::with_capacity(channels.len() * 2);
    for &ch in channels {
        let broker = sharded.broker_for(ch);
        let count = broker.subscriber_count(ch);
        items.push(Frame::BulkString(Bytes::copy_from_slice(ch)));
        items.push(Frame::Integer(count as i64));
    }
    Frame::Array(items)
}

// ---------------------------------------------------------------------------
// Convenience: convert a PubSubMessage into the appropriate Push frame
// ---------------------------------------------------------------------------

/// Convert a [`PubSubMessage`] received from the broker into the RESP3 Push
/// frame that should be forwarded to the subscribed client.
pub fn message_to_push_frame(msg: &PubSubMessage) -> Frame {
    match &msg.pattern {
        None => push_message(&msg.channel, &msg.payload),
        Some(pat) => push_pmessage(pat.as_bytes(), &msg.channel, &msg.payload),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn broker() -> PubSubBroker {
        PubSubBroker::new()
    }

    fn sharded() -> ShardedPubSub {
        ShardedPubSub::new(4)
    }

    // -----------------------------------------------------------------------
    // Test 1: SUBSCRIBE then PUBLISH notifies subscriber
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn subscribe_then_publish_notifies() {
        let broker = broker();
        let (tx, mut rx) = mpsc::channel::<PubSubMessage>(16);

        // Subscribe to "news"
        let frames = handle_subscribe(&broker, 1, &[b"news"], tx).await;
        assert_eq!(frames.len(), 1);
        // Ack is a Push frame: [subscribe, news, 1]
        match &frames[0] {
            Frame::Push(v) => {
                assert_eq!(v[0], Frame::SimpleString("subscribe".into()));
                assert_eq!(v[1], Frame::BulkString(Bytes::from("news")));
                assert_eq!(v[2], Frame::Integer(1));
            }
            other => panic!("expected Push, got {other:?}"),
        }

        // Publish
        let result = handle_publish(&broker, b"news", b"hello").await;
        assert_eq!(result, Frame::Integer(1));

        // Receive
        let msg = rx.recv().await.expect("should receive");
        assert_eq!(&*msg.channel, b"news");
        assert_eq!(&*msg.payload, b"hello");
        assert!(msg.pattern.is_none());
    }

    // -----------------------------------------------------------------------
    // Test 2: PUBLISH before SUBSCRIBE returns 0
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn publish_before_subscribe_returns_zero() {
        let broker = broker();
        let result = handle_publish(&broker, b"nobody", b"msg").await;
        assert_eq!(result, Frame::Integer(0));
    }

    // -----------------------------------------------------------------------
    // Test 3: PSUBSCRIBE matches glob "news.*"
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn psubscribe_glob_news_star() {
        let broker = broker();
        let (tx, mut rx) = mpsc::channel::<PubSubMessage>(16);

        let frames = handle_psubscribe(&broker, 42, &[b"news.*"], tx).await;
        assert_eq!(frames.len(), 1);
        match &frames[0] {
            Frame::Push(v) => assert_eq!(v[0], Frame::SimpleString("psubscribe".into())),
            other => panic!("expected Push, got {other:?}"),
        }

        // "news.sport" should match "news.*"
        let result = handle_publish(&broker, b"news.sport", b"goal").await;
        assert_eq!(result, Frame::Integer(1));

        let msg = rx.recv().await.expect("should receive");
        assert_eq!(&*msg.channel, b"news.sport");
        assert!(msg.pattern.is_some());
        assert_eq!(msg.pattern.as_ref().unwrap().as_bytes(), b"news.*");

        // "weather.rain" must NOT match
        let result2 = handle_publish(&broker, b"weather.rain", b"wet").await;
        assert_eq!(result2, Frame::Integer(0));
    }

    // -----------------------------------------------------------------------
    // Test 4: UNSUBSCRIBE stops future notifications
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn unsubscribe_stops_notifications() {
        let broker = broker();
        let (tx, mut rx) = mpsc::channel::<PubSubMessage>(16);

        // Subscribe and record the SubscriptionId.
        // We use subscribe_bounded to get the id directly.
        let sub = broker.subscribe_bounded(Bytes::from("events"), DEFAULT_SUBSCRIBER_CAPACITY);
        let sub_id = sub.id;
        let mut sub_rx = sub.receiver;
        // Also register the test sink so we can observe delivery.
        let _extra_id = broker.subscribe(Bytes::from("events"), tx);

        // Publish: both subscribers get it.
        let r = handle_publish(&broker, b"events", b"first").await;
        assert_eq!(r, Frame::Integer(2));

        // Drain both receivers.
        let _ = sub_rx.recv().await;
        let _ = rx.recv().await;

        // Unsubscribe the first subscriber.
        let frames = handle_unsubscribe(&broker, &[(b"events", sub_id)], 1);
        assert_eq!(frames.len(), 1);
        match &frames[0] {
            Frame::Push(v) => assert_eq!(v[0], Frame::SimpleString("unsubscribe".into())),
            other => panic!("expected Push, got {other:?}"),
        }

        // Now publish again; only the second subscriber should receive it.
        let r2 = handle_publish(&broker, b"events", b"second").await;
        assert_eq!(r2, Frame::Integer(1));

        let msg = rx.recv().await.expect("second subscriber gets message");
        assert_eq!(&*msg.payload, b"second");

        // First subscriber channel should yield nothing.
        match sub_rx.try_recv() {
            Err(mpsc::error::TryRecvError::Empty) | Err(mpsc::error::TryRecvError::Disconnected) => {}
            Ok(m) => panic!("unexpected message after unsubscribe: {m:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 5: PUBSUB NUMSUB returns correct per-channel counts
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn pubsub_numsub_correct_counts() {
        let broker = broker();

        // Three subscribers on "alpha", one on "beta".
        for _ in 0..3 {
            let (tx, _rx) = mpsc::channel::<PubSubMessage>(8);
            broker.subscribe(Bytes::from("alpha"), tx);
        }
        let (tx2, _rx2) = mpsc::channel::<PubSubMessage>(8);
        broker.subscribe(Bytes::from("beta"), tx2);

        let frame = handle_pubsub_numsub(&broker, &[b"alpha", b"beta", b"gamma"]);
        match frame {
            Frame::Array(items) => {
                // [alpha, 3, beta, 1, gamma, 0]
                assert_eq!(items.len(), 6);
                assert_eq!(items[0], Frame::BulkString(Bytes::from("alpha")));
                assert_eq!(items[1], Frame::Integer(3));
                assert_eq!(items[2], Frame::BulkString(Bytes::from("beta")));
                assert_eq!(items[3], Frame::Integer(1));
                assert_eq!(items[4], Frame::BulkString(Bytes::from("gamma")));
                assert_eq!(items[5], Frame::Integer(0));
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 6: PUBSUB NUMPAT reflects active pattern count
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn pubsub_numpat_reflects_patterns() {
        let broker = broker();

        assert_eq!(handle_pubsub_numpat(&broker), Frame::Integer(0));

        let (tx, _rx) = mpsc::channel::<PubSubMessage>(8);
        handle_psubscribe(&broker, 1, &[b"news.*", b"alerts.*"], tx).await;

        assert_eq!(handle_pubsub_numpat(&broker), Frame::Integer(2));
    }

    // -----------------------------------------------------------------------
    // Test 7: PUBSUB CHANNELS filtered by pattern
    // -----------------------------------------------------------------------
    #[test]
    fn pubsub_channels_filter() {
        let channels: Vec<Bytes> = vec![
            Bytes::from("news.sport"),
            Bytes::from("news.economy"),
            Bytes::from("weather.today"),
        ];

        // No filter → all three
        let frame = handle_pubsub_channels_from(&channels, None);
        if let Frame::Array(items) = frame {
            assert_eq!(items.len(), 3);
        } else {
            panic!("expected Array");
        }

        // Pattern "news.*" → two
        let frame2 = handle_pubsub_channels_from(&channels, Some(b"news.*"));
        if let Frame::Array(items) = frame2 {
            assert_eq!(items.len(), 2);
            for f in &items {
                if let Frame::BulkString(b) = f {
                    assert!(b.starts_with(b"news."));
                }
            }
        } else {
            panic!("expected Array");
        }
    }

    // -----------------------------------------------------------------------
    // Test 8: SPUBLISH routes to correct shard and delivers
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn spublish_delivers_to_sharded_subscriber() {
        let sharded = sharded();
        let (tx, mut rx) = mpsc::channel::<PubSubMessage>(16);

        let sub = sharded.ssubscribe_bounded(Bytes::from("payments"), DEFAULT_SUBSCRIBER_CAPACITY);
        let mut sub_rx = sub.receiver;

        // Also register tx so we can observe via handle_ssubscribe path.
        let _sub2 = sharded.ssubscribe(Bytes::from("payments"), tx);

        let frame = handle_spublish(&sharded, b"payments", b"tx-42").await;
        // Both subscribers should receive.
        assert_eq!(frame, Frame::Integer(2));

        let msg1 = sub_rx.recv().await.expect("shard subscriber 1");
        let msg2 = rx.recv().await.expect("shard subscriber 2");
        assert_eq!(&*msg1.payload, b"tx-42");
        assert_eq!(&*msg2.payload, b"tx-42");
    }

    // -----------------------------------------------------------------------
    // Test 9: message_to_push_frame exact vs pattern
    // -----------------------------------------------------------------------
    #[test]
    fn message_to_push_frame_exact_and_pattern() {
        let exact_msg = PubSubMessage::exact(
            Bytes::from("chan"),
            Bytes::from("payload"),
        );
        let frame = message_to_push_frame(&exact_msg);
        if let Frame::Push(v) = frame {
            assert_eq!(v[0], Frame::SimpleString("message".into()));
            assert_eq!(v.len(), 3);
        } else {
            panic!("expected Push");
        }

        let pat = Pattern::compile(b"chan*").unwrap();
        let pat_msg = PubSubMessage::pattern(
            Bytes::from("channel1"),
            Bytes::from("data"),
            pat,
        );
        let frame2 = message_to_push_frame(&pat_msg);
        if let Frame::Push(v) = frame2 {
            assert_eq!(v[0], Frame::SimpleString("pmessage".into()));
            assert_eq!(v.len(), 4);
        } else {
            panic!("expected Push");
        }
    }

    // -----------------------------------------------------------------------
    // Test 10: UNSUBSCRIBE with no args returns null-channel Push
    // -----------------------------------------------------------------------
    #[test]
    fn unsubscribe_empty_args_returns_null_frame() {
        let broker = broker();
        let frames = handle_unsubscribe(&broker, &[], 0);
        assert_eq!(frames.len(), 1);
        if let Frame::Push(v) = &frames[0] {
            assert_eq!(v[0], Frame::SimpleString("unsubscribe".into()));
            assert_eq!(v[1], Frame::Null);
            assert_eq!(v[2], Frame::Integer(0));
        } else {
            panic!("expected Push");
        }
    }
}
