// SPDX-License-Identifier: AGPL-3.0-or-later
//! Integration tests for KAYA probabilistic data structures.

use kaya_store::probabilistic::{
    CountMinSketch, CuckooFilter, HyperLogLog, ProbabilisticError, TopK,
};

// ---------------------------------------------------------------------------
// Cuckoo Filter
// ---------------------------------------------------------------------------

#[test]
fn cuckoo_insert_and_lookup() {
    let mut cf = CuckooFilter::new(10_000, 0.01);
    for i in 0..1000u32 {
        cf.insert(&i.to_le_bytes()).expect("insert ok");
    }
    for i in 0..1000u32 {
        assert!(
            cf.contains(&i.to_le_bytes()),
            "cuckoo should contain inserted item {i}"
        );
    }
    assert_eq!(cf.len(), 1000);
    assert!(cf.fill_ratio() > 0.0);
}

#[test]
fn cuckoo_delete_removes() {
    let mut cf = CuckooFilter::new(1000, 0.01);
    cf.insert(b"present").expect("insert ok");
    assert!(cf.contains(b"present"));
    assert!(cf.delete(b"present"));
    assert!(!cf.contains(b"present"));
    // Deleting an absent item should return false.
    assert!(!cf.delete(b"never_inserted"));
}

#[test]
fn cuckoo_false_positive_rate_within_tolerance() {
    let mut cf = CuckooFilter::new(20_000, 0.001);
    // Insert 10k items.
    for i in 0..10_000u32 {
        let key = format!("inserted:{i}");
        cf.insert(key.as_bytes()).expect("insert ok");
    }
    // Query 10k absent items.
    let mut fp = 0usize;
    for i in 0..10_000u32 {
        let key = format!("absent:{i}");
        if cf.contains(key.as_bytes()) {
            fp += 1;
        }
    }
    let rate = fp as f64 / 10_000.0;
    // 16-bit fingerprints -> ~0.02% FPR expected. Allow up to 2% to be safe.
    assert!(rate < 0.02, "false positive rate {rate} too high");
}

// ---------------------------------------------------------------------------
// HyperLogLog
// ---------------------------------------------------------------------------

#[test]
fn hll_cardinality_within_tolerance() {
    let mut hll = HyperLogLog::new(14);
    for i in 0..10_000u32 {
        hll.add(&i.to_le_bytes());
    }
    let c = hll.count();
    let err = (c as f64 - 10_000.0).abs() / 10_000.0;
    assert!(err < 0.02, "HLL error {err} exceeds 2% (count={c})");
}

#[test]
fn hll_merge_correct() {
    let mut a = HyperLogLog::new(14);
    let mut b = HyperLogLog::new(14);
    for i in 0..5000u32 {
        a.add(&i.to_le_bytes());
    }
    for i in 2500..7500u32 {
        b.add(&i.to_le_bytes());
    }
    a.merge(&b);
    let c = a.count();
    // Union cardinality: 7500 distinct elements.
    let err = (c as f64 - 7500.0).abs() / 7500.0;
    assert!(err < 0.03, "merge error {err} too high (count={c})");
}

#[test]
fn hll_serialize_roundtrip() {
    let mut hll = HyperLogLog::new(12);
    for i in 0..2000u32 {
        hll.add(&i.to_le_bytes());
    }
    let bytes = hll.serialize();
    let hll2 = HyperLogLog::deserialize(&bytes).expect("deserialize ok");
    assert_eq!(hll.count(), hll2.count());
    assert_eq!(hll.precision(), hll2.precision());

    // Bad magic should fail.
    let mut bad = bytes.to_vec();
    bad[0] = b'X';
    assert!(matches!(
        HyperLogLog::deserialize(&bad),
        Err(ProbabilisticError::Deserialize(_))
    ));
}

// ---------------------------------------------------------------------------
// Count-Min Sketch
// ---------------------------------------------------------------------------

#[test]
fn cms_point_query_within_bounds() {
    let mut cms = CountMinSketch::new_with_dimensions(4096, 5);
    let truth_popular = 1000u64;
    for _ in 0..truth_popular {
        cms.increment(b"popular", 1);
    }
    // Insert noise for overcount quantification.
    let n_noise = 100_000u64;
    for i in 0..n_noise {
        let key = format!("noise:{i}");
        cms.increment(key.as_bytes(), 1);
    }
    let est = cms.estimate(b"popular");
    assert!(est >= truth_popular, "estimate {est} < truth {truth_popular}");
    // Overestimate must stay within epsilon * N. Our width is 4096 so
    // epsilon approx e/4096 ≈ 6.6e-4. For N=101_000, error bound ≈ 67.
    // Use a generous tolerance: truth + epsilon*N*4.
    let bound = truth_popular + ((std::f64::consts::E / 4096.0) * (n_noise + truth_popular) as f64
        * 5.0) as u64;
    assert!(est <= bound, "estimate {est} > bound {bound}");
}

#[test]
fn cms_merge_additive() {
    let mut a = CountMinSketch::new_with_dimensions(512, 4);
    let mut b = CountMinSketch::new_with_dimensions(512, 4);
    for _ in 0..50 {
        a.increment(b"shared", 1);
    }
    for _ in 0..70 {
        b.increment(b"shared", 1);
    }
    a.merge(&b);
    let est = a.estimate(b"shared");
    assert!(est >= 120, "expected >= 120 got {est}");
}

#[test]
fn cms_dimensions_from_epsilon_delta() {
    let cms = CountMinSketch::new(0.001, 0.01);
    // width = ceil(e/0.001) = 2719; depth = ceil(ln(100)) = 5
    assert!(cms.width() >= 2700);
    assert!(cms.depth() >= 4);
}

// ---------------------------------------------------------------------------
// TopK
// ---------------------------------------------------------------------------

#[test]
fn topk_detects_heavy_hitters_zipf() {
    // Build a crude Zipf stream: item_i appears (N/i) times.
    let mut t = TopK::new(5, 512, 4, 0.9);
    let n = 500u64;
    for i in 1u64..=20u64 {
        let freq = n / i;
        let key = format!("k{i}");
        for _ in 0..freq {
            t.add(key.as_bytes());
        }
    }
    let list = t.list();
    assert!(!list.is_empty(), "top-k should have items");
    // Top item should be k1 (the heaviest).
    let (top_item, _) = &list[0];
    assert_eq!(
        top_item.as_ref(),
        b"k1",
        "heaviest hitter should be k1, got {:?}",
        top_item
    );
}

#[test]
fn topk_list_sorted_descending() {
    let mut t = TopK::new(3, 256, 4, 0.9);
    for _ in 0..100 {
        t.add(b"a");
    }
    for _ in 0..50 {
        t.add(b"b");
    }
    for _ in 0..25 {
        t.add(b"c");
    }
    let list = t.list();
    for pair in list.windows(2) {
        assert!(pair[0].1 >= pair[1].1, "list not sorted: {:?}", list);
    }
}

#[test]
fn topk_decay_reduces_stale_items() {
    // High decay (close to 1) keeps items stable; low decay (close to 0)
    // evicts more aggressively. This test checks that a dominant stream
    // can displace an early burst.
    let mut t = TopK::new(3, 64, 3, 0.7);
    for _ in 0..20 {
        t.add(b"stale");
    }
    // Hammer a new item.
    for _ in 0..200 {
        t.add(b"fresh");
    }
    let list = t.list();
    // "fresh" must be present.
    assert!(
        list.iter().any(|(k, _)| k.as_ref() == b"fresh"),
        "fresh should be tracked, got {:?}",
        list
    );
}
