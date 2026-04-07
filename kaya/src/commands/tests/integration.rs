//! Integration tests for KAYA core functionality.
//!
//! Tests the full command pipeline: Command -> CommandRouter -> Store/Streams.

use std::sync::Arc;

use bytes::Bytes;

use kaya_commands::{CommandContext, CommandRouter};
use kaya_protocol::{Command, Frame};
use kaya_store::{BloomManager, Store};
use kaya_streams::StreamManager;

/// Create a test context with defaults.
fn test_ctx() -> Arc<CommandContext> {
    let store = Arc::new(Store::default());
    let streams = Arc::new(StreamManager::default());
    let blooms = Arc::new(BloomManager::new());
    Arc::new(CommandContext::new(store, streams, blooms))
}

/// Build a Command from string arguments.
fn cmd(name: &str, args: &[&str]) -> Command {
    let frame = Frame::Array(
        std::iter::once(Frame::bulk(Bytes::from(name.to_string())))
            .chain(args.iter().map(|a| Frame::bulk(Bytes::from(a.to_string()))))
            .collect(),
    );
    Command::from_frame(frame).unwrap()
}

// ---------------------------------------------------------------------------
// Test 1: PING / ECHO
// ---------------------------------------------------------------------------

#[test]
fn test_ping_echo() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    let resp = router.execute(&cmd("PING", &[]));
    assert_eq!(resp, Frame::SimpleString("PONG".into()));

    let resp = router.execute(&cmd("PING", &["hello"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("hello")));

    let resp = router.execute(&cmd("ECHO", &["world"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("world")));
}

// ---------------------------------------------------------------------------
// Test 2: SET / GET / DEL / EXISTS
// ---------------------------------------------------------------------------

#[test]
fn test_set_get_del_exists() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // SET
    let resp = router.execute(&cmd("SET", &["mykey", "myvalue"]));
    assert_eq!(resp, Frame::ok());

    // GET
    let resp = router.execute(&cmd("GET", &["mykey"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("myvalue")));

    // EXISTS
    let resp = router.execute(&cmd("EXISTS", &["mykey"]));
    assert_eq!(resp, Frame::Integer(1));

    // DEL
    let resp = router.execute(&cmd("DEL", &["mykey"]));
    assert_eq!(resp, Frame::Integer(1));

    // GET after DEL
    let resp = router.execute(&cmd("GET", &["mykey"]));
    assert_eq!(resp, Frame::Null);

    // EXISTS after DEL
    let resp = router.execute(&cmd("EXISTS", &["mykey"]));
    assert_eq!(resp, Frame::Integer(0));
}

// ---------------------------------------------------------------------------
// Test 3: INCR / DECR / INCRBY / DECRBY
// ---------------------------------------------------------------------------

#[test]
fn test_incr_decr() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // INCR on non-existent key starts at 0
    let resp = router.execute(&cmd("INCR", &["counter"]));
    assert_eq!(resp, Frame::Integer(1));

    let resp = router.execute(&cmd("INCR", &["counter"]));
    assert_eq!(resp, Frame::Integer(2));

    let resp = router.execute(&cmd("DECR", &["counter"]));
    assert_eq!(resp, Frame::Integer(1));

    let resp = router.execute(&cmd("INCRBY", &["counter", "10"]));
    assert_eq!(resp, Frame::Integer(11));

    let resp = router.execute(&cmd("DECRBY", &["counter", "5"]));
    assert_eq!(resp, Frame::Integer(6));

    // INCR on non-integer should error
    router.execute(&cmd("SET", &["strkey", "notanumber"]));
    let resp = router.execute(&cmd("INCR", &["strkey"]));
    assert!(resp.is_error());
}

// ---------------------------------------------------------------------------
// Test 4: SET with EX / TTL / PERSIST / EXPIRE
// ---------------------------------------------------------------------------

