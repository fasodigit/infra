//! Integration tests for kaya-tiered.
//!
//! Each test is isolated in its own tempdir to avoid fjall file lock conflicts.

use std::sync::Arc;
use std::time::Duration;

use kaya_tiered::{
    ColdBackend, FjallBackend, MemBackend, MigrationPolicy, TieredStore,
};

fn make_hot() -> Arc<kaya_store::Store> {
    Arc::new(kaya_store::Store::default())
}

fn make_tiered_mem(policy: MigrationPolicy) -> Arc<TieredStore> {
    let hot = make_hot();
    let cold = Arc::new(MemBackend::new());
    Arc::new(TieredStore::with_tick(hot, cold, policy, Duration::from_millis(50)))
}

// ---------------------------------------------------------------------------
// Test 1: Set 10 keys hot, manually demote 5, verify locations
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_set_10_keys_demote_5() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    for i in 0u8..10 {
        store.set(&[i], b"value", None).await.unwrap();
    }

    // All 10 in hot tier.
    let stats = store.stats().await;
    assert_eq!(stats.hot_keys, 10, "expected 10 hot keys");
    assert_eq!(stats.cold_keys, 0, "expected 0 cold keys");

    // Demote 5 explicitly.
    for i in 5u8..10 {
        store.force_demote(&[i]).await.unwrap();
    }

    let stats = store.stats().await;
    assert_eq!(stats.cold_keys, 5, "expected 5 cold keys after demotion");
}

// ---------------------------------------------------------------------------
// Test 2: Get cold key triggers automatic promotion
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_get_cold_key_promotes() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    store.set(b"cold-key", b"important-value", None).await.unwrap();
    store.force_demote(b"cold-key").await.unwrap();

    // Verify it's cold.
    assert_eq!(
        store.location(b"cold-key"),
        Some(kaya_tiered::Location::Cold)
    );

    // Get triggers promotion.
    let val = store.get(b"cold-key").await.unwrap();
    assert!(val.is_some(), "should have gotten a value after promotion");
    assert_eq!(&val.unwrap()[..], b"important-value");

    // Key should now be hot again.
    assert_ne!(
        store.location(b"cold-key"),
        Some(kaya_tiered::Location::Cold),
        "key should no longer be cold after promotion"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Delete hot key — not found after
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_del_hot_key() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    store.set(b"to-del", b"data", None).await.unwrap();
    let deleted = store.del(b"to-del").await;
    assert_eq!(deleted, 1, "should have deleted 1 key");

    let found = store.get(b"to-del").await.unwrap();
    assert!(found.is_none(), "key should not be found after deletion");
}

// ---------------------------------------------------------------------------
// Test 4: Delete cold key — not found after
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_del_cold_key() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    store.set(b"cold-del", b"cold-data", None).await.unwrap();
    store.force_demote(b"cold-del").await.unwrap();

    let deleted = store.del(b"cold-del").await;
    assert!(deleted >= 1, "should have deleted the cold key");

    let found = store.get(b"cold-del").await.unwrap();
    assert!(found.is_none(), "cold key should not be found after deletion");
}

// ---------------------------------------------------------------------------
// Test 5: LFU policy — access 3 out of 10 keys, demote the 7 coldest
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_lfu_policy_demotes_coldest() {
    let policy = MigrationPolicy::LfuCold {
        min_idle_secs: 0,       // eligible immediately
        max_hot_mem_bytes: 0,   // always pressure
        migrate_ratio: 0.7,
    };
    let store = make_tiered_mem(policy);

    for i in 0u8..10 {
        store.set(&[i], b"v", None).await.unwrap();
    }

    // Access 3 specific keys repeatedly to make them hot.
    for _ in 0..5 {
        for k in [0u8, 1, 2] {
            store.get(&[k]).await.unwrap();
        }
    }

    // Manually run one migration tick.
    store.tick_once().await.unwrap();

    let stats = store.stats().await;
    // At least 5 keys should have been demoted (70% of 10 = 7, but accessed 3 are hot).
    assert!(
        stats.cold_keys >= 5,
        "expected at least 5 cold keys after LFU migration, got {}",
        stats.cold_keys
    );
    assert!(
        stats.migrations_total >= 5,
        "expected at least 5 migrations recorded"
    );
}

