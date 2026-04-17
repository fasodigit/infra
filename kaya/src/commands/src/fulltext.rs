// SPDX-License-Identifier: AGPL-3.0-or-later
//! RESP3 command handlers for KAYA Full-Text Search (FT.*).
//!
//! This module exposes a single `dispatch_fulltext_command` entry-point that
//! routes FT.* commands to the appropriate handler.  The wire-up in the
//! router calls this function *after* any vector-search check, so that
//! FT.CREATE / FT.SEARCH can coexist with V3.2 vector HNSW without conflict.
//!
//! # Supported commands
//!
//! - `FT.CREATE`  index SCHEMA …
//! - `FT.DROP`    index [DD]
//! - `FT.ADD`     index doc_id [FIELDS] field value …
//! - `FT.DEL`     index doc_id [DD]
//! - `FT.SEARCH`  index query [LIMIT offset num] [RETURN n f …] [SORTBY f ASC|DESC]
//! - `FT.AGGREGATE` index query [GROUPBY n f … REDUCE COUNT 0]
//! - `FT.EXPLAIN` index query
//! - `FT.INFO`    index
//! - `FT.ALTER`   index SCHEMA ADD field type [options]
//! - `FT.ALIASADD`    alias index
//! - `FT.ALIASDEL`    alias
//! - `FT.ALIASUPDATE` alias index

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use kaya_protocol::{Command, Frame};

use kaya_fulltext::{FieldDef, FieldType, FieldValue, FtSchema, FtStore, SortBy};

use crate::CommandError;

// ---------------------------------------------------------------------------
// Public entry-point
// ---------------------------------------------------------------------------