#[test]
fn test_ttl_expire_persist() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // SET with EX
    router.execute(&cmd("SET", &["ttlkey", "val", "EX", "3600"]));

    let resp = router.execute(&cmd("TTL", &["ttlkey"]));
    if let Frame::Integer(ttl) = resp {
        assert!(ttl > 3590 && ttl <= 3600);
    } else {
        panic!("expected integer");
    }

    // PERSIST
    let resp = router.execute(&cmd("PERSIST", &["ttlkey"]));
    assert_eq!(resp, Frame::Integer(1));

    let resp = router.execute(&cmd("TTL", &["ttlkey"]));
    assert_eq!(resp, Frame::Integer(-1)); // no TTL

    // EXPIRE
    router.execute(&cmd("EXPIRE", &["ttlkey", "60"]));
    let resp = router.execute(&cmd("TTL", &["ttlkey"]));
    if let Frame::Integer(ttl) = resp {
        assert!(ttl > 50 && ttl <= 60);
    } else {
        panic!("expected integer");
    }

    // TTL on non-existent key
    let resp = router.execute(&cmd("TTL", &["nokey"]));
    assert_eq!(resp, Frame::Integer(-2));
}

// ---------------------------------------------------------------------------
// Test 5: SADD / SISMEMBER / SMEMBERS / SREM / SCARD
// ---------------------------------------------------------------------------

#[test]
fn test_set_operations() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // SADD
    let resp = router.execute(&cmd("SADD", &["myset", "a", "b", "c"]));
    assert_eq!(resp, Frame::Integer(3));

    // SADD duplicate
    let resp = router.execute(&cmd("SADD", &["myset", "a", "d"]));
    assert_eq!(resp, Frame::Integer(1)); // only "d" is new

    // SCARD
    let resp = router.execute(&cmd("SCARD", &["myset"]));
    assert_eq!(resp, Frame::Integer(4));

    // SISMEMBER
    let resp = router.execute(&cmd("SISMEMBER", &["myset", "a"]));
    assert_eq!(resp, Frame::Integer(1));

    let resp = router.execute(&cmd("SISMEMBER", &["myset", "z"]));
    assert_eq!(resp, Frame::Integer(0));

    // SMEMBERS
    let resp = router.execute(&cmd("SMEMBERS", &["myset"]));
    if let Frame::Array(members) = resp {
        assert_eq!(members.len(), 4);
    } else {
        panic!("expected array");
    }

    // SREM
    let resp = router.execute(&cmd("SREM", &["myset", "a", "b"]));
    assert_eq!(resp, Frame::Integer(2));

    let resp = router.execute(&cmd("SCARD", &["myset"]));
    assert_eq!(resp, Frame::Integer(2));
}

// ---------------------------------------------------------------------------
// Test 6: ZADD / ZSCORE / ZCARD / ZRANGE / ZRANGEBYSCORE / ZREM
// ---------------------------------------------------------------------------

#[test]
fn test_sorted_set_operations() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // ZADD
    let resp = router.execute(&cmd("ZADD", &["leaderboard", "100", "alice", "200", "bob", "150", "carol"]));
    assert_eq!(resp, Frame::Integer(3));

    // ZCARD
    let resp = router.execute(&cmd("ZCARD", &["leaderboard"]));
    assert_eq!(resp, Frame::Integer(3));

    // ZSCORE
    let resp = router.execute(&cmd("ZSCORE", &["leaderboard", "bob"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("200")));

    let resp = router.execute(&cmd("ZSCORE", &["leaderboard", "nobody"]));
    assert_eq!(resp, Frame::Null);

    // ZRANGE (ascending by score)
    let resp = router.execute(&cmd("ZRANGE", &["leaderboard", "0", "-1"]));
    if let Frame::Array(members) = resp {
        assert_eq!(members.len(), 3);
        // Ascending: alice(100), carol(150), bob(200)
        assert_eq!(members[0], Frame::BulkString(Bytes::from("alice")));
        assert_eq!(members[1], Frame::BulkString(Bytes::from("carol")));
        assert_eq!(members[2], Frame::BulkString(Bytes::from("bob")));
    } else {
        panic!("expected array");
    }

    // ZRANGE with WITHSCORES
    let resp = router.execute(&cmd("ZRANGE", &["leaderboard", "0", "0", "WITHSCORES"]));
    if let Frame::Array(items) = resp {
        assert_eq!(items.len(), 2); // member + score
        assert_eq!(items[0], Frame::BulkString(Bytes::from("alice")));
        assert_eq!(items[1], Frame::BulkString(Bytes::from("100")));
    } else {
        panic!("expected array");
    }

    // ZRANGEBYSCORE
    let resp = router.execute(&cmd("ZRANGEBYSCORE", &["leaderboard", "100", "150"]));
    if let Frame::Array(members) = resp {
        assert_eq!(members.len(), 2);
        assert_eq!(members[0], Frame::BulkString(Bytes::from("alice")));
        assert_eq!(members[1], Frame::BulkString(Bytes::from("carol")));
    } else {
        panic!("expected array");
    }

    // ZREM
    let resp = router.execute(&cmd("ZREM", &["leaderboard", "alice"]));
    assert_eq!(resp, Frame::Integer(1));

    let resp = router.execute(&cmd("ZCARD", &["leaderboard"]));
    assert_eq!(resp, Frame::Integer(2));
}