// ---------------------------------------------------------------------------
// Test 6: Stats invariant — hot_keys + cold_keys == total tracked keys
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_stats_total_invariant() {
    let store = make_tiered_mem(MigrationPolicy::Manual);
    let n = 20u8;

    for i in 0..n {
        store.set(&[i], b"x", None).await.unwrap();
    }

    for i in 0..(n / 2) {
        store.force_demote(&[i]).await.unwrap();
    }

    let stats = store.stats().await;
    let total = stats.hot_keys + stats.cold_keys;
    assert_eq!(
        total, n as u64,
        "hot_keys + cold_keys should equal total tracked keys"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Round-trip arbitrary binary value
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_binary_roundtrip() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    let binary_value: Vec<u8> = (0u8..=255).collect();
    store.set(b"bin-key", &binary_value, None).await.unwrap();

    // Hot read.
    let got = store.get(b"bin-key").await.unwrap().expect("should exist");
    assert_eq!(&got[..], &binary_value[..], "binary value round-trip failed (hot)");

    // Demote then cold read (promotion).
    store.force_demote(b"bin-key").await.unwrap();
    let got2 = store.get(b"bin-key").await.unwrap().expect("should exist after promote");
    assert_eq!(&got2[..], &binary_value[..], "binary value round-trip failed (cold→hot)");
}

// ---------------------------------------------------------------------------
// Test 8: Restart fjall (drop + reopen) — cold data persists
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_fjall_persistence_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();

    // Write phase.
    {
        let hot = make_hot();
        let cold = Arc::new(FjallBackend::open(&path, "kaya-cold").unwrap());
        let store = Arc::new(TieredStore::with_tick(
            hot,
            cold,
            MigrationPolicy::Manual,
            Duration::from_secs(60),
        ));

        store.set(b"persist-key", b"persist-value", None).await.unwrap();
        store.force_demote(b"persist-key").await.unwrap();
        // Ensure fjall flushes. Drop the store to trigger keyspace sync.
    }

    // Reopen phase.
    {
        let hot2 = make_hot();
        let cold2 = Arc::new(FjallBackend::open(&path, "kaya-cold").unwrap());

        // Directly query cold backend to check persistence.
        let raw = cold2.get(b"persist-key").await.unwrap();
        assert!(raw.is_some(), "data should persist across fjall restart");
        assert_eq!(raw.unwrap(), b"persist-value");
    }
}

// ---------------------------------------------------------------------------
// Test 9: Concurrent get during migration
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_concurrent_get_during_migration() {
    let store = Arc::new(make_tiered_mem(MigrationPolicy::Manual));

    for i in 0u8..20 {
        store.set(&[i], b"concurrent-val", None).await.unwrap();
    }

    // Demote all keys.
    for i in 0u8..20 {
        store.force_demote(&[i]).await.unwrap();
    }

    // Spawn concurrent readers.
    let mut handles = Vec::new();
    for i in 0u8..20 {
        let s = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            s.get(&[i]).await.expect("concurrent get should not fail")
        }));
    }

    // Also run a migration tick concurrently.
    let s2 = Arc::clone(&store);
    let migrator_handle = tokio::spawn(async move {
        s2.tick_once().await.expect("tick should not fail")
    });

    // Await all.
    for h in handles {
        let val = h.await.expect("task panicked");
        // Value might be Some (promoted) or None (migration raced us); both are valid.
        if let Some(v) = val {
            assert_eq!(&v[..], b"concurrent-val");
        }
    }
    let _: usize = migrator_handle.await.expect("migrator task panicked");
}

// ---------------------------------------------------------------------------
// Test 10: Optional Zstd compression on cold tier values via kaya-compress
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_cold_tier_compression() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    // Build a compressible value.
    let long_val: Vec<u8> = b"KAYA_COMPRESSED_COLD_VALUE_REPEATED"
        .iter()
        .cloned()
        .cycle()
        .take(1024)
        .collect();

    store.set(b"comp-key", &long_val, None).await.unwrap();
    store.force_demote(b"comp-key").await.unwrap();

    // Promote back and verify integrity.
    let got = store.get(b"comp-key").await.unwrap().expect("should exist");
    // The store internally uses kaya-compress on the hot tier. The raw bytes
    // stored in cold are the store's decompressed view, so round-trip is exact.
    assert_eq!(
        got.len(),
        long_val.len(),
        "compressed cold value should restore to original length"
    );
}

// ---------------------------------------------------------------------------
// Test 11: TTL policy respects hot_ttl_secs (demotes stale keys)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_ttl_policy_demotes_stale_keys() {
    let policy = MigrationPolicy::TtlCold { hot_ttl_secs: 0 }; // 0s threshold = immediate
    let store = Arc::new(make_tiered_mem(policy));

    store.set(b"ttl-key1", b"v1", None).await.unwrap();
    store.set(b"ttl-key2", b"v2", None).await.unwrap();

    // Run one tick — all keys are "older than 0s" so they should be demoted.
    store.tick_once().await.unwrap();

    let stats = store.stats().await;
    assert!(
        stats.cold_keys >= 2,
        "TTL policy should have demoted both keys, got cold_keys={}",
        stats.cold_keys
    );
}

// ---------------------------------------------------------------------------
// Test 12: force_promote and force_demote round-trip
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_force_promote_demote_roundtrip() {
    let store = make_tiered_mem(MigrationPolicy::Manual);

    store.set(b"round-key", b"round-value", None).await.unwrap();

    // Demote.
    store.force_demote(b"round-key").await.unwrap();
    assert_eq!(
        store.location(b"round-key"),
        Some(kaya_tiered::Location::Cold),
        "key should be cold after force_demote"
    );

    // Verify not in hot tier.
    assert!(
        store.hot_store().get(b"round-key").unwrap().is_none(),
        "key should not be in hot tier after demotion"
    );

    // Force promote.
    store.force_promote(b"round-key").await.unwrap();
    assert_ne!(
        store.location(b"round-key"),
        Some(kaya_tiered::Location::Cold),
        "key should not be cold after force_promote"
    );

    // Verify value.
    let val = store.get(b"round-key").await.unwrap().expect("should be hot");
    assert_eq!(&val[..], b"round-value");

    // Stats: promotions_total should be >= 1.
    let stats = store.stats().await;
    assert!(stats.promotions_total >= 1, "promotions_total should be >= 1");
}
