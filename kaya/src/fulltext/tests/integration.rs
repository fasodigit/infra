//! Integration tests for kaya-fulltext covering all 10+ required cases.

use std::collections::HashMap;
use std::sync::Arc;

use kaya_fulltext::{FieldDef, FieldType, FieldValue, FtSchema, FtStore, SearchHit};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_store() -> Arc<FtStore> {
    Arc::new(FtStore::new())
}

fn text_schema() -> FtSchema {
    let mut s = FtSchema::new();
    s.add_field(FieldDef {
        name: "title".into(),
        ty: FieldType::Text { tokenized: true, analyzer: None, boost: 1.0 },
        stored: true,
    });
    s
}

fn full_schema() -> FtSchema {
    let mut s = FtSchema::new();
    s.add_field(FieldDef {
        name: "title".into(),
        ty: FieldType::Text { tokenized: true, analyzer: None, boost: 1.0 },
        stored: true,
    });
    s.add_field(FieldDef {
        name: "price".into(),
        ty: FieldType::Numeric { indexed: true, sortable: true },
        stored: true,
    });
    s.add_field(FieldDef {
        name: "category".into(),
        ty: FieldType::Tag { separator: ',', case_sensitive: false },
        stored: true,
    });
    s
}

fn add(store: &FtStore, idx: &[u8], id: &str, fields: HashMap<String, FieldValue>) {
    store.add_doc(idx, id.as_bytes(), fields).unwrap();
}

// ---------------------------------------------------------------------------
// Test 1: FT.CREATE with TEXT + NUMERIC + TAG schema
// ---------------------------------------------------------------------------

#[test]
fn create_full_schema() {
    let store = make_store();
    store.create(b"idx_t1", full_schema()).unwrap();
    let info = store.info(b"idx_t1").unwrap();
    assert_eq!(info.num_fields, 3);
    assert_eq!(info.num_docs, 0);
}

// ---------------------------------------------------------------------------
// Test 2: Add 100 docs then search returns top-10 with BM25 scores
// ---------------------------------------------------------------------------

#[test]
fn add_100_docs_search_top_10() {
    let store = make_store();
    store.create(b"idx_t2", text_schema()).unwrap();

    for i in 0..100u32 {
        let mut fields = HashMap::new();
        let body = if i % 5 == 0 {
            "kaya sovereign in-memory database".to_owned()
        } else {
            format!("generic document {i}")
        };
        fields.insert("title".into(), FieldValue::Text(body));
        add(&store, b"idx_t2", &format!("doc{i}"), fields);
    }

    let hits = store.search(b"idx_t2", "@title:kaya", 10, None).unwrap();
    assert!(!hits.is_empty(), "should find hits for 'kaya'");
    assert!(hits.len() <= 10, "at most 10 hits returned");
    // All hits should have a BM25 score > 0.
    for hit in &hits {
        assert!(hit.score >= 0.0, "score should be non-negative");
    }
}

// ---------------------------------------------------------------------------
// Test 3: FT.SEARCH with exact phrase query
// ---------------------------------------------------------------------------

#[test]
fn search_phrase_query() {
    let store = make_store();
    store.create(b"idx_t3", text_schema()).unwrap();

    let mut f1 = HashMap::new();
    f1.insert("title".into(), FieldValue::Text("hello world from kaya".into()));
    add(&store, b"idx_t3", "p1", f1);

    let mut f2 = HashMap::new();
    f2.insert("title".into(), FieldValue::Text("hello kaya database".into()));
    add(&store, b"idx_t3", "p2", f2);

    let hits = store
        .search(b"idx_t3", r#"@title:"hello world""#, 10, None)
        .unwrap();
    assert_eq!(hits.len(), 1, "only the phrase match should be returned");
    assert_eq!(hits[0].doc_id, "p1");
}

// ---------------------------------------------------------------------------
// Test 4: FT.SEARCH with numeric range
// ---------------------------------------------------------------------------

#[test]
fn search_numeric_range() {
    let mut s = FtSchema::new();
    s.add_field(FieldDef {
        name: "price".into(),
        ty: FieldType::Numeric { indexed: true, sortable: true },
        stored: true,
    });
    let store = make_store();
    store.create(b"idx_t4", s).unwrap();

    for (id, price) in [("n1", 5.0_f64), ("n2", 50.0), ("n3", 200.0)] {
        let mut fields = HashMap::new();
        fields.insert("price".into(), FieldValue::Numeric(price));
        add(&store, b"idx_t4", id, fields);
    }

    let hits = store
        .search(b"idx_t4", "@price:[20 100]", 10, None)
        .unwrap();
    assert_eq!(hits.len(), 1, "only price=50 is in [20,100]");
    assert_eq!(hits[0].doc_id, "n2");
}

// ---------------------------------------------------------------------------
// Test 5: FT.SEARCH with tag filter
// ---------------------------------------------------------------------------

#[test]
fn search_tag_filter() {
    let mut s = FtSchema::new();
    s.add_field(FieldDef {
        name: "lang".into(),
        ty: FieldType::Tag { separator: ',', case_sensitive: false },
        stored: true,
    });
    let store = make_store();
    store.create(b"idx_t5", s).unwrap();

    for (id, lang) in [("t1", "rust"), ("t2", "go"), ("t3", "rust")] {
        let mut fields = HashMap::new();
        fields.insert("lang".into(), FieldValue::Tag(lang.into()));
        add(&store, b"idx_t5", id, fields);
    }

    let hits = store
        .search(b"idx_t5", "@lang:{rust}", 10, None)
        .unwrap();
    assert_eq!(hits.len(), 2, "two docs have lang=rust");
}

// ---------------------------------------------------------------------------
// Test 6: FT.DEL then FT.SEARCH returns N-1 results
// ---------------------------------------------------------------------------

#[test]
fn del_doc_reduces_results() {
    let store = make_store();
    store.create(b"idx_t6", text_schema()).unwrap();

    let mut f1 = HashMap::new();
    f1.insert("title".into(), FieldValue::Text("kaya is fast".into()));
    add(&store, b"idx_t6", "d1", f1);

    let mut f2 = HashMap::new();
    f2.insert("title".into(), FieldValue::Text("kaya is sovereign".into()));
    add(&store, b"idx_t6", "d2", f2);

    let before = store.search(b"idx_t6", "@title:kaya", 10, None).unwrap();
    assert_eq!(before.len(), 2);

    store.del_doc(b"idx_t6", b"d1").unwrap();

    let after = store.search(b"idx_t6", "@title:kaya", 10, None).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].doc_id, "d2");
}