// ---------------------------------------------------------------------------
// Test 7: Bloom filter commands
// ---------------------------------------------------------------------------

#[test]
fn test_bloom_filter() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // BF.RESERVE
    let resp = router.execute(&cmd("BF.RESERVE", &["myfilter", "0.01", "10000"]));
    assert_eq!(resp, Frame::ok());

    // BF.ADD
    let resp = router.execute(&cmd("BF.ADD", &["myfilter", "item1"]));
    assert_eq!(resp, Frame::Integer(1)); // new

    // BF.ADD duplicate
    let resp = router.execute(&cmd("BF.ADD", &["myfilter", "item1"]));
    assert_eq!(resp, Frame::Integer(0)); // already exists

    // BF.EXISTS
    let resp = router.execute(&cmd("BF.EXISTS", &["myfilter", "item1"]));
    assert_eq!(resp, Frame::Integer(1));

    let resp = router.execute(&cmd("BF.EXISTS", &["myfilter", "item999"]));
    assert_eq!(resp, Frame::Integer(0));
}

// ---------------------------------------------------------------------------
// Test 8: Stream commands (XADD / XLEN / XREAD / XACK)
// ---------------------------------------------------------------------------

#[test]
fn test_stream_operations() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // XADD
    let resp = router.execute(&cmd("XADD", &["events", "1-0", "type", "order", "id", "123"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("1-0")));

    let resp = router.execute(&cmd("XADD", &["events", "2-0", "type", "payment", "id", "456"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("2-0")));

    // XLEN
    let resp = router.execute(&cmd("XLEN", &["events"]));
    assert_eq!(resp, Frame::Integer(2));

    // XRANGE
    let resp = router.execute(&cmd("XRANGE", &["events", "-", "+"]));
    if let Frame::Array(entries) = resp {
        assert_eq!(entries.len(), 2);
    } else {
        panic!("expected array");
    }

    // XGROUP CREATE
    let resp = router.execute(&cmd("XGROUP", &["CREATE", "events", "mygroup", "0"]));
    assert_eq!(resp, Frame::ok());

    // XREADGROUP
    let resp = router.execute(&cmd(
        "XREADGROUP",
        &["GROUP", "mygroup", "consumer1", "COUNT", "10", "STREAMS", "events", ">"],
    ));
    if let Frame::Array(entries) = &resp {
        assert_eq!(entries.len(), 2);
    } else {
        panic!("expected array, got {:?}", resp);
    }

    // XACK
    let resp = router.execute(&cmd("XACK", &["events", "mygroup", "1-0", "2-0"]));
    assert_eq!(resp, Frame::Integer(2));
}

// ---------------------------------------------------------------------------
// Test 9: MGET / MSET
// ---------------------------------------------------------------------------

#[test]
fn test_mget_mset() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // MSET
    let resp = router.execute(&cmd("MSET", &["k1", "v1", "k2", "v2", "k3", "v3"]));
    assert_eq!(resp, Frame::ok());

    // MGET
    let resp = router.execute(&cmd("MGET", &["k1", "k2", "missing", "k3"]));
    if let Frame::Array(values) = resp {
        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Frame::BulkString(Bytes::from("v1")));
        assert_eq!(values[1], Frame::BulkString(Bytes::from("v2")));
        assert_eq!(values[2], Frame::Null);
        assert_eq!(values[3], Frame::BulkString(Bytes::from("v3")));
    } else {
        panic!("expected array");
    }
}

// ---------------------------------------------------------------------------
// Test 10: DBSIZE / FLUSHDB / INFO
// ---------------------------------------------------------------------------

