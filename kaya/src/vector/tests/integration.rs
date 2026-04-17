//! Integration tests for kaya-vector: exercises VectorStore and VectorIndex
//! end-to-end, covering the same scenarios as FT.* commands.

use std::collections::HashMap;

use kaya_vector::{DistanceMetric, IndexOpts, VectorStore, VectorIndex};
use kaya_vector::store::Filter;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn new_store() -> VectorStore {
    VectorStore::new()
}

fn opts(m: usize, ef: usize, cap: usize) -> IndexOpts {
    IndexOpts { m, ef_construction: ef, max_elements: cap }
}

// Encode &[f32] to little-endian bytes then back, simulating the wire protocol.
fn roundtrip_vec(v: &[f32]) -> Vec<f32> {
    let bytes: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
    bytes
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

// ---------------------------------------------------------------------------
// Test 1: FT.CREATE then FT.ADD 100 docs, confirm count
// ---------------------------------------------------------------------------

#[test]
fn create_and_add_100_docs() {
    let s = new_store();
    s.create_index("idx100", 8, DistanceMetric::Cosine, opts(16, 200, 200))
        .unwrap();

    for i in 0u64..100 {
        let v: Vec<f32> = (0..8).map(|j| (i as f32 * 0.1 + j as f32)).collect();
        // Simulate little-endian wire encoding
        let encoded = roundtrip_vec(&v);
        let mut attrs = HashMap::new();
        attrs.insert("doc_id".into(), i.to_string());
        s.add_doc("idx100", i, &encoded, attrs).unwrap();
    }

    let info = s.info("idx100").unwrap();
    assert_eq!(info.doc_count, 100);
}

// ---------------------------------------------------------------------------
// Test 2: FT.SEARCH KNN top-10, Cosine, results sorted ascending
// ---------------------------------------------------------------------------

#[test]
fn knn_top10_sorted_ascending_cosine() {
    let s = new_store();
    s.create_index("cosine", 4, DistanceMetric::Cosine, opts(16, 200, 1000))
        .unwrap();

    // Insert 50 docs; doc 0 = [1,0,0,0], doc 1 = [0,1,0,0], ...
    // Query [1,0,0,0] should put doc 0 first (distance=0).
    for i in 0u64..50 {
        let mut v = vec![0.0f32; 4];
        v[(i % 4) as usize] = 1.0 + i as f32 * 0.01;
        s.add_doc("cosine", i, &roundtrip_vec(&v), HashMap::new()).unwrap();
    }

    let query = roundtrip_vec(&[1.0f32, 0.0, 0.0, 0.0]);
    let results = s.search("cosine", &query, 10, 50, None).unwrap();

    assert_eq!(results.len(), 10);
    // Check ascending order by distance
    for w in results.windows(2) {
        assert!(
            w[0].1 <= w[1].1,
            "results not sorted ascending: {} > {}",
            w[0].1, w[1].1
        );
    }
    // First result should be one of the docs parallel to [1,0,0,0] (i%4==0)
    // HNSW is approximate so we check the top-3 contain at least one such doc.
    let parallel_ids: Vec<u64> = results.iter()
        .take(3)
        .filter(|(id, _, _)| id % 4 == 0)
        .map(|(id, _, _)| *id)
        .collect();
    assert!(
        !parallel_ids.is_empty(),
        "expected at least one doc with i%4==0 in top-3, got: {:?}",
        results.iter().take(3).map(|(id,_,_)| *id).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Test 3: L2 metric correctness
// ---------------------------------------------------------------------------

#[test]
fn l2_metric_nearest_is_origin() {
    let s = new_store();
    s.create_index("l2", 3, DistanceMetric::L2, opts(8, 100, 500))
        .unwrap();

    s.add_doc("l2", 1, &[0.0, 0.0, 0.0], HashMap::new()).unwrap();
    s.add_doc("l2", 2, &[10.0, 10.0, 10.0], HashMap::new()).unwrap();
    s.add_doc("l2", 3, &[0.1, 0.1, 0.1], HashMap::new()).unwrap();

    let q = [0.05f32, 0.05, 0.05];
    let results = s.search("l2", &q, 3, 50, None).unwrap();

    // Doc 1 (origin) is closest, doc 3 (0.1,0.1,0.1) is second
    assert!(results[0].0 == 1 || results[0].0 == 3, "unexpected nearest: {}", results[0].0);
    // In any case doc 2 (far away) should be last
    assert_eq!(results.last().unwrap().0, 2, "doc 2 should be farthest");
}

// ---------------------------------------------------------------------------
// Test 4: IP metric nearest has highest dot product
// ---------------------------------------------------------------------------

#[test]
fn ip_metric_highest_dot_first() {
    let s = new_store();
    s.create_index("ip", 2, DistanceMetric::IP, opts(8, 100, 100))
        .unwrap();

    // HnswIP uses cosine distance internally (always ≥ 0) but orderings
    // are the same as IP for L2-normalised vectors.
    // query  = [1/√2, 1/√2]  (unit vector at 45°)
    // doc 10: [1, 0]           angle=45°  cos_dist ≈ 0.29
    // doc 20: [0, 1]           angle=45°  cos_dist ≈ 0.29
    // doc 30: [1/√2, 1/√2]    angle=0°   cos_dist = 0   ← nearest
    let q2 = 1.0f32 / 2.0f32.sqrt();
    s.add_doc("ip", 10, &[1.0f32, 0.0], HashMap::new()).unwrap();
    s.add_doc("ip", 20, &[0.0f32, 1.0], HashMap::new()).unwrap();
    s.add_doc("ip", 30, &[q2, q2], HashMap::new()).unwrap();

    let query = [q2, q2];
    let results = s.search("ip", &query, 3, 50, None).unwrap();

    assert_eq!(results.len(), 3);
    // Doc 30 is identical to query → smallest distance → first result
    assert_eq!(results[0].0, 30, "doc 30 (same direction as query) should be nearest");
}

// ---------------------------------------------------------------------------
// Test 5: Filter returns NotImplemented
// ---------------------------------------------------------------------------

#[test]
fn filter_returns_not_implemented() {
    let s = new_store();
    s.create_index("fi", 4, DistanceMetric::L2, opts(8, 100, 100))
        .unwrap();
    s.add_doc("fi", 1, &[1.0f32, 0.0, 0.0, 0.0], HashMap::new()).unwrap();

    let filter = Some(Filter { field: "category".into(), value: "A".into() });
    let err = s.search("fi", &[1.0, 0.0, 0.0, 0.0], 1, 50, filter.as_ref()).unwrap_err();
    assert!(
        matches!(err, kaya_vector::VectorError::FilterNotImplemented),
        "expected FilterNotImplemented, got {:?}", err
    );
}

// ---------------------------------------------------------------------------
// Test 6: Dim mismatch on add → error
// ---------------------------------------------------------------------------

#[test]
fn dim_mismatch_on_add_returns_error() {
    let s = new_store();
    s.create_index("dm", 4, DistanceMetric::L2, opts(8, 100, 100))
        .unwrap();
    let err = s.add_doc("dm", 1, &[1.0f32, 0.0], HashMap::new()).unwrap_err();
    assert!(
        matches!(err, kaya_vector::VectorError::DimMismatch { expected: 4, got: 2 }),
        "unexpected error: {:?}", err
    );
}

// ---------------------------------------------------------------------------
// Test 7: Accuracy on 1000 random vectors (dim=32), recall@10 ≥ 0.90
// ---------------------------------------------------------------------------

#[test]
fn hnsw_accuracy_recall_at_10() {
    use rand::{Rng, SeedableRng};
    use kaya_vector::distance::cosine_distance;

    let mut rng = rand::rngs::StdRng::seed_from_u64(12345);
    let n = 1000usize;
    let dim = 32usize;
    let k = 10usize;

    let s = new_store();
    s.create_index("acc", dim, DistanceMetric::Cosine, opts(16, 200, n + 100))
        .unwrap();

    let mut vecs: Vec<Vec<f32>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut v: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-9 {
            v.iter_mut().for_each(|x| *x /= norm);
        }
        vecs.push(v.clone());
        s.add_doc("acc", i as u64, &v, HashMap::new()).unwrap();
    }

    let mut q: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();
    let qnorm: f32 = q.iter().map(|x| x * x).sum::<f32>().sqrt();
    if qnorm > 1e-9 {
        q.iter_mut().for_each(|x| *x /= qnorm);
    }

    // Brute-force ground truth
    let mut bf: Vec<(usize, f32)> = vecs
        .iter()
        .enumerate()
        .map(|(i, v)| (i, cosine_distance(&q, v)))
        .collect();
    bf.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let bf_top: std::collections::HashSet<u64> = bf.iter().take(k).map(|(i, _)| *i as u64).collect();

    // HNSW results
    let results = s.search("acc", &q, k, k * 10, None).unwrap();
    let hnsw_top: std::collections::HashSet<u64> = results.iter().map(|(id, _, _)| *id).collect();

    let overlap = hnsw_top.intersection(&bf_top).count();
    let recall = overlap as f64 / k as f64;
    assert!(
        recall >= 0.90,
        "recall@{k}={recall:.2} < 0.90 (overlap={overlap}/{k})"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Add/Del idempotency
// ---------------------------------------------------------------------------

#[test]
fn add_del_idempotency() {
    let s = new_store();
    s.create_index("idem", 2, DistanceMetric::L2, opts(8, 100, 100))
        .unwrap();

    // Add twice → idempotent
    s.add_doc("idem", 7, &[1.0f32, 0.0], HashMap::new()).unwrap();
    s.add_doc("idem", 7, &[1.0f32, 0.0], HashMap::new()).unwrap(); // should not error
    let info = s.info("idem").unwrap();
    assert_eq!(info.doc_count, 1);

    // Delete once
    assert!(s.del_doc("idem", 7).unwrap());
    // Delete again → false (already tombstoned)
    assert!(!s.del_doc("idem", 7).unwrap());
    let info2 = s.info("idem").unwrap();
    assert_eq!(info2.doc_count, 0);
}

// ---------------------------------------------------------------------------
// Test 9: FT.INFO returns correct dim/count/metric
// ---------------------------------------------------------------------------

#[test]
fn info_returns_correct_fields() {
    let s = new_store();
    s.create_index(
        "meta",
        768,
        DistanceMetric::Cosine,
        opts(16, 200, 50_000),
    )
    .unwrap();
    s.add_doc("meta", 1, &vec![0.01f32; 768], HashMap::new()).unwrap();

    let info = s.info("meta").unwrap();
    assert_eq!(info.name, "meta");
    assert_eq!(info.dim, 768);
    assert_eq!(info.metric, "COSINE");
    assert_eq!(info.doc_count, 1);
    assert_eq!(info.m, 16);
    assert_eq!(info.ef_construction, 200);
}

// ---------------------------------------------------------------------------
// Test 10: Range search with L2
// ---------------------------------------------------------------------------

#[test]
fn range_search_l2() {
    let idx = VectorIndex::new(2, DistanceMetric::L2, IndexOpts::default()).unwrap();
    idx.add(1, &[0.0, 0.0], HashMap::new()).unwrap();
    idx.add(2, &[3.0, 4.0], HashMap::new()).unwrap(); // L2² = 25
    idx.add(3, &[0.5, 0.0], HashMap::new()).unwrap(); // L2² = 0.25

    let q = [0.0f32, 0.0];
    let within = idx.range_search(&q, 1.0); // radius ≤ 1.0 (squared dist)
    let ids: Vec<u64> = within.iter().map(|(id, _)| *id).collect();

    assert!(ids.contains(&1), "origin should be within radius");
    assert!(ids.contains(&3), "doc 3 (dist²=0.25) should be within radius");
    assert!(!ids.contains(&2), "doc 2 (dist²=25) should be outside radius");
}

// ---------------------------------------------------------------------------
// Test 11: Alias lifecycle
// ---------------------------------------------------------------------------

#[test]
fn alias_full_lifecycle() {
    let s = new_store();
    s.create_index("main", 4, DistanceMetric::Cosine, opts(8, 100, 100))
        .unwrap();

    // Add alias
    s.alias_add("alt", "main").unwrap();
    // Access via alias
    s.add_doc("alt", 1, &[1.0f32, 0.0, 0.0, 0.0], HashMap::new()).unwrap();
    let info = s.info("alt").unwrap();
    assert_eq!(info.doc_count, 1);

    // Update alias to another index
    s.create_index("other", 4, DistanceMetric::Cosine, opts(8, 100, 100))
        .unwrap();
    s.alias_update("alt", "other").unwrap();
    let info2 = s.info("alt").unwrap();
    assert_eq!(info2.doc_count, 0); // points to fresh "other" index

    // Delete alias
    assert!(s.alias_del("alt"));
    assert!(s.info("alt").is_err());
}

// ---------------------------------------------------------------------------
// Test 12: NotFound on missing index
// ---------------------------------------------------------------------------

#[test]
fn missing_index_returns_not_found() {
    let s = new_store();
    let err = s.info("nonexistent").unwrap_err();
    assert!(
        matches!(err, kaya_vector::VectorError::IndexNotFound(_)),
        "unexpected error: {:?}", err
    );
    let err2 = s.add_doc("nonexistent", 1, &[1.0f32], HashMap::new()).unwrap_err();
    assert!(matches!(err2, kaya_vector::VectorError::IndexNotFound(_)));
}