// ---------------------------------------------------------------------------
// Test 7: FT.AGGREGATE GROUPBY field returns correct counts
// ---------------------------------------------------------------------------

#[test]
fn aggregate_groupby() {
    let mut s = FtSchema::new();
    s.add_field(FieldDef {
        name: "lang".into(),
        ty: FieldType::Tag { separator: ',', case_sensitive: false },
        stored: true,
    });
    let store = make_store();
    store.create(b"idx_t7", s).unwrap();

    for (id, lang) in [
        ("a1", "rust"),
        ("a2", "go"),
        ("a3", "rust"),
        ("a4", "rust"),
        ("a5", "go"),
    ] {
        let mut fields = HashMap::new();
        fields.insert("lang".into(), FieldValue::Tag(lang.into()));
        add(&store, b"idx_t7", id, fields);
    }

    let counts = store.aggregate(b"idx_t7", "lang").unwrap();
    assert_eq!(counts.get("rust").copied().unwrap_or(0), 3);
    assert_eq!(counts.get("go").copied().unwrap_or(0), 2);
}

// ---------------------------------------------------------------------------
// Test 8: FT.EXPLAIN returns non-empty string
// ---------------------------------------------------------------------------

#[test]
fn explain_returns_nonempty() {
    use kaya_fulltext::query::translate_to_tantivy;
    let translated = translate_to_tantivy("@title:kaya");
    assert!(!translated.is_empty());
    assert!(translated.contains("kaya"));
}

// ---------------------------------------------------------------------------
// Test 9: FT.INFO counts docs correctly
// ---------------------------------------------------------------------------

#[test]
fn info_counts_docs() {
    let store = make_store();
    store.create(b"idx_t9", text_schema()).unwrap();

    for i in 0..7u32 {
        let mut fields = HashMap::new();
        fields.insert("title".into(), FieldValue::Text(format!("doc {i}")));
        add(&store, b"idx_t9", &format!("i{i}"), fields);
    }

    let info = store.info(b"idx_t9").unwrap();
    assert_eq!(info.num_docs, 7);
}

// ---------------------------------------------------------------------------
// Test 10: FT.ALIAS round-trip (add → search via alias → update → delete)
// ---------------------------------------------------------------------------

#[test]
fn alias_round_trip() {
    let store = make_store();
    store.create(b"real", text_schema()).unwrap();
    store.create(b"other", text_schema()).unwrap();

    // ALIASADD
    store.alias_add(b"myalias", b"real").unwrap();

    // Add doc via alias.
    let mut fields = HashMap::new();
    fields.insert("title".into(), FieldValue::Text("aliased document".into()));
    store.add_doc(b"myalias", b"ax1", fields).unwrap();

    // Search via alias.
    let hits = store.search(b"myalias", "@title:aliased", 10, None).unwrap();
    assert_eq!(hits.len(), 1, "alias search should find the document");

    // ALIASUPDATE
    store.alias_update(b"myalias", b"other").unwrap();

    // ALIASDEL
    assert!(store.alias_del(b"myalias"));
    assert!(!store.alias_del(b"myalias"), "second delete should return false");
}

// ---------------------------------------------------------------------------
// Test 11: drop index also removes aliases pointing to it
// ---------------------------------------------------------------------------

#[test]
fn drop_removes_aliases() {
    let store = make_store();
    store.create(b"todrop", text_schema()).unwrap();
    store.alias_add(b"alias_for_drop", b"todrop").unwrap();

    store.drop_index(b"todrop");
    // Searching via the alias now returns an error (index not found).
    let result = store.search(b"alias_for_drop", "*", 10, None);
    assert!(result.is_err(), "alias target was dropped, should error");
}

// ---------------------------------------------------------------------------
// Test 12: OR / AND boolean operators at query level
// ---------------------------------------------------------------------------

#[test]
fn boolean_or_query() {
    let store = make_store();
    store.create(b"idx_bool", text_schema()).unwrap();

    let mut f1 = HashMap::new();
    f1.insert("title".into(), FieldValue::Text("rust programming".into()));
    add(&store, b"idx_bool", "b1", f1);

    let mut f2 = HashMap::new();
    f2.insert("title".into(), FieldValue::Text("go programming".into()));
    add(&store, b"idx_bool", "b2", f2);

    let mut f3 = HashMap::new();
    f3.insert("title".into(), FieldValue::Text("java enterprise".into()));
    add(&store, b"idx_bool", "b3", f3);

    let hits = store
        .search(b"idx_bool", "@title:rust OR @title:go", 10, None)
        .unwrap();
    assert_eq!(hits.len(), 2, "rust OR go should match 2 docs");
}