#[test]
fn test_dbsize_flushdb_info() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // Initially empty
    let resp = router.execute(&cmd("DBSIZE", &[]));
    assert_eq!(resp, Frame::Integer(0));

    // Add some keys
    router.execute(&cmd("SET", &["a", "1"]));
    router.execute(&cmd("SET", &["b", "2"]));
    router.execute(&cmd("SET", &["c", "3"]));

    let resp = router.execute(&cmd("DBSIZE", &[]));
    assert_eq!(resp, Frame::Integer(3));

    // INFO
    let resp = router.execute(&cmd("INFO", &[]));
    if let Frame::BulkString(info) = resp {
        let s = String::from_utf8(info.to_vec()).unwrap();
        assert!(s.contains("kaya_version"));
    } else {
        panic!("expected bulk string");
    }

    // FLUSHDB
    let resp = router.execute(&cmd("FLUSHDB", &[]));
    assert_eq!(resp, Frame::ok());

    let resp = router.execute(&cmd("DBSIZE", &[]));
    assert_eq!(resp, Frame::Integer(0));
}

// ---------------------------------------------------------------------------
// Test 11: MULTI / EXEC transaction
// ---------------------------------------------------------------------------

#[test]
fn test_multi_exec() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // Build a batch of commands as if MULTI/EXEC were used
    let commands = vec![
        cmd("SET", &["txkey1", "val1"]),
        cmd("SET", &["txkey2", "val2"]),
        cmd("INCR", &["txcounter"]),
    ];

    let resp = router.execute_multi(&commands);
    if let Frame::Array(results) = resp {
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Frame::ok());
        assert_eq!(results[1], Frame::ok());
        assert_eq!(results[2], Frame::Integer(1));
    } else {
        panic!("expected array");
    }

    // Verify the values persisted
    let resp = router.execute(&cmd("GET", &["txkey1"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("val1")));
}

// ---------------------------------------------------------------------------
// Test 12: TYPE command
// ---------------------------------------------------------------------------

#[test]
fn test_type_command() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    router.execute(&cmd("SET", &["strkey", "value"]));
    let resp = router.execute(&cmd("TYPE", &["strkey"]));
    assert_eq!(resp, Frame::SimpleString("string".into()));

    router.execute(&cmd("SADD", &["setkey", "a"]));
    let resp = router.execute(&cmd("TYPE", &["setkey"]));
    assert_eq!(resp, Frame::SimpleString("set".into()));

    router.execute(&cmd("ZADD", &["zkey", "1.0", "a"]));
    let resp = router.execute(&cmd("TYPE", &["zkey"]));
    assert_eq!(resp, Frame::SimpleString("zset".into()));

    let resp = router.execute(&cmd("TYPE", &["nokey"]));
    assert_eq!(resp, Frame::SimpleString("none".into()));
}

// ---------------------------------------------------------------------------
// Test 13: CONFIG command
// ---------------------------------------------------------------------------

#[test]
fn test_config_command() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    let resp = router.execute(&cmd("CONFIG", &["GET", "databases"]));
    if let Frame::Array(items) = resp {
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected array");
    }

    let resp = router.execute(&cmd("CONFIG", &["SET", "something", "value"]));
    assert_eq!(resp, Frame::ok());
}

// ---------------------------------------------------------------------------
// Test 14: XTRIM
// ---------------------------------------------------------------------------

#[test]
fn test_xtrim() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    for i in 1..=10 {
        router.execute(&cmd("XADD", &["trimstream", &format!("{}-0", i), "k", "v"]));
    }

    let resp = router.execute(&cmd("XLEN", &["trimstream"]));
    assert_eq!(resp, Frame::Integer(10));

    // Trim to 5
    let resp = router.execute(&cmd("XTRIM", &["trimstream", "MAXLEN", "5"]));
    assert_eq!(resp, Frame::Integer(5)); // 5 trimmed

    let resp = router.execute(&cmd("XLEN", &["trimstream"]));
    assert_eq!(resp, Frame::Integer(5));
}

// ---------------------------------------------------------------------------
// Test 15: Unknown command returns error
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_command() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    let resp = router.execute(&cmd("FOOBAR", &[]));
    assert!(resp.is_error());
}

// ---------------------------------------------------------------------------
// Test 16: XGROUP DELCONSUMER
// ---------------------------------------------------------------------------

