//! KAYA JSON command handlers.
//!
//! Each `handle_*` function maps to a KAYA JSON command and follows the
//! signature:
//! ```text
//! pub fn handle_X(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError>
//! ```
//!
//! The functions are intentionally NOT wired into `router.rs` yet — the
//! wire-up will be done in a separate step.
//!
//! ## Command surface
//! JSON.SET, JSON.GET, JSON.DEL, JSON.FORGET (alias DEL), JSON.TYPE,
//! JSON.NUMINCRBY, JSON.NUMMULTBY, JSON.STRAPPEND, JSON.STRLEN,
//! JSON.ARRAPPEND, JSON.ARRLEN, JSON.ARRPOP, JSON.ARRINDEX, JSON.ARRINSERT,
//! JSON.ARRTRIM, JSON.OBJKEYS, JSON.OBJLEN, JSON.TOGGLE, JSON.CLEAR,
//! JSON.MGET, JSON.DEBUG MEMORY, JSON.RESP.

use bytes::Bytes;
use kaya_json::{JsonError, JsonSetOpts, JsonStore};
use kaya_protocol::{Command, Frame};
use serde_json::Value;
use tracing::instrument;

use crate::CommandError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `JsonError` into a `CommandError`.
impl From<JsonError> for CommandError {
    fn from(e: JsonError) -> Self {
        CommandError::Syntax(e.to_string())
    }
}

/// Parse a JSON string from a command argument at `idx`.
fn parse_json_arg(cmd: &Command, idx: usize) -> Result<Value, CommandError> {
    let s = cmd.arg_str(idx)?;
    serde_json::from_str(s).map_err(|e| {
        CommandError::Syntax(format!("invalid JSON at argument {idx}: {e}"))
    })
}

/// Return the path argument at `idx`, defaulting to `"$"` if not provided.
fn path_or_root(cmd: &Command, idx: usize) -> &str {
    cmd.arg_str(idx).unwrap_or("$")
}

/// Encode a `serde_json::Value` as a `Frame::BulkString` (compact JSON).
fn value_to_bulk(v: Value) -> Frame {
    Frame::BulkString(Bytes::from(v.to_string()))
}

// ---------------------------------------------------------------------------
// JSON.SET  key path value [NX | XX]
// ---------------------------------------------------------------------------