/// Attempt to dispatch a command to the fulltext handlers.
///
/// Returns `Some(frame)` if the command name matched an FT.* command, or
/// `None` if the command is not a fulltext command (so the caller can try
/// other handlers).
pub fn dispatch_fulltext_command(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Option<Frame> {
    let frame = match cmd.name.as_str() {
        "FT.CREATE" => handle_ft_create(store, cmd),
        "FT.DROP" => handle_ft_drop(store, cmd),
        "FT.ADD" => handle_ft_add(store, cmd),
        "FT.DEL" => handle_ft_del(store, cmd),
        "FT.SEARCH" => handle_ft_search(store, cmd),
        "FT.AGGREGATE" => handle_ft_aggregate(store, cmd),
        "FT.EXPLAIN" => handle_ft_explain(store, cmd),
        "FT.INFO" => handle_ft_info(store, cmd),
        "FT.ALTER" => handle_ft_alter(store, cmd),
        "FT.ALIASADD" => handle_ft_aliasadd(store, cmd),
        "FT.ALIASDEL" => handle_ft_aliasdel(store, cmd),
        "FT.ALIASUPDATE" => handle_ft_aliasupdate(store, cmd),
        _ => return None,
    };
    Some(match frame {
        Ok(f) => f,
        Err(e) => Frame::Error(format!("ERR {e}")),
    })
}

// ---------------------------------------------------------------------------
// FT.CREATE
// ---------------------------------------------------------------------------

/// FT.CREATE index SCHEMA field1 TEXT [NOSTEM] [SORTABLE]
///                        field2 NUMERIC [SORTABLE]
///                        field3 TAG [SEPARATOR ","]
fn handle_ft_create(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    // args: index SCHEMA field1 type [options] …
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?.clone();

    // Find "SCHEMA" keyword.
    let schema_pos = (1..cmd.arg_count())
        .find(|&i| cmd.arg_str(i).map(|s| s.eq_ignore_ascii_case("SCHEMA")).unwrap_or(false))
        .ok_or_else(|| CommandError::Syntax("FT.CREATE: missing SCHEMA keyword".into()))?;

    let mut schema = FtSchema::new();
    let mut i = schema_pos + 1;

    while i < cmd.arg_count() {
        let field_name = cmd.arg_str(i)?.to_owned();
        i += 1;

        if i >= cmd.arg_count() {
            return Err(CommandError::Syntax(format!(
                "FT.CREATE: missing type for field '{field_name}'"
            )));
        }

        let type_str = cmd.arg_str(i)?.to_ascii_uppercase();
        i += 1;

        let mut stored = true;
        let fdef = match type_str.as_str() {
            "TEXT" => {
                let mut tokenized = true;
                let mut boost: f32 = 1.0;
                // consume optional TEXT modifiers
                while i < cmd.arg_count() {
                    let tok = cmd.arg_str(i)?.to_ascii_uppercase();
                    match tok.as_str() {
                        "NOSTEM" => { tokenized = false; i += 1; }
                        "WEIGHT" => {
                            i += 1;
                            boost = cmd.arg_str(i)
                                .ok()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1.0);
                            i += 1;
                        }
                        "SORTABLE" => { i += 1; }
                        "NOINDEX" => { stored = false; i += 1; }
                        _ => break,
                    }
                }
                FieldDef {
                    name: field_name,
                    ty: FieldType::Text { tokenized, analyzer: None, boost },
                    stored,
                }
            }
            "NUMERIC" => {
                let mut sortable = false;
                while i < cmd.arg_count() {
                    let tok = cmd.arg_str(i)?.to_ascii_uppercase();
                    match tok.as_str() {
                        "SORTABLE" => { sortable = true; i += 1; }
                        "NOINDEX" => { stored = false; i += 1; }
                        _ => break,
                    }
                }
                FieldDef {
                    name: field_name,
                    ty: FieldType::Numeric { indexed: true, sortable },
                    stored,
                }
            }
            "TAG" => {
                let mut separator = ',';
                let mut case_sensitive = false;
                while i < cmd.arg_count() {
                    let tok = cmd.arg_str(i)?.to_ascii_uppercase();
                    match tok.as_str() {
                        "SEPARATOR" => {
                            i += 1;
                            separator = cmd.arg_str(i)
                                .ok()
                                .and_then(|s| s.chars().next())
                                .unwrap_or(',');
                            i += 1;
                        }
                        "CASESENSITIVE" => { case_sensitive = true; i += 1; }
                        _ => break,
                    }
                }
                FieldDef {
                    name: field_name,
                    ty: FieldType::Tag { separator, case_sensitive },
                    stored,
                }
            }
            "GEO" => {
                FieldDef { name: field_name, ty: FieldType::Geo, stored }
            }
            other => {
                return Err(CommandError::Syntax(format!(
                    "FT.CREATE: unknown field type '{other}'"
                )));
            }
        };

        schema.add_field(fdef);
    }

    store
        .create(index_name.as_ref(), schema)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// FT.DROP
// ---------------------------------------------------------------------------

fn handle_ft_drop(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?;
    let removed = store.drop_index(index_name.as_ref());
    if removed {
        Ok(Frame::ok())
    } else {
        Ok(Frame::Error("ERR Unknown index name".into()))
    }
}

// ---------------------------------------------------------------------------
// FT.ADD
// ---------------------------------------------------------------------------

/// FT.ADD index doc_id [FIELDS] field1 value1 field2 value2 …
fn handle_ft_add(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    // Minimum: index doc_id field value
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?.clone();
    let doc_id = cmd.arg_bytes(1)?.clone();

    // Skip optional "FIELDS" keyword.
    let fields_start = if cmd
        .arg_str(2)
        .map(|s| s.eq_ignore_ascii_case("FIELDS"))
        .unwrap_or(false)
    {
        3
    } else {
        2
    };

    let remaining = cmd.arg_count() - fields_start;
    if remaining == 0 || remaining % 2 != 0 {
        return Err(CommandError::Syntax(
            "FT.ADD: field/value pairs must be even".into(),
        ));
    }

    let mut fields: HashMap<String, FieldValue> = HashMap::new();
    let mut i = fields_start;
    while i + 1 < cmd.arg_count() {
        let fname = cmd.arg_str(i)?.to_owned();
        let fval_str = cmd.arg_str(i + 1)?.to_owned();

        // Heuristic: try numeric, else treat as Text/Tag.
        let fval = if let Ok(n) = fval_str.parse::<f64>() {
            FieldValue::Numeric(n)
        } else {
            FieldValue::Text(fval_str)
        };

        fields.insert(fname, fval);
        i += 2;
    }

    store
        .add_doc(index_name.as_ref(), doc_id.as_ref(), fields)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// FT.DEL
// ---------------------------------------------------------------------------

fn handle_ft_del(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?;
    let doc_id = cmd.arg_bytes(1)?;

    let n = store
        .del_doc(index_name.as_ref(), doc_id.as_ref())
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(Frame::Integer(n as i64))
}

// ---------------------------------------------------------------------------
// FT.SEARCH
// ---------------------------------------------------------------------------

/// FT.SEARCH index query [LIMIT offset num] [RETURN n field …] [SORTBY field ASC|DESC]
fn handle_ft_search(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?;
    let query = cmd.arg_str(1)?.to_owned();

    // Defaults.
    let mut offset: usize = 0;
    let mut num: usize = 10;
    let mut return_fields: Option<Vec<String>> = None;
    let mut sort_by: Option<SortBy> = None;

    let mut i = 2;
    while i < cmd.arg_count() {
        let tok = cmd.arg_str(i)?.to_ascii_uppercase();
        match tok.as_str() {
            "LIMIT" => {
                offset = cmd.arg_str(i + 1)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                num = cmd.arg_str(i + 2)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10);
                i += 3;
            }
            "RETURN" => {
                let n: usize = cmd.arg_str(i + 1)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let mut names = Vec::with_capacity(n);
                for j in 0..n {
                    if let Ok(f) = cmd.arg_str(i + 2 + j) {
                        names.push(f.to_owned());
                    }
                }
                return_fields = Some(names);
                i += 2 + n;
            }
            "SORTBY" => {
                let field = cmd.arg_str(i + 1).map(|s| s.to_owned()).unwrap_or_default();
                let ascending = cmd.arg_str(i + 2)
                    .map(|s| s.eq_ignore_ascii_case("ASC"))
                    .unwrap_or(true);
                sort_by = Some(SortBy { field, ascending });
                i += 3;
            }
            _ => { i += 1; }
        }
    }

    let limit = offset + num;
    let hits = store
        .search(
            index_name.as_ref(),
            &query,
            limit,
            sort_by.as_ref(),
        )
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    // Apply offset.
    let page: Vec<_> = hits.into_iter().skip(offset).take(num).collect();

    // Build RESP3 response: [total_count, doc_id, {fields…}, …]
    let total = page.len() as i64;
    let mut resp = vec![Frame::Integer(total)];

    for hit in page {
        resp.push(Frame::BulkString(Bytes::from(hit.doc_id)));

        let fields_to_show: Box<dyn Iterator<Item = (String, FieldValue)>> =
            match &return_fields {
                Some(names) => Box::new(
                    hit.fields
                        .into_iter()
                        .filter(|(k, _)| names.contains(k)),
                ),
                None => Box::new(hit.fields.into_iter()),
            };

        let mut field_array: Vec<Frame> = Vec::new();
        for (k, v) in fields_to_show {
            field_array.push(Frame::BulkString(Bytes::from(k)));
            field_array.push(Frame::BulkString(Bytes::from(v.as_display())));
        }
        resp.push(Frame::Array(field_array));
    }

    Ok(Frame::Array(resp))
}

// ---------------------------------------------------------------------------
// FT.AGGREGATE
// ---------------------------------------------------------------------------

/// FT.AGGREGATE index query [GROUPBY n @field … REDUCE COUNT 0]
fn handle_ft_aggregate(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?;
    let _query = cmd.arg_str(1)?; // currently unused in MVP aggregate

    // Find the GROUPBY field.
    let group_field = (2..cmd.arg_count())
        .find(|&i| {
            cmd.arg_str(i)
                .map(|s| s.eq_ignore_ascii_case("GROUPBY"))
                .unwrap_or(false)
        })
        .and_then(|gi| {
            // GROUPBY n @field — skip count arg, take first field.
            let n: usize = cmd.arg_str(gi + 1)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            if n >= 1 {
                cmd.arg_str(gi + 2).ok().map(|f| {
                    // Strip leading '@' if present.
                    if f.starts_with('@') { f[1..].to_owned() } else { f.to_owned() }
                })
            } else {
                None
            }
        })
        .ok_or_else(|| {
            CommandError::Syntax("FT.AGGREGATE: missing GROUPBY clause".into())
        })?;

    let counts = store
        .aggregate(index_name.as_ref(), &group_field)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    // Return as array of [group_value, count] pairs.
    let mut rows: Vec<Frame> = Vec::with_capacity(counts.len());
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by_key(|(k, _)| k.clone());

    for (group_val, count) in sorted {
        rows.push(Frame::Array(vec![
            Frame::BulkString(Bytes::from(group_val)),
            Frame::Integer(count as i64),
        ]));
    }

    Ok(Frame::Array(rows))
}

// ---------------------------------------------------------------------------
// FT.EXPLAIN
// ---------------------------------------------------------------------------

fn handle_ft_explain(
    _store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_str(0)?;
    let query = cmd.arg_str(1)?;

    // Translate to Tantivy query string and return that as the AST explanation.
    let translated = kaya_fulltext::query::translate_to_tantivy(query);
    let explanation = format!(
        "index={index_name}\ntranslated_query={translated}\nengine=tantivy-0.22"
    );

    Ok(Frame::BulkString(Bytes::from(explanation)))
}

// ---------------------------------------------------------------------------
// FT.INFO
// ---------------------------------------------------------------------------

fn handle_ft_info(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?;
    let info = store
        .info(index_name.as_ref())
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    let resp = Frame::Array(vec![
        Frame::BulkString(Bytes::from_static(b"index_name")),
        Frame::BulkString(Bytes::from(info.name)),
        Frame::BulkString(Bytes::from_static(b"num_docs")),
        Frame::Integer(info.num_docs as i64),
        Frame::BulkString(Bytes::from_static(b"num_fields")),
        Frame::Integer(info.num_fields as i64),
    ]);

    Ok(resp)
}

// ---------------------------------------------------------------------------
// FT.ALTER
// ---------------------------------------------------------------------------

/// FT.ALTER index SCHEMA ADD field type [options]
fn handle_ft_alter(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 5 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }

    let index_name = cmd.arg_bytes(0)?;
    // Validate keyword sequence: SCHEMA ADD
    let kw1 = cmd.arg_str(1)?.to_ascii_uppercase();
    let kw2 = cmd.arg_str(2)?.to_ascii_uppercase();
    if kw1 != "SCHEMA" || kw2 != "ADD" {
        return Err(CommandError::Syntax(
            "FT.ALTER: expected SCHEMA ADD".into(),
        ));
    }

    let field_name = cmd.arg_str(3)?.to_owned();
    let type_str = cmd.arg_str(4)?.to_ascii_uppercase();

    let field_def = match type_str.as_str() {
        "TEXT" => FieldDef {
            name: field_name,
            ty: FieldType::Text { tokenized: true, analyzer: None, boost: 1.0 },
            stored: true,
        },
        "NUMERIC" => FieldDef {
            name: field_name,
            ty: FieldType::Numeric { indexed: true, sortable: false },
            stored: true,
        },
        "TAG" => FieldDef {
            name: field_name,
            ty: FieldType::Tag { separator: ',', case_sensitive: false },
            stored: true,
        },
        other => {
            return Err(CommandError::Syntax(format!(
                "FT.ALTER: unknown field type '{other}'"
            )));
        }
    };

    store
        .alter_add_field(index_name.as_ref(), field_def)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;

    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// FT.ALIASADD / FT.ALIASDEL / FT.ALIASUPDATE
// ---------------------------------------------------------------------------

fn handle_ft_aliasadd(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let alias = cmd.arg_bytes(0)?;
    let target = cmd.arg_bytes(1)?;
    store
        .alias_add(alias.as_ref(), target.as_ref())
        .map_err(|e| CommandError::Syntax(e.to_string()))?;
    Ok(Frame::ok())
}

fn handle_ft_aliasdel(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let alias = cmd.arg_bytes(0)?;
    if store.alias_del(alias.as_ref()) {
        Ok(Frame::ok())
    } else {
        Ok(Frame::Error("ERR Alias not found".into()))
    }
}

fn handle_ft_aliasupdate(
    store: &Arc<FtStore>,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let alias = cmd.arg_bytes(0)?;
    let new_target = cmd.arg_bytes(1)?;
    store
        .alias_update(alias.as_ref(), new_target.as_ref())
        .map_err(|e| CommandError::Syntax(e.to_string()))?;
    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_protocol::Frame;

    fn make_store() -> Arc<FtStore> {
        Arc::new(FtStore::new())
    }

    fn cmd(args: &[&str]) -> Command {
        Command {
            name: args[0].to_ascii_uppercase(),
            args: args[1..]
                .iter()
                .map(|s| Bytes::from(s.to_string()))
                .collect(),
        }
    }

    fn dispatch(store: &Arc<FtStore>, args: &[&str]) -> Frame {
        let c = cmd(args);
        dispatch_fulltext_command(store, &c).expect("command should be FT.*")
    }

    // -----------------------------------------------------------------------
    // 1. FT.CREATE with TEXT + NUMERIC + TAG schema
    // -----------------------------------------------------------------------

    #[test]
    fn ft_create_schema_text_numeric_tag() {
        let store = make_store();
        let f = dispatch(&store, &[
            "FT.CREATE", "test_idx", "SCHEMA",
            "title", "TEXT",
            "price", "NUMERIC", "SORTABLE",
            "category", "TAG", "SEPARATOR", ",",
        ]);
        assert!(!f.is_error(), "FT.CREATE should succeed: {f:?}");
    }

    // -----------------------------------------------------------------------
    // 2. FT.ADD 100 docs then FT.SEARCH returns top-10 with BM25 scores
    // -----------------------------------------------------------------------

    #[test]
    fn ft_add_100_docs_search_top10() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "bulk_idx", "SCHEMA", "body", "TEXT"]);

        for i in 0..100u32 {
            let doc_id = format!("doc{i}");
            let body = if i % 5 == 0 {
                "kaya sovereign database".to_owned()
            } else {
                format!("document number {i}")
            };
            dispatch(&store, &["FT.ADD", "bulk_idx", &doc_id, "FIELDS", "body", &body]);
        }

        let result = dispatch(&store, &["FT.SEARCH", "bulk_idx", "@body:kaya", "LIMIT", "0", "10"]);
        match result {
            Frame::Array(arr) => {
                // First element is the count; rest are doc_id + field pairs.
                let count = arr[0].as_integer().unwrap_or(0);
                assert!(count > 0, "should have results for 'kaya'");
                // At most 10 results returned.
                let doc_entries = (arr.len() - 1) / 2;
                assert!(doc_entries <= 10);
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 3. FT.SEARCH with phrase query
    // -----------------------------------------------------------------------

    #[test]
    fn ft_search_phrase_query() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "phrase_idx", "SCHEMA", "title", "TEXT"]);
        dispatch(&store, &["FT.ADD", "phrase_idx", "p1", "FIELDS", "title", "hello world"]);
        dispatch(&store, &["FT.ADD", "phrase_idx", "p2", "FIELDS", "title", "hello kaya"]);

        let result = dispatch(&store, &["FT.SEARCH", "phrase_idx", r#"@title:"hello world""#]);
        match result {
            Frame::Array(arr) => {
                let count = arr[0].as_integer().unwrap_or(0);
                assert_eq!(count, 1, "only 'hello world' should match the phrase");
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 4. FT.SEARCH with numeric range
    // -----------------------------------------------------------------------

    #[test]
    fn ft_search_numeric_range() {
        let store = make_store();
        dispatch(&store, &[
            "FT.CREATE", "num_idx", "SCHEMA", "price", "NUMERIC", "SORTABLE",
        ]);
        for (id, price) in [("n1", "10"), ("n2", "50"), ("n3", "200")] {
            dispatch(&store, &["FT.ADD", "num_idx", id, "FIELDS", "price", price]);
        }

        let result = dispatch(&store, &["FT.SEARCH", "num_idx", "@price:[20 100]"]);
        match result {
            Frame::Array(arr) => {
                let count = arr[0].as_integer().unwrap_or(0);
                assert_eq!(count, 1, "only price=50 is in [20,100]");
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 5. FT.SEARCH with tag filter
    // -----------------------------------------------------------------------

    #[test]
    fn ft_search_tag_filter() {
        let store = make_store();
        dispatch(&store, &[
            "FT.CREATE", "tag_idx", "SCHEMA", "lang", "TAG",
        ]);
        for (id, lang) in [("t1", "rust"), ("t2", "go"), ("t3", "rust")] {
            dispatch(&store, &["FT.ADD", "tag_idx", id, "FIELDS", "lang", lang]);
        }

        let result = dispatch(&store, &["FT.SEARCH", "tag_idx", "@lang:{rust}"]);
        match result {
            Frame::Array(arr) => {
                let count = arr[0].as_integer().unwrap_or(0);
                assert_eq!(count, 2, "two documents with lang=rust");
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 6. FT.DEL then FT.SEARCH returns N-1
    // -----------------------------------------------------------------------

    #[test]
    fn ft_del_reduces_search_results() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "del_idx", "SCHEMA", "body", "TEXT"]);
        dispatch(&store, &["FT.ADD", "del_idx", "d1", "FIELDS", "body", "kaya is great"]);
        dispatch(&store, &["FT.ADD", "del_idx", "d2", "FIELDS", "body", "kaya is fast"]);

        let before = dispatch(&store, &["FT.SEARCH", "del_idx", "@body:kaya"]);
        let before_count = match before {
            Frame::Array(ref arr) => arr[0].as_integer().unwrap_or(0),
            _ => panic!("expected Array"),
        };
        assert_eq!(before_count, 2);

        dispatch(&store, &["FT.DEL", "del_idx", "d1"]);
        let after = dispatch(&store, &["FT.SEARCH", "del_idx", "@body:kaya"]);
        let after_count = match after {
            Frame::Array(ref arr) => arr[0].as_integer().unwrap_or(0),
            _ => panic!("expected Array"),
        };
        assert_eq!(after_count, 1, "after deletion only one doc should match");
    }

    // -----------------------------------------------------------------------
    // 7. FT.AGGREGATE GROUPBY field
    // -----------------------------------------------------------------------

    #[test]
    fn ft_aggregate_groupby() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "agg_idx2", "SCHEMA", "lang", "TAG"]);
        for (id, lang) in [("a1", "rust"), ("a2", "go"), ("a3", "rust"), ("a4", "go"), ("a5", "rust")] {
            dispatch(&store, &["FT.ADD", "agg_idx2", id, "FIELDS", "lang", lang]);
        }

        let result = dispatch(&store, &[
            "FT.AGGREGATE", "agg_idx2", "*",
            "GROUPBY", "1", "@lang",
            "REDUCE", "COUNT", "0",
        ]);

        match result {
            Frame::Array(rows) => {
                // rows: [[lang, count], …]
                assert!(!rows.is_empty(), "aggregation should produce rows");
                let mut counts: HashMap<String, i64> = HashMap::new();
                for row in &rows {
                    if let Frame::Array(pair) = row {
                        if pair.len() == 2 {
                            let k = pair[0].as_str().unwrap_or("").to_owned();
                            let v = pair[1].as_integer().unwrap_or(0);
                            counts.insert(k, v);
                        }
                    }
                }
                assert_eq!(counts.get("rust").copied().unwrap_or(0), 3);
                assert_eq!(counts.get("go").copied().unwrap_or(0), 2);
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 8. FT.EXPLAIN returns non-empty AST string
    // -----------------------------------------------------------------------

    #[test]
    fn ft_explain_nonempty() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "exp_idx", "SCHEMA", "title", "TEXT"]);
        let result = dispatch(&store, &["FT.EXPLAIN", "exp_idx", "@title:kaya"]);
        match result {
            Frame::BulkString(b) => {
                let s = std::str::from_utf8(&b).unwrap();
                assert!(!s.is_empty(), "explain should return non-empty AST");
                assert!(s.contains("tantivy"), "should mention engine");
            }
            other => panic!("expected BulkString, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 9. FT.INFO counts docs correctly
    // -----------------------------------------------------------------------

    #[test]
    fn ft_info_counts_docs() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "info_idx", "SCHEMA", "body", "TEXT"]);

        for i in 0..5u32 {
            dispatch(&store, &["FT.ADD", "info_idx", &format!("i{i}"), "FIELDS", "body", "text"]);
        }

        let result = dispatch(&store, &["FT.INFO", "info_idx"]);
        match result {
            Frame::Array(arr) => {
                // arr: [key, val, key, val, …]
                let mut map: HashMap<String, Frame> = HashMap::new();
                let mut it = arr.into_iter();
                while let (Some(k), Some(v)) = (it.next(), it.next()) {
                    if let Some(key) = k.as_str() {
                        map.insert(key.to_owned(), v);
                    }
                }
                let num_docs = map
                    .get("num_docs")
                    .and_then(|f| f.as_integer())
                    .unwrap_or(-1);
                assert_eq!(num_docs, 5, "should report 5 docs");
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 10. FT.ALIAS round-trip (ALIASADD → ALIASUPDATE → ALIASDEL)
    // -----------------------------------------------------------------------

    #[test]
    fn ft_alias_round_trip() {
        let store = make_store();
        dispatch(&store, &["FT.CREATE", "real_idx2", "SCHEMA", "body", "TEXT"]);
        dispatch(&store, &["FT.CREATE", "other_idx", "SCHEMA", "body", "TEXT"]);

        // Add alias.
        let add = dispatch(&store, &["FT.ALIASADD", "myalias", "real_idx2"]);
        assert!(!add.is_error(), "ALIASADD should succeed");

        // Search via alias.
        dispatch(&store, &["FT.ADD", "myalias", "ax1", "FIELDS", "body", "aliased"]);
        let hits = dispatch(&store, &["FT.SEARCH", "myalias", "@body:aliased"]);
        match hits {
            Frame::Array(ref arr) => {
                assert_eq!(arr[0].as_integer().unwrap_or(0), 1, "alias search works");
            }
            _ => panic!("expected Array"),
        }

        // Update alias.
        let upd = dispatch(&store, &["FT.ALIASUPDATE", "myalias", "other_idx"]);
        assert!(!upd.is_error(), "ALIASUPDATE should succeed");

        // Delete alias.
        let del = dispatch(&store, &["FT.ALIASDEL", "myalias"]);
        assert!(!del.is_error(), "ALIASDEL should succeed");

        // Second delete should return error.
        let del2 = dispatch(&store, &["FT.ALIASDEL", "myalias"]);
        assert!(del2.is_error(), "second ALIASDEL should fail");
    }
}