#[test]
fn test_xgroup_delconsumer() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // Setup stream and group
    router.execute(&cmd("XADD", &["s", "1-0", "k", "v"]));
    router.execute(&cmd("XGROUP", &["CREATE", "s", "g", "0"]));

    // Read to create consumer with pending entries
    router.execute(&cmd(
        "XREADGROUP",
        &["GROUP", "g", "c1", "COUNT", "10", "STREAMS", "s", ">"],
    ));

    // DELCONSUMER
    let resp = router.execute(&cmd("XGROUP", &["DELCONSUMER", "s", "g", "c1"]));
    assert_eq!(resp, Frame::Integer(1)); // 1 pending entry removed
}

// ---------------------------------------------------------------------------
// Test 17: Write-Behind pattern (SET + SADD + XADD atomically)
// ---------------------------------------------------------------------------

#[test]
fn test_write_behind_pattern() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // Simulate Write-Behind: SET the value, SADD to pending set, XADD to stream
    let commands = vec![
        cmd("SET", &["user:123", "{\"name\":\"Alice\"}"]),
        cmd("SADD", &["pending:user", "user:123"]),
        cmd("XADD", &["writebehind:user", "*", "key", "user:123", "op", "SET"]),
    ];

    let resp = router.execute_multi(&commands);
    if let Frame::Array(results) = resp {
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Frame::ok());                      // SET ok
        assert_eq!(results[1], Frame::Integer(1));                // SADD added 1
        assert!(!results[2].is_error());                          // XADD returned an ID
    } else {
        panic!("expected array");
    }

    // Verify: user is in pending set
    let resp = router.execute(&cmd("SISMEMBER", &["pending:user", "user:123"]));
    assert_eq!(resp, Frame::Integer(1));
}

// ---------------------------------------------------------------------------
// Test 18: auth-ms session pattern (sorted sets for session limiting)
// ---------------------------------------------------------------------------

#[test]
fn test_session_limiting_pattern() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    // Add sessions with timestamps as scores
    let now = 1712500000u64;
    router.execute(&cmd("ZADD", &[
        "sessions:user42",
        &(now).to_string(), "sess-aaa",
        &(now + 10).to_string(), "sess-bbb",
        &(now + 20).to_string(), "sess-ccc",
    ]));

    // Count sessions
    let resp = router.execute(&cmd("ZCARD", &["sessions:user42"]));
    assert_eq!(resp, Frame::Integer(3));

    // Get sessions within time window
    let resp = router.execute(&cmd("ZRANGEBYSCORE", &[
        "sessions:user42",
        &(now).to_string(),
        &(now + 15).to_string(),
    ]));
    if let Frame::Array(members) = resp {
        assert_eq!(members.len(), 2); // sess-aaa and sess-bbb
    } else {
        panic!("expected array");
    }

    // Remove oldest session
    router.execute(&cmd("ZREM", &["sessions:user42", "sess-aaa"]));
    let resp = router.execute(&cmd("ZCARD", &["sessions:user42"]));
    assert_eq!(resp, Frame::Integer(2));
}

// ---------------------------------------------------------------------------
// Test 19: SELECT command (only db 0)
// ---------------------------------------------------------------------------

#[test]
fn test_select() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    let resp = router.execute(&cmd("SELECT", &["0"]));
    assert_eq!(resp, Frame::ok());

    let resp = router.execute(&cmd("SELECT", &["1"]));
    assert!(resp.is_error());
}

// ---------------------------------------------------------------------------
// Test 20: ZADD update score
// ---------------------------------------------------------------------------

#[test]
fn test_zadd_update_score() {
    let ctx = test_ctx();
    let router = CommandRouter::new(ctx);

    router.execute(&cmd("ZADD", &["z", "1.0", "member"]));
    let resp = router.execute(&cmd("ZSCORE", &["z", "member"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("1")));

    // Update score: should return 0 (not new)
    let resp = router.execute(&cmd("ZADD", &["z", "5.0", "member"]));
    assert_eq!(resp, Frame::Integer(0));

    // Check updated score
    let resp = router.execute(&cmd("ZSCORE", &["z", "member"]));
    assert_eq!(resp, Frame::BulkString(Bytes::from("5")));
}