/// Handle JSON.SET key path value [NX | XX]
#[instrument(skip(store, cmd))]
pub fn handle_json_set(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: "JSON.SET".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;
    let value = parse_json_arg(cmd, 2)?;

    let mut opts = JsonSetOpts::default();
    let mut i = 3;
    while i < cmd.arg_count() {
        match cmd.arg_str(i)?.to_ascii_uppercase().as_str() {
            "NX" => opts.nx = true,
            "XX" => opts.xx = true,
            other => {
                return Err(CommandError::Syntax(format!(
                    "unknown JSON.SET option: {other}"
                )))
            }
        }
        i += 1;
    }

    match store.set(key, path, value, opts) {
        Ok(()) => Ok(Frame::ok()),
        Err(JsonError::NxConditionFailed) | Err(JsonError::XxConditionFailed) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.GET  key [path [path …]]
// ---------------------------------------------------------------------------

/// Handle JSON.GET key [path …]
#[instrument(skip(store, cmd))]
pub fn handle_json_get(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.GET".into() });
    }
    let key = cmd.arg_bytes(0)?;

    let paths: Vec<&str> = if cmd.arg_count() == 1 {
        vec!["$"]
    } else {
        (1..cmd.arg_count())
            .map(|i| cmd.arg_str(i))
            .collect::<Result<Vec<_>, _>>()?
    };

    match store.get(key, &paths) {
        Ok(v) => Ok(value_to_bulk(v)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.DEL  key [path]
// JSON.FORGET  key [path]   (alias)
// ---------------------------------------------------------------------------

/// Handle JSON.DEL key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_del(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.DEL".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);
    let n = store.del(key, path);
    Ok(Frame::Integer(n as i64))
}

/// Handle JSON.FORGET key [path] (alias for JSON.DEL).
pub fn handle_json_forget(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    handle_json_del(store, cmd)
}

// ---------------------------------------------------------------------------
// JSON.TYPE  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.TYPE key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_type(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.TYPE".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.type_at(key, path) {
        Some(t) => Ok(Frame::SimpleString(t.into())),
        None => Ok(Frame::Null),
    }
}

// ---------------------------------------------------------------------------
// JSON.NUMINCRBY  key path value
// ---------------------------------------------------------------------------

/// Handle JSON.NUMINCRBY key path value
#[instrument(skip(store, cmd))]
pub fn handle_json_numincrby(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: "JSON.NUMINCRBY".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;
    let delta: f64 = cmd
        .arg_str(2)?
        .parse()
        .map_err(|_| CommandError::Syntax("value is not a number".into()))?;

    let result = store.num_incrby(key, path, delta)?;
    Ok(value_to_bulk(result))
}

// ---------------------------------------------------------------------------
// JSON.NUMMULTBY  key path value
// ---------------------------------------------------------------------------

/// Handle JSON.NUMMULTBY key path value
#[instrument(skip(store, cmd))]
pub fn handle_json_nummultby(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: "JSON.NUMMULTBY".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;
    let factor: f64 = cmd
        .arg_str(2)?
        .parse()
        .map_err(|_| CommandError::Syntax("value is not a number".into()))?;

    let result = store.num_multby(key, path, factor)?;
    Ok(value_to_bulk(result))
}

// ---------------------------------------------------------------------------
// JSON.STRAPPEND  key [path] value
// ---------------------------------------------------------------------------

/// Handle JSON.STRAPPEND key [path] value
#[instrument(skip(store, cmd))]
pub fn handle_json_strappend(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    // Signature: key path value   OR   key value (path defaults to $)
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: "JSON.STRAPPEND".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let (path, suffix_raw) = if cmd.arg_count() >= 3 {
        (cmd.arg_str(1)?, cmd.arg_str(2)?)
    } else {
        ("$", cmd.arg_str(1)?)
    };

    // The suffix may be a JSON string (with quotes) or a bare string.
    let suffix: String = if suffix_raw.starts_with('"') {
        serde_json::from_str(suffix_raw).unwrap_or_else(|_| suffix_raw.to_string())
    } else {
        suffix_raw.to_string()
    };

    let new_len = store.str_append(key, path, &suffix)?;
    Ok(Frame::Integer(new_len as i64))
}

// ---------------------------------------------------------------------------
// JSON.STRLEN  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.STRLEN key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_strlen(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.STRLEN".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.str_len(key, path) {
        Ok(n) => Ok(Frame::Integer(n as i64)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.ARRAPPEND  key [path] value [value …]
// ---------------------------------------------------------------------------

/// Handle JSON.ARRAPPEND key [path] value [value …]
#[instrument(skip(store, cmd))]
pub fn handle_json_arrappend(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    // JSON.ARRAPPEND key path value [value ...]
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: "JSON.ARRAPPEND".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;

    let mut values = Vec::new();
    for i in 2..cmd.arg_count() {
        values.push(parse_json_arg(cmd, i)?);
    }

    let new_len = store.arr_append(key, path, values)?;
    Ok(Frame::Integer(new_len as i64))
}

// ---------------------------------------------------------------------------
// JSON.ARRLEN  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.ARRLEN key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_arrlen(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.ARRLEN".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.arr_len(key, path) {
        Ok(n) => Ok(Frame::Integer(n as i64)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.ARRPOP  key [path [index]]
// ---------------------------------------------------------------------------

/// Handle JSON.ARRPOP key [path [index]]
///
/// Note: the `index` argument is accepted for API parity but we always pop
/// from the end. A future version may honour negative indices.
#[instrument(skip(store, cmd))]
pub fn handle_json_arrpop(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.ARRPOP".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.arr_pop(key, path) {
        Ok(v) => Ok(value_to_bulk(v)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.ARRINDEX  key path value [start [stop]]
// ---------------------------------------------------------------------------

/// Handle JSON.ARRINDEX key path value [start [stop]]
///
/// `start` and `stop` are accepted for parity; the current implementation
/// always scans the entire array.
#[instrument(skip(store, cmd))]
pub fn handle_json_arrindex(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: "JSON.ARRINDEX".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;
    let scalar = parse_json_arg(cmd, 2)?;

    match store.arr_index(key, path, &scalar) {
        Ok(idx) => Ok(Frame::Integer(idx)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Integer(-1)),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.ARRINSERT  key path index value [value …]
// ---------------------------------------------------------------------------

/// Handle JSON.ARRINSERT key path index value [value …]
#[instrument(skip(store, cmd))]
pub fn handle_json_arrinsert(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: "JSON.ARRINSERT".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;
    let index: i64 = cmd
        .arg_str(2)?
        .parse()
        .map_err(|_| CommandError::Syntax("index is not an integer".into()))?;

    let mut values = Vec::new();
    for i in 3..cmd.arg_count() {
        values.push(parse_json_arg(cmd, i)?);
    }

    let new_len = store.arr_insert(key, path, index, values)?;
    Ok(Frame::Integer(new_len as i64))
}

// ---------------------------------------------------------------------------
// JSON.ARRTRIM  key path start stop
// ---------------------------------------------------------------------------

/// Handle JSON.ARRTRIM key path start stop
#[instrument(skip(store, cmd))]
pub fn handle_json_arrtrim(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: "JSON.ARRTRIM".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = cmd.arg_str(1)?;
    let start: i64 = cmd
        .arg_str(2)?
        .parse()
        .map_err(|_| CommandError::Syntax("start is not an integer".into()))?;
    let stop: i64 = cmd
        .arg_str(3)?
        .parse()
        .map_err(|_| CommandError::Syntax("stop is not an integer".into()))?;

    let new_len = store.arr_trim(key, path, start, stop)?;
    Ok(Frame::Integer(new_len as i64))
}

// ---------------------------------------------------------------------------
// JSON.OBJKEYS  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.OBJKEYS key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_objkeys(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.OBJKEYS".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.obj_keys(key, path) {
        Ok(keys) => {
            let frames: Vec<Frame> = keys
                .into_iter()
                .map(|k| Frame::BulkString(Bytes::from(k)))
                .collect();
            Ok(Frame::Array(frames))
        }
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.OBJLEN  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.OBJLEN key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_objlen(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.OBJLEN".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.obj_len(key, path) {
        Ok(n) => Ok(Frame::Integer(n as i64)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

// ---------------------------------------------------------------------------
// JSON.TOGGLE  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.TOGGLE key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_toggle(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.TOGGLE".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    let new_val = store.toggle(key, path)?;
    Ok(Frame::Integer(if new_val { 1 } else { 0 }))
}

// ---------------------------------------------------------------------------
// JSON.CLEAR  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.CLEAR key [path]
#[instrument(skip(store, cmd))]
pub fn handle_json_clear(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.CLEAR".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    let n = store.clear(key, path)?;
    Ok(Frame::Integer(n as i64))
}

// ---------------------------------------------------------------------------
// JSON.MGET  key [key …] path
// ---------------------------------------------------------------------------

/// Handle JSON.MGET key [key …] path
///
/// Protocol: the last argument is the path; all preceding arguments are keys.
#[instrument(skip(store, cmd))]
pub fn handle_json_mget(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: "JSON.MGET".into() });
    }
    // Last arg is the path; all before are keys.
    let path = cmd.arg_str(cmd.arg_count() - 1)?;
    let keys: Vec<&[u8]> = (0..cmd.arg_count() - 1)
        .map(|i| cmd.args[i].as_ref())
        .collect();

    let results = store.mget(&keys, path);
    let frames: Vec<Frame> = results
        .into_iter()
        .map(|opt| match opt {
            Some(v) => value_to_bulk(v),
            None => Frame::Null,
        })
        .collect();
    Ok(Frame::Array(frames))
}

// ---------------------------------------------------------------------------
// JSON.DEBUG MEMORY  key
// ---------------------------------------------------------------------------

/// Handle JSON.DEBUG subcommand dispatch.
#[instrument(skip(store, cmd))]
pub fn handle_json_debug(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.DEBUG".into() });
    }
    let subcmd = cmd.arg_str(0)?.to_ascii_uppercase();
    match subcmd.as_str() {
        "MEMORY" => {
            if cmd.arg_count() < 2 {
                return Err(CommandError::WrongArity { command: "JSON.DEBUG MEMORY".into() });
            }
            let key = cmd.arg_bytes(1)?;
            match store.resp_debug_memory(key) {
                Some(size) => Ok(Frame::Integer(size as i64)),
                None => Ok(Frame::Integer(0)),
            }
        }
        _ => Err(CommandError::Syntax(format!(
            "unknown JSON.DEBUG subcommand: {subcmd}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// JSON.RESP  key [path]
// ---------------------------------------------------------------------------

/// Handle JSON.RESP key [path]
///
/// Returns a RESP-style representation of the JSON value (for debugging).
/// Scalars are returned as their RESP equivalents; objects and arrays are
/// returned as arrays of alternating key/value pairs.
#[instrument(skip(store, cmd))]
pub fn handle_json_resp(store: &JsonStore, cmd: &Command) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: "JSON.RESP".into() });
    }
    let key = cmd.arg_bytes(0)?;
    let path = path_or_root(cmd, 1);

    match store.get(key, &[path]) {
        Ok(v) => Ok(json_value_to_resp(v)),
        Err(JsonError::KeyNotFound(_)) => Ok(Frame::Null),
        Err(e) => Err(e.into()),
    }
}

/// Recursively convert a `serde_json::Value` into a RESP3 `Frame`.
fn json_value_to_resp(v: Value) -> Frame {
    match v {
        Value::Null => Frame::Null,
        Value::Bool(b) => Frame::Integer(if b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Frame::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Frame::Double(f)
            } else {
                Frame::BulkString(Bytes::from(n.to_string()))
            }
        }
        Value::String(s) => Frame::BulkString(Bytes::from(s)),
        Value::Array(arr) => {
            Frame::Array(arr.into_iter().map(json_value_to_resp).collect())
        }
        Value::Object(map) => {
            let mut frames = Vec::with_capacity(map.len() * 2 + 1);
            // First element is the `{` marker (for clients that parse RESP.OBJECT).
            frames.push(Frame::SimpleString("{".into()));
            for (k, v) in map {
                frames.push(Frame::BulkString(Bytes::from(k)));
                frames.push(json_value_to_resp(v));
            }
            Frame::Array(frames)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use kaya_protocol::Command;
    use serde_json::json;

    // -- Helper: build a Command from a list of string args ------------------

    fn cmd(args: &[&str]) -> Command {
        Command {
            name: args[0].to_ascii_uppercase(),
            args: args[1..].iter().map(|s| Bytes::from(s.to_string())).collect(),
        }
    }

    fn fresh_store() -> JsonStore {
        JsonStore::new()
    }

    fn seeded_store() -> JsonStore {
        let s = fresh_store();
        let c = cmd(&["JSON.SET", "doc", "$", r#"{"name":"kaya","count":10,"tags":["db","fast"],"active":true,"nested":{"x":1}}"#]);
        handle_json_set(&s, &c).unwrap();
        s
    }

    // -- JSON.SET / JSON.GET --------------------------------------------------

    #[test]
    fn test_set_root_value() {
        let store = fresh_store();
        let c = cmd(&["JSON.SET", "k", "$", r#"{"a":1}"#]);
        let r = handle_json_set(&store, &c).unwrap();
        assert!(matches!(r, Frame::SimpleString(ref s) if s == "OK"));
    }

    #[test]
    fn test_get_dotted_path() {
        let store = seeded_store();
        let c = cmd(&["JSON.GET", "doc", "$.name"]);
        let r = handle_json_get(&store, &c).unwrap();
        // Result is BulkString JSON: ["kaya"]
        if let Frame::BulkString(b) = r {
            let v: Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(v, json!(["kaya"]));
        } else {
            panic!("expected BulkString, got {:?}", r);
        }
    }

    #[test]
    fn test_get_missing_key_returns_null() {
        let store = fresh_store();
        let c = cmd(&["JSON.GET", "no-such-key", "$"]);
        let r = handle_json_get(&store, &c).unwrap();
        assert!(matches!(r, Frame::Null));
    }

    // -- JSON.DEL / JSON.FORGET ---------------------------------------------

    #[test]
    fn test_del_returns_count() {
        let store = seeded_store();
        let c = cmd(&["JSON.DEL", "doc", "$"]);
        let r = handle_json_del(&store, &c).unwrap();
        assert_eq!(r, Frame::Integer(1));
    }

    #[test]
    fn test_forget_alias() {
        let store = seeded_store();
        let c = cmd(&["JSON.FORGET", "doc", "$"]);
        let r = handle_json_forget(&store, &c).unwrap();
        assert_eq!(r, Frame::Integer(1));
    }

    // -- JSON.TYPE ----------------------------------------------------------

    #[test]
    fn test_type_all_types() {
        let store = fresh_store();
        let c = cmd(&[
            "JSON.SET",
            "t",
            "$",
            r#"{"s":"hi","n":5,"f":1.5,"b":true,"nil":null,"arr":[1],"obj":{}}"#,
        ]);
        handle_json_set(&store, &c).unwrap();

        let check = |path: &str, expected: &str| {
            let c2 = cmd(&["JSON.TYPE", "t", path]);
            if let Frame::SimpleString(s) = handle_json_type(&store, &c2).unwrap() {
                assert_eq!(s, expected, "path={}", path);
            } else {
                panic!("expected SimpleString for path={}", path);
            }
        };
        check("$.s", "string");
        check("$.n", "integer");
        check("$.b", "boolean");
        check("$.nil", "null");
        check("$.arr", "array");
        check("$.obj", "object");
    }

    // -- JSON.NUMINCRBY / JSON.NUMMULTBY ------------------------------------

    #[test]
    fn test_numincrby_integer() {
        let store = seeded_store();
        let c = cmd(&["JSON.NUMINCRBY", "doc", "$.count", "5"]);
        let r = handle_json_numincrby(&store, &c).unwrap();
        if let Frame::BulkString(b) = r {
            let v: Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(v, json!(15));
        } else {
            panic!("unexpected frame: {:?}", r);
        }
    }

    #[test]
    fn test_nummultby() {
        let store = seeded_store();
        let c = cmd(&["JSON.NUMMULTBY", "doc", "$.count", "2"]);
        let r = handle_json_nummultby(&store, &c).unwrap();
        if let Frame::BulkString(b) = r {
            let v: Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(v, json!(20));
        } else {
            panic!("unexpected frame: {:?}", r);
        }
    }

    // -- JSON.STRAPPEND / JSON.STRLEN ---------------------------------------

    #[test]
    fn test_strappend_and_strlen() {
        let store = seeded_store();
        let c = cmd(&["JSON.STRAPPEND", "doc", "$.name", "\"!!\""]);
        let r = handle_json_strappend(&store, &c).unwrap();
        assert_eq!(r, Frame::Integer(6)); // "kaya!!" = 6 chars

        let c2 = cmd(&["JSON.STRLEN", "doc", "$.name"]);
        let r2 = handle_json_strlen(&store, &c2).unwrap();
        assert_eq!(r2, Frame::Integer(6));
    }

    // -- JSON.ARRAPPEND / JSON.ARRLEN / JSON.ARRPOP -------------------------

    #[test]
    fn test_arr_commands() {
        let store = seeded_store();

        // ARRAPPEND
        let c = cmd(&["JSON.ARRAPPEND", "doc", "$.tags", r#""sovereign""#]);
        let r = handle_json_arrappend(&store, &c).unwrap();
        assert_eq!(r, Frame::Integer(3));

        // ARRLEN
        let c2 = cmd(&["JSON.ARRLEN", "doc", "$.tags"]);
        assert_eq!(handle_json_arrlen(&store, &c2).unwrap(), Frame::Integer(3));

        // ARRPOP
        let c3 = cmd(&["JSON.ARRPOP", "doc", "$.tags"]);
        if let Frame::BulkString(b) = handle_json_arrpop(&store, &c3).unwrap() {
            let v: Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(v, json!("sovereign"));
        } else {
            panic!("expected BulkString");
        }
    }

    // -- JSON.ARRINDEX -------------------------------------------------------

    #[test]
    fn test_arrindex() {
        let store = seeded_store();
        let c = cmd(&["JSON.ARRINDEX", "doc", "$.tags", r#""fast""#]);
        assert_eq!(handle_json_arrindex(&store, &c).unwrap(), Frame::Integer(1));

        let c2 = cmd(&["JSON.ARRINDEX", "doc", "$.tags", r#""missing""#]);
        assert_eq!(handle_json_arrindex(&store, &c2).unwrap(), Frame::Integer(-1));
    }

    // -- JSON.OBJKEYS / JSON.OBJLEN -----------------------------------------

    #[test]
    fn test_objkeys_and_objlen() {
        let store = seeded_store();
        let c = cmd(&["JSON.OBJKEYS", "doc", "$.nested"]);
        if let Frame::Array(frames) = handle_json_objkeys(&store, &c).unwrap() {
            assert_eq!(frames.len(), 1);
            assert!(matches!(&frames[0], Frame::BulkString(b) if b == "x".as_bytes()));
        } else {
            panic!("expected Array");
        }

        let c2 = cmd(&["JSON.OBJLEN", "doc", "$"]);
        assert_eq!(handle_json_objlen(&store, &c2).unwrap(), Frame::Integer(5));
    }

    // -- JSON.TOGGLE ---------------------------------------------------------

    #[test]
    fn test_toggle() {
        let store = seeded_store();
        let c = cmd(&["JSON.TOGGLE", "doc", "$.active"]);
        // Initial value is true → should become false → returns 0.
        assert_eq!(handle_json_toggle(&store, &c).unwrap(), Frame::Integer(0));
        // Toggle again → true → returns 1.
        let c2 = cmd(&["JSON.TOGGLE", "doc", "$.active"]);
        assert_eq!(handle_json_toggle(&store, &c2).unwrap(), Frame::Integer(1));
    }

    // -- JSON.MGET -----------------------------------------------------------

    #[test]
    fn test_mget_multi_key() {
        let store = fresh_store();
        let c1 = cmd(&["JSON.SET", "a", "$", r#"{"v":1}"#]);
        let c2_set = cmd(&["JSON.SET", "b", "$", r#"{"v":2}"#]);
        handle_json_set(&store, &c1).unwrap();
        handle_json_set(&store, &c2_set).unwrap();

        // JSON.MGET a b missing $.v
        let c = cmd(&["JSON.MGET", "a", "b", "missing", "$.v"]);
        if let Frame::Array(frames) = handle_json_mget(&store, &c).unwrap() {
            assert_eq!(frames.len(), 3);
            assert!(!matches!(frames[0], Frame::Null));
            assert!(!matches!(frames[1], Frame::Null));
            assert!(matches!(frames[2], Frame::Null));
        } else {
            panic!("expected Array");
        }
    }

    // -- JSON.DEBUG MEMORY ---------------------------------------------------

    #[test]
    fn test_debug_memory() {
        let store = seeded_store();
        let c = cmd(&["JSON.DEBUG", "MEMORY", "doc"]);
        if let Frame::Integer(size) = handle_json_debug(&store, &c).unwrap() {
            assert!(size > 0);
        } else {
            panic!("expected Integer");
        }
    }

    // -- JSON.RESP -----------------------------------------------------------

    #[test]
    fn test_resp_encoding() {
        let store = fresh_store();
        let c = cmd(&["JSON.SET", "r", "$", r#"{"k":42}"#]);
        handle_json_set(&store, &c).unwrap();
        let c2 = cmd(&["JSON.RESP", "r", "$"]);
        // Should return a non-null frame (exact structure varies).
        let r = handle_json_resp(&store, &c2).unwrap();
        assert!(!matches!(r, Frame::Null));
    }

    // -- NX / XX conditions --------------------------------------------------

    #[test]
    fn test_nx_condition() {
        let store = seeded_store();
        let c = cmd(&["JSON.SET", "doc", "$", r#"{}"#, "NX"]);
        let r = handle_json_set(&store, &c).unwrap();
        // NX fails because key exists → should return Null (not OK).
        assert!(matches!(r, Frame::Null));
    }

    #[test]
    fn test_xx_condition_missing_key() {
        let store = fresh_store();
        let c = cmd(&["JSON.SET", "no-key", "$", r#"{}"#, "XX"]);
        let r = handle_json_set(&store, &c).unwrap();
        // XX fails because key doesn't exist → Null.
        assert!(matches!(r, Frame::Null));
    }

    // -- JSONPath wildcard descent -------------------------------------------

    #[test]
    fn test_jsonpath_wildcard_descent() {
        let store = fresh_store();
        let c = cmd(&[
            "JSON.SET",
            "d",
            "$",
            r#"{"people":[{"name":"alice"},{"name":"bob"}]}"#,
        ]);
        handle_json_set(&store, &c).unwrap();

        let c2 = cmd(&["JSON.GET", "d", "$..name"]);
        if let Frame::BulkString(b) = handle_json_get(&store, &c2).unwrap() {
            let v: Value = serde_json::from_slice(&b).unwrap();
            assert!(v.is_array());
            let arr = v.as_array().unwrap();
            assert_eq!(arr.len(), 2);
        } else {
            panic!("expected BulkString");
        }
    }
}
