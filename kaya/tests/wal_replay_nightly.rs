// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Nightly WAL replay test: deterministic workload → snapshot → crash → recover → diff.
// Run via `cargo test --release --test wal_replay_nightly -- --nocapture`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use kaya_compress::CompressConfig;
use kaya_store::persistence::{CompressionAlgo, FsyncPolicy, PersistenceConfig, PersistenceManager};
use kaya_store::{Store, StoreConfig};

fn config_for(dir: &Path, fsync: FsyncPolicy) -> PersistenceConfig {
    let mut c = PersistenceConfig::default();
    c.enabled = true;
    c.data_dir = dir.to_path_buf();
    c.fsync = fsync;
    c.compression = CompressionAlgo::Zstd;
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wal_replay_round_trip() {
    let tmp = tempfile::tempdir().expect("tmpdir");
    let fsync = std::env::var("KAYA_FSYNC_POLICY")
        .ok()
        .as_deref()
        .map(|s| match s {
            "always" => FsyncPolicy::Always,
            "no" => FsyncPolicy::No,
            _ => FsyncPolicy::Everysec,
        })
        .unwrap_or(FsyncPolicy::Everysec);

    let cfg = config_for(tmp.path(), fsync);

    // ------------------------------------------------------------------- phase 1
    // Workload and ground truth.
    let store = Arc::new(Store::new(StoreConfig::default(), CompressConfig::default()));
    let manager = Arc::new(PersistenceManager::new(cfg.clone(), store.clone()).await
        .expect("manager init"));

    let mut ground_truth: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();

    // 10_000 SETs
    for i in 0..10_000u32 {
        let k = format!("k:{i}").into_bytes();
        let v = format!("v:{i}:{}", "x".repeat(64)).into_bytes();
        store.set(&k, &v, None).unwrap();
        ground_truth.insert(k, v);
    }

    // Force snapshot at 5_000 logical ops mark (after all sets).
    let seq = manager.current_seq().await;
    let _ = store.save_snapshot(tmp.path(), CompressionAlgo::Zstd, 3, seq).await
        .expect("snapshot");

    // Continue with 1_000 DELs and 700 SADDs.
    for i in 0..1_000u32 {
        let k = format!("k:{i}").into_bytes();
        store.del(&[k.as_slice()]);
        ground_truth.remove(&k);
    }

    // Drop manager → simulate crash (flush everysec but not manual sync).
    drop(manager);
    drop(store);

    // ------------------------------------------------------------------- phase 2
    // Recover on a fresh store.
    let recovered = Arc::new(Store::new(StoreConfig::default(), CompressConfig::default()));
    let manager2 = Arc::new(PersistenceManager::new(cfg.clone(), recovered.clone()).await
        .expect("manager2"));
    let reports = recovered.recover_with(&manager2).await.expect("recover");
    eprintln!("recover reports: {reports:?}");

    // ------------------------------------------------------------------- phase 3
    // Diff.
    let mut diff_count = 0usize;
    for (k, expected) in &ground_truth {
        match recovered.get(k).unwrap() {
            Some(got) if got.as_ref() == expected.as_slice() => {}
            _ => {
                diff_count += 1;
                if diff_count <= 5 {
                    eprintln!("DIFF: key={:?}", String::from_utf8_lossy(k));
                }
            }
        }
    }

    println!(
        "{}",
        if diff_count == 0 { "PASS".to_string() } else { format!("FAIL {diff_count}") }
    );
    assert_eq!(diff_count, 0, "WAL replay divergence: {diff_count} keys differ");
}
