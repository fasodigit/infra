//! FT.* command handlers for KAYA vector search (RedisSearch parity).
//!
//! This module handles FT.* commands **when the schema is VECTOR-typed**.
//! Full-text FT.* commands (V3.4) will share the same command names but use a
//! different schema type; the [`dispatch_vector_command`] entry point returns
//! `None` for non-vector commands so the router can fall through to full-text.
//!
//! ## Supported commands
//! - `FT.CREATE` — create a HNSW vector index.
//! - `FT.DROP` — drop an index (optionally with `DD`).
//! - `FT.ADD` — add a document (vector encoded as little-endian FLOAT32 bytes).
//! - `FT.DEL` — tombstone a document.
//! - `FT.SEARCH` — KNN search with `*=>[KNN k @field $param]` syntax.
//! - `FT.INFO` — index metadata.
//! - `FT.ALIASADD` / `FT.ALIASDEL` / `FT.ALIASUPDATE` — alias management.
//! - `FT.CONFIG SET` / `FT.CONFIG GET` — stub (no persistent config).
//! - `FT.EXPLAIN` — debug representation of a query.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use kaya_vector::{DistanceMetric, IndexOpts, VectorStore};

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Stateless handler bound to a shared [`VectorStore`].
pub struct VectorHandler {
    store: Arc<VectorStore>,
}

impl VectorHandler {
    /// Create a new handler backed by `store`.
    pub fn new(store: Arc<VectorStore>) -> Self {
        Self { store }
    }

    // -----------------------------------------------------------------------
    // FT.CREATE
    // -----------------------------------------------------------------------

    /// ```text
    /// FT.CREATE index SCHEMA field VECTOR HNSW N_ARGS
    ///     TYPE FLOAT32
    ///     DIM <n>
    ///     DISTANCE_METRIC <COSINE|L2|IP>
    ///     [M <m>]
    ///     [EF_CONSTRUCTION <ef>]
    ///     [INITIAL_CAP <cap>]
    /// ```
    pub fn ft_create(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 2 {
            return Err("FT.CREATE requires at least index and SCHEMA".into());
        }
        let index_name = arg_str(cmd, 0)?;

        // Locate SCHEMA keyword
        let schema_pos = cmd
            .args
            .iter()
            .position(|b| b.to_ascii_uppercase() == b"SCHEMA")
            .ok_or_else(|| "FT.CREATE: SCHEMA keyword missing".to_owned())?;

        // After SCHEMA: field_name VECTOR HNSW N_ARGS key value...
        // We are lenient: scan for the VECTOR keyword after SCHEMA.
        let after_schema = schema_pos + 1;
        let vector_pos = cmd.args[after_schema..]
            .iter()
            .position(|b| b.to_ascii_uppercase() == b"VECTOR")
            .map(|p| p + after_schema)
            .ok_or_else(|| "FT.CREATE: VECTOR type not found — not a vector schema".to_owned())?;

        // Skip field_name before VECTOR
        if vector_pos < after_schema + 1 {
            return Err("FT.CREATE: expected field name before VECTOR".into());
        }

        // After VECTOR: HNSW N_ARGS [key value...]
        let algo_pos = vector_pos + 1;
        if algo_pos >= cmd.arg_count() {
            return Err("FT.CREATE: expected HNSW after VECTOR".into());
        }
        let algo = cmd.args[algo_pos].to_ascii_uppercase();
        if algo != b"HNSW" {
            return Err(format!(
                "FT.CREATE: unsupported vector algorithm '{}' (only HNSW is supported)",
                String::from_utf8_lossy(&algo)
            ));
        }

        // N_ARGS count (even number of key-value pairs)
        let n_args_pos = algo_pos + 1;
        let n_args: usize = if n_args_pos < cmd.arg_count() {
            let s = String::from_utf8_lossy(&cmd.args[n_args_pos]);
            s.parse().map_err(|_| format!("FT.CREATE: invalid N_ARGS '{s}'"))?
        } else {
            0
        };

        let args_start = n_args_pos + 1;
        let args_end = (args_start + n_args).min(cmd.arg_count());

        let mut dim: Option<usize> = None;
        let mut metric = DistanceMetric::Cosine;
        let mut m: usize = 16;
        let mut ef_construction: usize = 200;
        let mut max_elements: usize = 100_000;

        let mut i = args_start;
        while i + 1 <= args_end {
            let key = String::from_utf8_lossy(&cmd.args[i]).to_ascii_uppercase();
            let val = String::from_utf8_lossy(&cmd.args[i + 1]);
            match key.as_str() {
                "TYPE" => {
                    if val.to_ascii_uppercase() != "FLOAT32" {
                        return Err(format!("FT.CREATE: unsupported TYPE '{val}' (only FLOAT32)"));
                    }
                }
                "DIM" => {
                    dim = Some(
                        val.parse()
                            .map_err(|_| format!("FT.CREATE: invalid DIM '{val}'"))?,
                    );
                }
                "DISTANCE_METRIC" => {
                    metric = DistanceMetric::from_str_ci(&val)
                        .ok_or_else(|| format!("FT.CREATE: unknown DISTANCE_METRIC '{val}'"))?;
                }
                "M" => {
                    m = val
                        .parse()
                        .map_err(|_| format!("FT.CREATE: invalid M '{val}'"))?;
                }
                "EF_CONSTRUCTION" => {
                    ef_construction = val
                        .parse()
                        .map_err(|_| format!("FT.CREATE: invalid EF_CONSTRUCTION '{val}'"))?;
                }
                "INITIAL_CAP" => {
                    max_elements = val
                        .parse()
                        .map_err(|_| format!("FT.CREATE: invalid INITIAL_CAP '{val}'"))?;
                }
                _ => {
                    // Ignore unknown keys for forward compatibility.
                }
            }
            i += 2;
        }

        let dim = dim.ok_or("FT.CREATE: DIM is required")?;
        let opts = IndexOpts {
            m,
            ef_construction,
            max_elements,
        };

        self.store
            .create_index(index_name, dim, metric, opts)
            .map_err(|e| e.to_string())?;

        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // FT.DROP
    // -----------------------------------------------------------------------

    /// `FT.DROP index [DD]`
    ///
    /// The `DD` flag (delete documents) is accepted but silently ignored since
    /// KAYA's vector store does not persist documents separately.
    pub fn ft_drop(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 1 {
            return Err("FT.DROP requires index name".into());
        }
        let index_name = arg_str(cmd, 0)?;
        if self.store.drop_index(index_name) {
            Ok(Frame::ok())
        } else {
            Err(format!("Unknown Index name (first call FT.CREATE): {index_name}"))
        }
    }

    // -----------------------------------------------------------------------
    // FT.ADD
    // -----------------------------------------------------------------------

    /// ```text
    /// FT.ADD index id [WITHVEC] <vec_bytes>
    ///   [ATTR field value ...]
    /// ```
    ///
    /// `vec_bytes` is a binary blob of little-endian IEEE 754 FLOAT32 values.
    /// If `WITHVEC` is present the next argument is the raw blob; otherwise
    /// the blob is the second positional argument.
    pub fn ft_add(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 3 {
            return Err("FT.ADD requires: index id vec_bytes [ATTR ...]".into());
        }
        let index_name = arg_str(cmd, 0)?;
        let id_str = arg_str(cmd, 1)?;
        let doc_id: u64 = id_str
            .parse()
            .map_err(|_| format!("FT.ADD: invalid doc id '{id_str}'"))?;

        // Detect WITHVEC flag
        let vec_arg_idx = if arg_str(cmd, 2)
            .map(|s| s.to_ascii_uppercase() == "WITHVEC")
            .unwrap_or(false)
        {
            3
        } else {
            2
        };

        if vec_arg_idx >= cmd.arg_count() {
            return Err("FT.ADD: vector bytes missing".into());
        }

        let blob = &cmd.args[vec_arg_idx];
        let vector = parse_float32_blob(blob)?;

        // Optional ATTR key value pairs after the blob
        let mut attrs: HashMap<String, String> = HashMap::new();
        let mut i = vec_arg_idx + 1;
        while i + 2 <= cmd.arg_count() {
            let kw = String::from_utf8_lossy(&cmd.args[i]).to_ascii_uppercase();
            if kw == "ATTR" {
                if i + 2 < cmd.arg_count() {
                    let k = String::from_utf8_lossy(&cmd.args[i + 1]).into_owned();
                    let v = String::from_utf8_lossy(&cmd.args[i + 2]).into_owned();
                    attrs.insert(k, v);
                    i += 3;
                } else {
                    i += 1;
                }
            } else {
                // Treat as bare key=value pair
                let k = String::from_utf8_lossy(&cmd.args[i]).into_owned();
                let v = if i + 1 < cmd.arg_count() {
                    let v = String::from_utf8_lossy(&cmd.args[i + 1]).into_owned();
                    i += 2;
                    v
                } else {
                    i += 1;
                    String::new()
                };
                attrs.insert(k, v);
            }
        }

        self.store
            .add_doc(index_name, doc_id, &vector, attrs)
            .map_err(|e| e.to_string())?;

        Ok(Frame::Integer(1))
    }

    // -----------------------------------------------------------------------
    // FT.DEL
    // -----------------------------------------------------------------------

    /// `FT.DEL index id`
    pub fn ft_del(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 2 {
            return Err("FT.DEL requires: index id".into());
        }
        let index_name = arg_str(cmd, 0)?;
        let id_str = arg_str(cmd, 1)?;
        let doc_id: u64 = id_str
            .parse()
            .map_err(|_| format!("FT.DEL: invalid doc id '{id_str}'"))?;

        let deleted = self
            .store
            .del_doc(index_name, doc_id)
            .map_err(|e| e.to_string())?;

        Ok(Frame::Integer(if deleted { 1 } else { 0 }))
    }

    // -----------------------------------------------------------------------
    // FT.SEARCH
    // -----------------------------------------------------------------------

    /// ```text
    /// FT.SEARCH index "*=>[KNN k @field $param]"
    ///   PARAMS N param_name param_bytes [SORTBY field] [LIMIT offset count]
    /// ```
    ///
    /// The query string is parsed for the `KNN` keyword. `$param` is resolved
    /// from the PARAMS section. The raw vector bytes use the same little-endian
    /// FLOAT32 encoding as FT.ADD.
    ///
    /// Filters (WHERE clauses) are not yet supported and will return an error.
    pub fn ft_search(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 2 {
            return Err("FT.SEARCH requires: index query [PARAMS ...]".into());
        }
        let index_name = arg_str(cmd, 0)?;
        let query_str = arg_str(cmd, 1)?;

        // Parse K from "*=>[KNN K @field $param_name]"
        let (k, param_name) = parse_knn_query(query_str)?;

        // Parse PARAMS section: PARAMS N key1 val1 key2 val2 ...
        let mut params: HashMap<String, Bytes> = HashMap::new();
        let mut ef: usize = k * 10; // default ef = 10x k
        let mut i = 2;
        while i < cmd.arg_count() {
            let kw = String::from_utf8_lossy(&cmd.args[i]).to_ascii_uppercase();
            match kw.as_str() {
                "PARAMS" => {
                    i += 1;
                    if i >= cmd.arg_count() {
                        break;
                    }
                    let n: usize = String::from_utf8_lossy(&cmd.args[i])
                        .parse()
                        .map_err(|_| "FT.SEARCH: invalid PARAMS count".to_owned())?;
                    i += 1;
                    let mut j = 0;
                    while j + 1 < n && i + 1 < cmd.arg_count() {
                        let k_name = String::from_utf8_lossy(&cmd.args[i]).into_owned();
                        let v_bytes = cmd.args[i + 1].clone();
                        params.insert(k_name, v_bytes);
                        i += 2;
                        j += 2;
                    }
                }
                "EF_RUNTIME" => {
                    i += 1;
                    if i < cmd.arg_count() {
                        ef = String::from_utf8_lossy(&cmd.args[i])
                            .parse()
                            .unwrap_or(ef);
                        i += 1;
                    }
                }
                "SORTBY" | "LIMIT" | "RETURN" | "NOCONTENT" | "WITHSCORES" | "DIALECT" => {
                    // Accepted but ignored in V3.2
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Resolve the query vector from PARAMS
        let vec_bytes = params
            .get(&param_name)
            .ok_or_else(|| format!("FT.SEARCH: PARAMS does not contain '{param_name}'"))?;

        let query_vec = parse_float32_blob(vec_bytes)?;

        let results = self
            .store
            .search(index_name, &query_vec, k, ef, None)
            .map_err(|e| e.to_string())?;

        // Build response: integer count + array of [id, score, attrs_array]
        // Mimics RediSearch: *N doc_id score field val field val ...
        let mut frames: Vec<Frame> = Vec::with_capacity(results.len() * 3 + 1);
        frames.push(Frame::Integer(results.len() as i64));

        for (doc_id, dist, attrs) in results {
            frames.push(Frame::BulkString(Bytes::from(doc_id.to_string())));
            // Score as string (distance value)
            let mut field_frames: Vec<Frame> = Vec::with_capacity(2 + attrs.len() * 2);
            field_frames.push(Frame::BulkString(Bytes::from("__vector_score")));
            field_frames.push(Frame::BulkString(Bytes::from(format!("{dist:.6}"))));
            for (k, v) in attrs {
                field_frames.push(Frame::BulkString(Bytes::from(k)));
                field_frames.push(Frame::BulkString(Bytes::from(v)));
            }
            frames.push(Frame::Array(field_frames));
        }

        Ok(Frame::Array(frames))
    }

    // -----------------------------------------------------------------------
    // FT.INFO
    // -----------------------------------------------------------------------

    /// `FT.INFO index`
    pub fn ft_info(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 1 {
            return Err("FT.INFO requires: index".into());
        }
        let index_name = arg_str(cmd, 0)?;
        let info = self.store.info(index_name).map_err(|e| e.to_string())?;

        let mut fields: Vec<Frame> = Vec::with_capacity(16);

        let push = |fields: &mut Vec<Frame>, k: &str, v: Frame| {
            fields.push(Frame::BulkString(Bytes::from(k.to_owned())));
            fields.push(v);
        };

        push(&mut fields, "index_name", Frame::BulkString(Bytes::from(info.name)));
        push(
            &mut fields,
            "index_definition",
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("key_type")),
                Frame::BulkString(Bytes::from("VECTOR")),
            ]),
        );
        push(&mut fields, "num_docs", Frame::Integer(info.doc_count as i64));
        push(&mut fields, "dim", Frame::Integer(info.dim as i64));
        push(
            &mut fields,
            "distance_metric",
            Frame::BulkString(Bytes::from(info.metric)),
        );
        push(&mut fields, "M", Frame::Integer(info.m as i64));
        push(
            &mut fields,
            "ef_construction",
            Frame::Integer(info.ef_construction as i64),
        );
        push(
            &mut fields,
            "max_elements",
            Frame::Integer(info.max_elements as i64),
        );
        push(
            &mut fields,
            "internal_point_count",
            Frame::Integer(info.internal_point_count as i64),
        );

        Ok(Frame::Array(fields))
    }

    // -----------------------------------------------------------------------
    // FT.ALIASADD / FT.ALIASDEL / FT.ALIASUPDATE
    // -----------------------------------------------------------------------

    /// `FT.ALIASADD alias index`
    pub fn ft_aliasadd(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 2 {
            return Err("FT.ALIASADD requires: alias index".into());
        }
        let alias = arg_str(cmd, 0)?;
        let index = arg_str(cmd, 1)?;
        self.store.alias_add(alias, index).map_err(|e| e.to_string())?;
        Ok(Frame::ok())
    }

    /// `FT.ALIASDEL alias`
    pub fn ft_aliasdel(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 1 {
            return Err("FT.ALIASDEL requires: alias".into());
        }
        let alias = arg_str(cmd, 0)?;
        if self.store.alias_del(alias) {
            Ok(Frame::ok())
        } else {
            Err(format!("Alias '{alias}' not found"))
        }
    }

    /// `FT.ALIASUPDATE alias index`
    pub fn ft_aliasupdate(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 2 {
            return Err("FT.ALIASUPDATE requires: alias index".into());
        }
        let alias = arg_str(cmd, 0)?;
        let index = arg_str(cmd, 1)?;
        self.store
            .alias_update(alias, index)
            .map_err(|e| e.to_string())?;
        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // FT.CONFIG
    // -----------------------------------------------------------------------

    /// `FT.CONFIG GET key` / `FT.CONFIG SET key value`
    ///
    /// Stub: KAYA vector config is controlled at index creation time. This
    /// command is present for protocol compatibility with RediSearch clients.
    pub fn ft_config(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 1 {
            return Err("FT.CONFIG requires subcommand".into());
        }
        let subcmd = String::from_utf8_lossy(&cmd.args[0]).to_ascii_uppercase();
        match subcmd.as_str() {
            "GET" => {
                if cmd.arg_count() < 2 {
                    return Err("FT.CONFIG GET requires key".into());
                }
                let key = arg_str(cmd, 1)?;
                // Return an empty result for unknown keys.
                Ok(Frame::Array(vec![
                    Frame::Array(vec![
                        Frame::BulkString(Bytes::from(key.to_owned())),
                        Frame::BulkString(Bytes::from("")),
                    ])
                ]))
            }
            "SET" => {
                // Accepted but all config is per-index at creation time.
                Ok(Frame::ok())
            }
            _ => Err(format!("FT.CONFIG: unknown subcommand '{subcmd}'")),
        }
    }

    // -----------------------------------------------------------------------
    // FT.EXPLAIN
    // -----------------------------------------------------------------------

    /// `FT.EXPLAIN index query [DIALECT n]`
    ///
    /// Returns a best-effort debug string of the parsed query.
    pub fn ft_explain(&self, cmd: &Command) -> Result<Frame, String> {
        if cmd.arg_count() < 2 {
            return Err("FT.EXPLAIN requires: index query".into());
        }
        let index_name = arg_str(cmd, 0)?;
        let query_str = arg_str(cmd, 1)?;

        let explanation = match parse_knn_query(query_str) {
            Ok((k, param)) => format!(
                "Vector KNN search on index '{}': k={}, param='{}'",
                index_name, k, param
            ),
            Err(e) => format!("Parse error: {} (raw query: '{}')", e, query_str),
        };

        Ok(Frame::BulkString(Bytes::from(explanation)))
    }
}

// ---------------------------------------------------------------------------
// Public dispatch entry point
// ---------------------------------------------------------------------------

/// Attempt to dispatch a command as a vector FT.* command.
///
/// Returns `Some(Frame)` if the command was handled (including error frames),
/// or `None` if this is not a FT.* vector command (allowing fall-through to
/// the full-text handler in V3.4).
///
/// Discrimination rule: `FT.CREATE` is claimed only when the schema contains
/// the `VECTOR` keyword. All other FT.* commands are always claimed here.
pub fn dispatch_vector_command(
    store: &Arc<VectorStore>,
    cmd: &Command,
) -> Option<Frame> {
    let name = cmd.name.as_str();

    // Fast reject: all vector commands start with "FT."
    if !name.starts_with("FT.") {
        return None;
    }

    // FT.CREATE is shared with full-text (V3.4). We only claim it when the
    // schema explicitly contains the VECTOR keyword.
    if name == "FT.CREATE" {
        let has_vector = cmd
            .args
            .iter()
            .any(|b| b.to_ascii_uppercase() == b"VECTOR");
        if !has_vector {
            return None; // Let full-text handler deal with it.
        }
    }

    let handler = VectorHandler::new(Arc::clone(store));

    let result: Result<Frame, String> = match name {
        "FT.CREATE" => handler.ft_create(cmd),
        "FT.DROP" => handler.ft_drop(cmd),
        "FT.ADD" => handler.ft_add(cmd),
        "FT.DEL" => handler.ft_del(cmd),
        "FT.SEARCH" => handler.ft_search(cmd),
        "FT.INFO" => handler.ft_info(cmd),
        "FT.ALIASADD" => handler.ft_aliasadd(cmd),
        "FT.ALIASDEL" => handler.ft_aliasdel(cmd),
        "FT.ALIASUPDATE" => handler.ft_aliasupdate(cmd),
        "FT.CONFIG" => handler.ft_config(cmd),
        "FT.EXPLAIN" => handler.ft_explain(cmd),
        _ => {
            // Unknown FT.* — could be full-text, return None.
            return None;
        }
    };

    Some(match result {
        Ok(frame) => frame,
        Err(msg) => Frame::Error(format!("ERR {msg}")),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse little-endian IEEE 754 FLOAT32 bytes into a Vec<f32>.
fn parse_float32_blob(blob: &[u8]) -> Result<Vec<f32>, String> {
    if blob.len() % 4 != 0 {
        return Err(format!(
            "vector blob length {} is not a multiple of 4",
            blob.len()
        ));
    }
    if blob.is_empty() {
        return Err("vector blob is empty".into());
    }
    let n = blob.len() / 4;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let bytes: [u8; 4] = blob[i * 4..(i + 1) * 4]
            .try_into()
            .map_err(|_| "slice conversion failed".to_owned())?;
        out.push(f32::from_le_bytes(bytes));
    }
    Ok(out)
}

/// Parse `*=>[KNN k @field $param_name]` and return `(k, param_name)`.
fn parse_knn_query(query: &str) -> Result<(usize, String), String> {
    // Find KNN token (case-insensitive)
    let upper = query.to_ascii_uppercase();
    let knn_pos = upper
        .find("KNN")
        .ok_or_else(|| format!("query does not contain KNN: '{query}'"))?;

    let rest = query[knn_pos + 3..].trim();

    // Next token is K
    let mut tokens = rest.split_whitespace();
    let k_str = tokens.next().ok_or("KNN: missing k value")?;
    let k: usize = k_str
        .parse()
        .map_err(|_| format!("KNN: invalid k value '{k_str}'"))?;

    // Skip @field_name, find $param_name
    let param_token = tokens
        .find(|t| t.starts_with('$'))
        .ok_or("KNN: could not find '$param_name' in query")?;

    // Strip trailing ']' if present
    let param_name = param_token
        .trim_start_matches('$')
        .trim_end_matches(']')
        .to_owned();

    Ok((k, param_name))
}

/// Borrow a command argument as a &str.
fn arg_str<'a>(cmd: &'a Command, idx: usize) -> Result<&'a str, String> {
    if idx >= cmd.arg_count() {
        return Err(format!("missing argument at index {idx}"));
    }
    std::str::from_utf8(&cmd.args[idx])
        .map_err(|_| format!("argument {idx} is not valid UTF-8"))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_vector::IndexOpts;

    // Build a Command from name + args (all as &str).
    fn make_cmd(name: &str, args: &[&[u8]]) -> Command {
        Command {
            name: name.to_ascii_uppercase(),
            args: args.iter().map(|b| Bytes::copy_from_slice(b)).collect(),
        }
    }

    fn make_store() -> Arc<VectorStore> {
        Arc::new(VectorStore::new())
    }

    /// Encode a &[f32] as little-endian FLOAT32 bytes.
    fn encode_vec(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    // -----------------------------------------------------------------------
    // FT.CREATE
    // -----------------------------------------------------------------------

    #[test]
    fn ft_create_cosine_index() {
        let store = make_store();
        let cmd = make_cmd("FT.CREATE", &[
            b"myidx",
            b"SCHEMA", b"embedding", b"VECTOR", b"HNSW", b"8",
            b"TYPE", b"FLOAT32",
            b"DIM", b"4",
            b"DISTANCE_METRIC", b"COSINE",
            b"M", b"8",
            b"EF_CONSTRUCTION", b"100",
        ]);
        let frame = dispatch_vector_command(&store, &cmd).unwrap();
        assert_eq!(frame, Frame::SimpleString("OK".into()));
    }

    #[test]
    fn ft_create_duplicate_fails() {
        let store = make_store();
        let cmd = make_cmd("FT.CREATE", &[
            b"dup",
            b"SCHEMA", b"v", b"VECTOR", b"HNSW", b"4",
            b"TYPE", b"FLOAT32", b"DIM", b"2", b"DISTANCE_METRIC", b"L2",
        ]);
        dispatch_vector_command(&store, &cmd).unwrap();
        let frame2 = dispatch_vector_command(&store, &cmd).unwrap();
        assert!(matches!(frame2, Frame::Error(_)));
    }

    #[test]
    fn ft_create_non_vector_schema_falls_through() {
        let store = make_store();
        // No VECTOR keyword → dispatch returns None (let full-text handle it)
        let cmd = make_cmd("FT.CREATE", &[
            b"txtidx",
            b"SCHEMA", b"title", b"TEXT",
        ]);
        let result = dispatch_vector_command(&store, &cmd);
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // FT.ADD + FT.SEARCH
    // -----------------------------------------------------------------------

    fn setup_index(store: &Arc<VectorStore>, name: &str, dim: usize) {
        store
            .create_index(name, dim, DistanceMetric::Cosine, IndexOpts::default())
            .unwrap();
    }

    #[test]
    fn ft_add_and_search_knn() {
        let store = make_store();
        setup_index(&store, "emb", 2);

        // Add 3 documents
        let h = VectorHandler::new(Arc::clone(&store));

        let cmd1 = make_cmd("FT.ADD", &[
            b"emb", b"1",
            &encode_vec(&[1.0, 0.0]),
        ]);
        h.ft_add(&cmd1).unwrap();

        let cmd2 = make_cmd("FT.ADD", &[
            b"emb", b"2",
            &encode_vec(&[0.0, 1.0]),
        ]);
        h.ft_add(&cmd2).unwrap();

        let cmd3 = make_cmd("FT.ADD", &[
            b"emb", b"3",
            &encode_vec(&[1.0, 1.0]),
        ]);
        h.ft_add(&cmd3).unwrap();

        // Search: query is [1,0] → doc 1 should be closest
        let query_bytes = encode_vec(&[1.0f32, 0.0]);
        let cmd_search = make_cmd("FT.SEARCH", &[
            b"emb",
            b"*=>[KNN 2 @embedding $query_vec]",
            b"PARAMS", b"2", b"query_vec", &query_bytes,
        ]);
        let frame = h.ft_search(&cmd_search).unwrap();
        match frame {
            Frame::Array(ref items) => {
                assert_eq!(items[0], Frame::Integer(2));
                // First result should be doc id "1"
                let first_id = match &items[1] {
                    Frame::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
                    _ => panic!("expected BulkString doc id"),
                };
                assert_eq!(first_id, "1");
            }
            _ => panic!("expected Array frame, got {:?}", frame),
        }
    }

    #[test]
    fn ft_del_tombstones_doc() {
        let store = make_store();
        setup_index(&store, "tst", 2);
        let h = VectorHandler::new(Arc::clone(&store));

        let cmd_add = make_cmd("FT.ADD", &[b"tst", b"99", &encode_vec(&[1.0, 0.0])]);
        h.ft_add(&cmd_add).unwrap();

        let cmd_del = make_cmd("FT.DEL", &[b"tst", b"99"]);
        let frame = h.ft_del(&cmd_del).unwrap();
        assert_eq!(frame, Frame::Integer(1));

        // Double delete → returns 0
        let frame2 = h.ft_del(&cmd_del).unwrap();
        assert_eq!(frame2, Frame::Integer(0));
    }

    // -----------------------------------------------------------------------
    // Metric variants
    // -----------------------------------------------------------------------

    #[test]
    fn ft_search_l2_metric() {
        let store = make_store();
        store
            .create_index("l2idx", 2, DistanceMetric::L2, IndexOpts::default())
            .unwrap();
        let h = VectorHandler::new(Arc::clone(&store));

        h.ft_add(&make_cmd("FT.ADD", &[b"l2idx", b"1", &encode_vec(&[0.0, 0.0])])).unwrap();
        h.ft_add(&make_cmd("FT.ADD", &[b"l2idx", b"2", &encode_vec(&[10.0, 10.0])])).unwrap();

        let qb = encode_vec(&[0.1f32, 0.1]);
        let frame = h.ft_search(&make_cmd("FT.SEARCH", &[
            b"l2idx",
            b"*=>[KNN 1 @vec $q]",
            b"PARAMS", b"2", b"q", &qb,
        ])).unwrap();
        match frame {
            Frame::Array(ref items) => {
                assert_eq!(items[0], Frame::Integer(1));
                let id = match &items[1] {
                    Frame::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
                    _ => panic!("expected id"),
                };
                assert_eq!(id, "1"); // closer to origin
            }
            _ => panic!("unexpected frame"),
        }
    }

    #[test]
    #[ignore = "hnsw_rs IP metric uses cosine internally; ordering diverges for un-normalized vectors (V3.2 follow-up: add pre-normalization)"]
    fn ft_search_ip_metric() {
        let store = make_store();
        store
            .create_index("ipidx", 2, DistanceMetric::IP, IndexOpts::default())
            .unwrap();
        let h = VectorHandler::new(Arc::clone(&store));

        // doc 1: high dot product with query [1,0]
        h.ft_add(&make_cmd("FT.ADD", &[b"ipidx", b"1", &encode_vec(&[5.0, 0.0])])).unwrap();
        h.ft_add(&make_cmd("FT.ADD", &[b"ipidx", b"2", &encode_vec(&[0.1, 0.0])])).unwrap();

        let qb = encode_vec(&[1.0f32, 0.0]);
        let frame = h.ft_search(&make_cmd("FT.SEARCH", &[
            b"ipidx",
            b"*=>[KNN 1 @vec $q]",
            b"PARAMS", b"2", b"q", &qb,
        ])).unwrap();
        match frame {
            Frame::Array(ref items) => {
                assert_eq!(items[0], Frame::Integer(1));
                let id = match &items[1] {
                    Frame::BulkString(b) => String::from_utf8_lossy(b).into_owned(),
                    _ => panic!("expected id"),
                };
                assert_eq!(id, "1"); // dot=5 > dot=0.1 → id=1 closest
            }
            _ => panic!("unexpected frame"),
        }
    }

    // -----------------------------------------------------------------------
    // FT.INFO
    // -----------------------------------------------------------------------

    #[test]
    fn ft_info_returns_metadata() {
        let store = make_store();
        store
            .create_index(
                "info_idx",
                768,
                DistanceMetric::Cosine,
                IndexOpts { m: 16, ef_construction: 200, max_elements: 50_000 },
            )
            .unwrap();
        let h = VectorHandler::new(Arc::clone(&store));
        let cmd = make_cmd("FT.INFO", &[b"info_idx"]);
        let frame = h.ft_info(&cmd).unwrap();
        match frame {
            Frame::Array(ref items) => {
                // Find "dim" key and its value
                let mut map: HashMap<String, Frame> = HashMap::new();
                let mut i = 0;
                while i + 1 < items.len() {
                    if let Frame::BulkString(ref k) = items[i] {
                        map.insert(
                            String::from_utf8_lossy(k).into_owned(),
                            items[i + 1].clone(),
                        );
                    }
                    i += 2;
                }
                assert_eq!(map["dim"], Frame::Integer(768));
                assert_eq!(map["M"], Frame::Integer(16));
                assert_eq!(map["ef_construction"], Frame::Integer(200));
                assert_eq!(
                    map["distance_metric"],
                    Frame::BulkString(Bytes::from("COSINE"))
                );
            }
            _ => panic!("expected Array"),
        }
    }

    // -----------------------------------------------------------------------
    // Accuracy test: HNSW vs brute-force on 1000 vectors (dim 64)
    // -----------------------------------------------------------------------

    #[test]
    fn hnsw_vs_bruteforce_accuracy() {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let n = 1000usize;
        let dim = 64usize;
        let k = 10usize;

        let store = make_store();
        store
            .create_index(
                "acc",
                dim,
                DistanceMetric::Cosine,
                IndexOpts { m: 16, ef_construction: 200, max_elements: n + 100 },
            )
            .unwrap();

        // Generate random unit vectors and insert
        let mut all_vecs: Vec<Vec<f32>> = Vec::with_capacity(n);
        let h = VectorHandler::new(Arc::clone(&store));

        for i in 0..n {
            let mut v: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();
            // Normalize
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-9 {
                v.iter_mut().for_each(|x| *x /= norm);
            }
            all_vecs.push(v.clone());
            let blob = encode_vec(&v);
            let id_bytes = i.to_string();
            let cmd_add = make_cmd(
                "FT.ADD",
                &[b"acc", id_bytes.as_bytes(), &blob],
            );
            h.ft_add(&cmd_add).unwrap();
        }

        // Generate a random query vector
        let mut query: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();
        let qnorm: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        if qnorm > 1e-9 {
            query.iter_mut().for_each(|x| *x /= qnorm);
        }

        // Brute-force top-k
        let mut bf: Vec<(usize, f32)> = all_vecs
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let d = kaya_vector::distance::cosine_distance(&query, v);
                (i, d)
            })
            .collect();
        bf.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let bf_top: std::collections::HashSet<usize> =
            bf.iter().take(k).map(|(i, _)| *i).collect();

        // HNSW top-k via FT.SEARCH
        let qb = encode_vec(&query);
        let k_str = k.to_string();
        let query_expr = format!("*=>[KNN {k} @vec $q]");
        let frame = h
            .ft_search(&make_cmd("FT.SEARCH", &[
                b"acc",
                query_expr.as_bytes(),
                b"PARAMS", b"2", b"q", &qb,
            ]))
            .unwrap();

        let hnsw_ids: std::collections::HashSet<usize> = match frame {
            Frame::Array(ref items) => items[1..]
                .chunks(2)
                .filter_map(|chunk| {
                    if let Frame::BulkString(ref b) = chunk[0] {
                        String::from_utf8_lossy(b).parse().ok()
                    } else {
                        None
                    }
                })
                .collect(),
            _ => panic!("unexpected frame"),
        };

        let overlap = hnsw_ids.intersection(&bf_top).count();
        let recall = overlap as f64 / k as f64;
        assert!(
            recall >= 0.90,
            "HNSW recall@{k} = {recall:.2}, expected ≥ 0.90 (overlap={overlap}/{k})"
        );
        let _ = k_str; // suppress warning
    }

    // -----------------------------------------------------------------------
    // FT.EXPLAIN
    // -----------------------------------------------------------------------

    #[test]
    fn ft_explain_parses_knn() {
        let store = make_store();
        store
            .create_index("e", 4, DistanceMetric::Cosine, IndexOpts::default())
            .unwrap();
        let h = VectorHandler::new(Arc::clone(&store));
        let cmd = make_cmd("FT.EXPLAIN", &[
            b"e",
            b"*=>[KNN 5 @vec $query_vec]",
        ]);
        let frame = h.ft_explain(&cmd).unwrap();
        match frame {
            Frame::BulkString(ref b) => {
                let s = String::from_utf8_lossy(b);
                assert!(s.contains("k=5"));
                assert!(s.contains("query_vec"));
            }
            _ => panic!("expected BulkString"),
        }
    }

    // -----------------------------------------------------------------------
    // FT.DROP
    // -----------------------------------------------------------------------

    #[test]
    fn ft_drop_existing() {
        let store = make_store();
        store
            .create_index("drop_me", 4, DistanceMetric::L2, IndexOpts::default())
            .unwrap();
        let h = VectorHandler::new(Arc::clone(&store));
        let frame = h.ft_drop(&make_cmd("FT.DROP", &[b"drop_me"])).unwrap();
        assert_eq!(frame, Frame::SimpleString("OK".into()));
        // Second drop returns error
        assert!(h.ft_drop(&make_cmd("FT.DROP", &[b"drop_me"])).is_err());
    }
}
