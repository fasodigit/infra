// SPDX-License-Identifier: AGPL-3.0-or-later
//! RESP3 command handlers for KAYA TimeSeries commands.
//!
//! Implements the full TS.* command surface (RedisTimeSeries parity):
//!   TS.CREATE / TS.ALTER / TS.DEL
//!   TS.ADD / TS.MADD / TS.INCRBY / TS.DECRBY
//!   TS.GET / TS.MGET
//!   TS.RANGE / TS.REVRANGE / TS.MRANGE / TS.MREVRANGE
//!   TS.CREATERULE / TS.DELETERULE
//!   TS.QUERYINDEX
//!   TS.INFO
//!
//! All handlers follow the same conventions as the rest of `kaya-commands`:
//! - No `unwrap()` / `expect()` in non-test code.
//! - Errors wrapped in `CommandError`.
//! - Tracing spans via `#[tracing::instrument]`.

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use kaya_timeseries::{
    Aggregator, DuplicatePolicy, LabelFilter, TimeSeriesStore, TsCreateOpts,
};

use crate::CommandError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse an `i64` argument from `cmd.args[idx]`.
fn parse_i64(cmd: &Command, idx: usize) -> Result<i64, CommandError> {
    cmd.arg_i64(idx).map_err(CommandError::Protocol)
}

/// Parse an `f64` argument from `cmd.args[idx]`.
fn parse_f64(cmd: &Command, idx: usize) -> Result<f64, CommandError> {
    let s = cmd.arg_str(idx)?;
    s.parse::<f64>()
        .map_err(|_| CommandError::Syntax(format!("not a float: '{s}'")))
}

/// Parse optional `RETENTION` / `DUPLICATE_POLICY` / `LABELS` options starting
/// at `idx`, filling in `opts`. Returns the index after all parsed tokens.
fn parse_create_opts(
    cmd: &Command,
    mut idx: usize,
    opts: &mut TsCreateOpts,
) -> Result<usize, CommandError> {
    while idx < cmd.arg_count() {
        let tok = cmd.arg_str(idx)?.to_ascii_uppercase();
        match tok.as_str() {
            "RETENTION" => {
                idx += 1;
                opts.retention_ms = parse_i64(cmd, idx)?;
                idx += 1;
            }
            "DUPLICATE_POLICY" | "ON_DUPLICATE" => {
                idx += 1;
                let pol_str = cmd.arg_str(idx)?;
                opts.duplicate_policy = Some(
                    DuplicatePolicy::from_str(pol_str).ok_or_else(|| {
                        CommandError::Syntax(format!(
                            "unknown duplicate policy: '{pol_str}'"
                        ))
                    })?,
                );
                idx += 1;
            }
            "LABELS" => {
                idx += 1;
                // Remaining pairs: key value key value …
                while idx + 1 < cmd.arg_count() {
                    // Stop if next token looks like another option keyword.
                    let peek = cmd.arg_str(idx)?.to_ascii_uppercase();
                    if matches!(
                        peek.as_str(),
                        "RETENTION" | "DUPLICATE_POLICY" | "ON_DUPLICATE"
                    ) {
                        break;
                    }
                    let k = cmd.arg_str(idx)?.to_string();
                    let v = cmd.arg_str(idx + 1)?.to_string();
                    opts.labels.insert(k, v);
                    idx += 2;
                }
            }
            _ => {
                // Unknown token — stop parsing options.
                break;
            }
        }
    }
    Ok(idx)
}

/// Parse optional `AGGREGATION aggtype bucket_ms` block.
fn parse_aggregation(
    cmd: &Command,
    mut idx: usize,
) -> Result<(usize, Option<Aggregator>, Option<i64>), CommandError> {
    let mut agg: Option<Aggregator> = None;
    let mut bucket_ms: Option<i64> = None;

    while idx < cmd.arg_count() {
        let tok = cmd.arg_str(idx)?.to_ascii_uppercase();
        if tok == "AGGREGATION" {
            idx += 1;
            let agg_str = cmd.arg_str(idx)?;
            agg = Some(
                Aggregator::from_str(agg_str).ok_or_else(|| {
                    CommandError::Syntax(format!("unknown aggregator: '{agg_str}'"))
                })?,
            );
            idx += 1;
            bucket_ms = Some(parse_i64(cmd, idx)?);
            idx += 1;
        } else {
            break;
        }
    }
    Ok((idx, agg, bucket_ms))
}

/// Parse a `FILTER key=val …` block starting at `idx`.
fn parse_filter(cmd: &Command, mut idx: usize) -> Result<(usize, LabelFilter), CommandError> {
    if idx >= cmd.arg_count() {
        return Err(CommandError::Syntax("FILTER keyword expected".into()));
    }
    let tok = cmd.arg_str(idx)?.to_ascii_uppercase();
    if tok != "FILTER" {
        return Err(CommandError::Syntax(format!(
            "expected FILTER, got '{tok}'"
        )));
    }
    idx += 1;
    let mut exprs: Vec<&str> = Vec::new();
    while idx < cmd.arg_count() {
        exprs.push(cmd.arg_str(idx)?);
        idx += 1;
    }
    let filter = LabelFilter::parse(&exprs)
        .map_err(|e| CommandError::Syntax(e.to_string()))?;
    Ok((idx, filter))
}

/// Convert a `TsError` into a `CommandError`.
fn ts_err(e: kaya_timeseries::TsError) -> CommandError {
    CommandError::Syntax(e.to_string())
}

// ---------------------------------------------------------------------------
// TS.CREATE
// ---------------------------------------------------------------------------

/// TS.CREATE key [RETENTION millis] [DUPLICATE_POLICY policy] [LABELS k v …]
#[tracing::instrument(skip(store, cmd))]
pub fn handle_ts_create(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let mut opts = TsCreateOpts::default();
    parse_create_opts(cmd, 1, &mut opts)?;
    store.create(key, opts).map_err(ts_err)?;
    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// TS.ALTER
// ---------------------------------------------------------------------------

/// TS.ALTER key [RETENTION millis] [DUPLICATE_POLICY policy] [LABELS k v …]
pub fn handle_ts_alter(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let mut opts = TsCreateOpts::default();
    parse_create_opts(cmd, 1, &mut opts)?;
    store.alter(key, opts).map_err(ts_err)?;
    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// TS.DEL (delete series entirely — note: TS.DEL key fromTs toTs deletes range)
// We implement both semantics: 1 arg = delete series, 3 args = delete range.
// ---------------------------------------------------------------------------

/// TS.DEL key fromTimestamp toTimestamp  — delete points in range.
pub fn handle_ts_del(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() == 1 {
        // Delete the series entirely.
        let key = cmd.arg_bytes(0)?;
        store.delete_series(key).map_err(ts_err)?;
        return Ok(Frame::ok());
    }
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let from = parse_i64(cmd, 1)?;
    let to = parse_i64(cmd, 2)?;
    let count = store.del_range(key, from, to).map_err(ts_err)?;
    Ok(Frame::Integer(count as i64))
}

// ---------------------------------------------------------------------------
// TS.ADD
// ---------------------------------------------------------------------------

/// TS.ADD key timestamp value [RETENTION …] [ON_DUPLICATE policy] [LABELS …]
pub fn handle_ts_add(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let ts = parse_i64(cmd, 1)?;
    let val = parse_f64(cmd, 2)?;

    // Optional modifiers (create-style opts).
    let mut opts = TsCreateOpts::default();
    parse_create_opts(cmd, 3, &mut opts)?;

    // Auto-create series if needed (with supplied opts).
    if !opts.labels.is_empty() || opts.retention_ms > 0 || opts.duplicate_policy.is_some() {
        let _ = store.create(key, opts); // ignore AlreadyExists
    }

    store.add(key, ts, val).map_err(ts_err)?;
    Ok(Frame::Integer(ts))
}

// ---------------------------------------------------------------------------
// TS.MADD
// ---------------------------------------------------------------------------

/// TS.MADD key ts val [key ts val …]
pub fn handle_ts_madd(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 || cmd.arg_count() % 3 != 0 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let mut results = Vec::with_capacity(cmd.arg_count() / 3);
    let mut idx = 0;
    while idx + 2 < cmd.arg_count() {
        let key = cmd.arg_bytes(idx)?;
        let ts = parse_i64(cmd, idx + 1)?;
        let val = parse_f64(cmd, idx + 2)?;
        store.add(key, ts, val).map_err(ts_err)?;
        results.push(Frame::Integer(ts));
        idx += 3;
    }
    Ok(Frame::Array(results))
}

// ---------------------------------------------------------------------------
// TS.INCRBY / TS.DECRBY
// ---------------------------------------------------------------------------

/// TS.INCRBY key value [TIMESTAMP ts]
pub fn handle_ts_incrby(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let delta = parse_f64(cmd, 1)?;
    let ts = extract_timestamp(cmd, 2)?;
    let new_val = store.incrby(key, delta, ts).map_err(ts_err)?;
    Ok(Frame::Double(new_val))
}

/// TS.DECRBY key value [TIMESTAMP ts]
pub fn handle_ts_decrby(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let delta = parse_f64(cmd, 1)?;
    let ts = extract_timestamp(cmd, 2)?;
    let new_val = store.decrby(key, delta, ts).map_err(ts_err)?;
    Ok(Frame::Double(new_val))
}

fn extract_timestamp(cmd: &Command, idx: usize) -> Result<Option<i64>, CommandError> {
    if idx + 1 < cmd.arg_count() {
        let tok = cmd.arg_str(idx)?.to_ascii_uppercase();
        if tok == "TIMESTAMP" {
            let ts = parse_i64(cmd, idx + 1)?;
            return Ok(Some(ts));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// TS.GET
// ---------------------------------------------------------------------------

/// TS.GET key — returns [timestamp value] or empty array.
pub fn handle_ts_get(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    match store.get(key).map_err(ts_err)? {
        Some((ts, val)) => Ok(Frame::Array(vec![
            Frame::Integer(ts),
            Frame::Double(val),
        ])),
        None => Ok(Frame::Array(vec![])),
    }
}

// ---------------------------------------------------------------------------
// TS.MGET
// ---------------------------------------------------------------------------

/// TS.MGET FILTER k=v … — returns array of [key, labels, [ts, val]]
pub fn handle_ts_mget(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let (_, filter) = parse_filter(cmd, 0)?;
    let results = store.mget(&filter);
    let frames = results
        .into_iter()
        .map(|(key, labels, last)| {
            let labels_frame = labels_to_frame(&labels);
            let data_frame = match last {
                Some((ts, val)) => Frame::Array(vec![Frame::Integer(ts), Frame::Double(val)]),
                None => Frame::Array(vec![]),
            };
            Frame::Array(vec![
                Frame::BulkString(Bytes::from(key)),
                labels_frame,
                data_frame,
            ])
        })
        .collect();
    Ok(Frame::Array(frames))
}

// ---------------------------------------------------------------------------
// TS.RANGE
// ---------------------------------------------------------------------------

/// TS.RANGE key fromTs toTs [AGGREGATION aggtype bucket_ms]
pub fn handle_ts_range(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let from = parse_ts_arg(cmd, 1)?;
    let to = parse_ts_arg(cmd, 2)?;
    let (_, agg, bucket_ms) = parse_aggregation(cmd, 3)?;
    let pts = store
        .range(key, from, to, agg.as_ref(), bucket_ms)
        .map_err(ts_err)?;
    Ok(points_to_frame(pts))
}

// ---------------------------------------------------------------------------
// TS.REVRANGE
// ---------------------------------------------------------------------------

/// TS.REVRANGE key fromTs toTs [AGGREGATION aggtype bucket_ms]
pub fn handle_ts_revrange(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 3 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let from = parse_ts_arg(cmd, 1)?;
    let to = parse_ts_arg(cmd, 2)?;
    let (_, agg, bucket_ms) = parse_aggregation(cmd, 3)?;
    let pts = store
        .revrange(key, from, to, agg.as_ref(), bucket_ms)
        .map_err(ts_err)?;
    Ok(points_to_frame(pts))
}

// ---------------------------------------------------------------------------
// TS.MRANGE
// ---------------------------------------------------------------------------

/// TS.MRANGE fromTs toTs [AGGREGATION aggtype bucket_ms] FILTER k=v …
pub fn handle_ts_mrange(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let from = parse_ts_arg(cmd, 0)?;
    let to = parse_ts_arg(cmd, 1)?;
    let (next_idx, agg, bucket_ms) = parse_aggregation(cmd, 2)?;
    let (_, filter) = parse_filter(cmd, next_idx)?;
    let rows = store.mrange(&filter, from, to, agg.as_ref(), bucket_ms);
    Ok(mrange_to_frame(rows))
}

// ---------------------------------------------------------------------------
// TS.MREVRANGE
// ---------------------------------------------------------------------------

/// TS.MREVRANGE fromTs toTs [AGGREGATION aggtype bucket_ms] FILTER k=v …
pub fn handle_ts_mrevrange(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 4 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let from = parse_ts_arg(cmd, 0)?;
    let to = parse_ts_arg(cmd, 1)?;
    let (next_idx, agg, bucket_ms) = parse_aggregation(cmd, 2)?;
    let (_, filter) = parse_filter(cmd, next_idx)?;
    let rows = store.mrevrange(&filter, from, to, agg.as_ref(), bucket_ms);
    Ok(mrange_to_frame(rows))
}

// ---------------------------------------------------------------------------
// TS.CREATERULE
// ---------------------------------------------------------------------------

/// TS.CREATERULE srcKey destKey AGGREGATION aggtype bucket_ms
pub fn handle_ts_createrule(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 5 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let src = cmd.arg_bytes(0)?;
    let dest = cmd.arg_bytes(1)?;
    let _agg_kw = cmd.arg_str(2)?.to_ascii_uppercase(); // should be "AGGREGATION"
    let agg_str = cmd.arg_str(3)?;
    let agg = Aggregator::from_str(agg_str)
        .ok_or_else(|| CommandError::Syntax(format!("unknown aggregator: '{agg_str}'")))?;
    let bucket_ms = parse_i64(cmd, 4)?;
    store
        .create_rule(src, dest.to_vec(), bucket_ms, agg)
        .map_err(ts_err)?;
    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// TS.DELETERULE
// ---------------------------------------------------------------------------

/// TS.DELETERULE srcKey destKey
pub fn handle_ts_deleterule(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 2 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let src = cmd.arg_bytes(0)?;
    let dest = cmd.arg_bytes(1)?;
    store.delete_rule(src, dest).map_err(ts_err)?;
    Ok(Frame::ok())
}

// ---------------------------------------------------------------------------
// TS.QUERYINDEX
// ---------------------------------------------------------------------------

/// TS.QUERYINDEX filter1 [filter2 …]
pub fn handle_ts_queryindex(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let filter_exprs: Vec<&str> = cmd
        .args
        .iter()
        .map(|b| std::str::from_utf8(b).unwrap_or(""))
        .collect();
    let filter = LabelFilter::parse(&filter_exprs).map_err(ts_err)?;
    let keys = store.query_index(&filter);
    let frames: Vec<Frame> = keys
        .into_iter()
        .map(|k| Frame::BulkString(Bytes::from(k)))
        .collect();
    Ok(Frame::Array(frames))
}

// ---------------------------------------------------------------------------
// TS.INFO
// ---------------------------------------------------------------------------

/// TS.INFO key [DEBUG]
pub fn handle_ts_info(
    store: &TimeSeriesStore,
    cmd: &Command,
) -> Result<Frame, CommandError> {
    if cmd.arg_count() < 1 {
        return Err(CommandError::WrongArity { command: cmd.name.clone() });
    }
    let key = cmd.arg_bytes(0)?;
    let info = store.info(key).map_err(ts_err)?;

    let labels_frame = labels_to_frame(&info.labels);

    let rules_frame = Frame::Array(
        info.rules
            .iter()
            .map(|r| {
                Frame::Array(vec![
                    Frame::BulkString(Bytes::from(r.dest_key.clone())),
                    Frame::Integer(r.bucket_ms),
                    Frame::BulkString(Bytes::from(r.aggregator.clone())),
                ])
            })
            .collect(),
    );

    Ok(Frame::Array(vec![
        Frame::BulkString(Bytes::from("totalSamples")),
        Frame::Integer(info.total_samples),
        Frame::BulkString(Bytes::from("memoryUsage")),
        Frame::Integer(info.memory_usage),
        Frame::BulkString(Bytes::from("firstTimestamp")),
        Frame::Integer(info.first_timestamp),
        Frame::BulkString(Bytes::from("lastTimestamp")),
        Frame::Integer(info.last_timestamp),
        Frame::BulkString(Bytes::from("retentionTime")),
        Frame::Integer(info.retention_time),
        Frame::BulkString(Bytes::from("chunkCount")),
        Frame::Integer(info.chunk_count),
        Frame::BulkString(Bytes::from("duplicatePolicy")),
        Frame::BulkString(Bytes::from(info.duplicate_policy)),
        Frame::BulkString(Bytes::from("labels")),
        labels_frame,
        Frame::BulkString(Bytes::from("rules")),
        rules_frame,
    ]))
}

// ---------------------------------------------------------------------------
// Private frame helpers
// ---------------------------------------------------------------------------

fn points_to_frame(pts: Vec<(i64, f64)>) -> Frame {
    Frame::Array(
        pts.into_iter()
            .map(|(ts, val)| Frame::Array(vec![Frame::Integer(ts), Frame::Double(val)]))
            .collect(),
    )
}

fn labels_to_frame(labels: &std::collections::HashMap<String, String>) -> Frame {
    let mut pairs = Vec::with_capacity(labels.len() * 2);
    for (k, v) in labels {
        pairs.push(Frame::BulkString(Bytes::from(k.clone())));
        pairs.push(Frame::BulkString(Bytes::from(v.clone())));
    }
    Frame::Array(pairs)
}

fn mrange_to_frame(
    rows: Vec<(Vec<u8>, std::collections::HashMap<String, String>, Vec<(i64, f64)>)>,
) -> Frame {
    Frame::Array(
        rows.into_iter()
            .map(|(key, labels, pts)| {
                Frame::Array(vec![
                    Frame::BulkString(Bytes::from(key)),
                    labels_to_frame(&labels),
                    points_to_frame(pts),
                ])
            })
            .collect(),
    )
}

/// Parse a timestamp argument that may be `-` (i64::MIN) or `+` (i64::MAX).
fn parse_ts_arg(cmd: &Command, idx: usize) -> Result<i64, CommandError> {
    let s = cmd.arg_str(idx)?;
    match s {
        "-" => Ok(i64::MIN),
        "+" => Ok(i64::MAX),
        _ => s
            .parse::<i64>()
            .map_err(|_| CommandError::Syntax(format!("invalid timestamp: '{s}'"))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_protocol::Command;

    fn cmd(name: &str, args: &[&str]) -> Command {
        Command {
            name: name.to_string(),
            args: args.iter().map(|s| Bytes::from(s.to_string())).collect(),
        }
    }

    fn store() -> TimeSeriesStore {
        TimeSeriesStore::new()
    }

    #[test]
    fn test_ts_create_and_info() {
        let s = store();
        let c = cmd("TS.CREATE", &["sensor:1", "RETENTION", "60000", "LABELS", "host", "node1"]);
        let res = handle_ts_create(&s, &c).unwrap();
        assert!(matches!(res, Frame::SimpleString(ref v) if v == "OK"));

        let c2 = cmd("TS.INFO", &["sensor:1"]);
        let info = handle_ts_info(&s, &c2).unwrap();
        // Should be an Array with key-value pairs.
        assert!(matches!(info, Frame::Array(_)));
    }

    #[test]
    fn test_ts_add_and_range() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["t"])).unwrap();

        for i in 0..5u32 {
            let ts = (i * 1000).to_string();
            let val = i.to_string();
            handle_ts_add(&s, &cmd("TS.ADD", &["t", &ts, &val])).unwrap();
        }

        let r = handle_ts_range(&s, &cmd("TS.RANGE", &["t", "0", "4999"])).unwrap();
        if let Frame::Array(pts) = r {
            assert_eq!(pts.len(), 5);
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_ts_range_aggregation_max() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["m"])).unwrap();
        for i in 0..6u32 {
            let ts = (i * 1000).to_string();
            let val = i.to_string();
            handle_ts_add(&s, &cmd("TS.ADD", &["m", &ts, &val])).unwrap();
        }
        let r = handle_ts_range(
            &s,
            &cmd("TS.RANGE", &["m", "0", "5999", "AGGREGATION", "max", "3000"]),
        )
        .unwrap();
        if let Frame::Array(buckets) = r {
            assert_eq!(buckets.len(), 2);
            // Bucket 0: max(0,1,2) = 2.0
            if let Frame::Array(ref pair) = buckets[0] {
                assert!(matches!(pair[1], Frame::Double(v) if (v - 2.0).abs() < 1e-9));
            }
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_ts_madd() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["a"])).unwrap();
        handle_ts_create(&s, &cmd("TS.CREATE", &["b"])).unwrap();
        let r = handle_ts_madd(
            &s,
            &cmd("TS.MADD", &["a", "1000", "1.5", "b", "2000", "3.0"]),
        )
        .unwrap();
        assert!(matches!(r, Frame::Array(_)));
    }

    #[test]
    fn test_ts_incrby() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["ctr"])).unwrap();
        handle_ts_incrby(&s, &cmd("TS.INCRBY", &["ctr", "10", "TIMESTAMP", "1000"])).unwrap();
        let r = handle_ts_incrby(&s, &cmd("TS.INCRBY", &["ctr", "5", "TIMESTAMP", "2000"])).unwrap();
        // last_val after first incrby = 10.0; second = 10.0 + 5.0 = 15.0
        assert!(matches!(r, Frame::Double(v) if (v - 15.0).abs() < 1e-9), "r={r:?}");
    }

    #[test]
    fn test_ts_decrby() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["gauge"])).unwrap();
        handle_ts_incrby(&s, &cmd("TS.INCRBY", &["gauge", "100", "TIMESTAMP", "1000"])).unwrap();
        let r = handle_ts_decrby(&s, &cmd("TS.DECRBY", &["gauge", "30", "TIMESTAMP", "2000"])).unwrap();
        // last_val=100, -30 = 70
        assert!(matches!(r, Frame::Double(v) if (v - 70.0).abs() < 1e-9), "r={r:?}");
    }

    #[test]
    fn test_ts_get() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["g"])).unwrap();
        handle_ts_add(&s, &cmd("TS.ADD", &["g", "5000", "99.9"])).unwrap();
        let r = handle_ts_get(&s, &cmd("TS.GET", &["g"])).unwrap();
        if let Frame::Array(pair) = r {
            assert_eq!(pair[0], Frame::Integer(5000));
            assert!(matches!(pair[1], Frame::Double(v) if (v - 99.9).abs() < 1e-9));
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_ts_revrange() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["rev"])).unwrap();
        for i in 0..5i64 {
            handle_ts_add(&s, &cmd("TS.ADD", &["rev", &(i * 1000).to_string(), &i.to_string()])).unwrap();
        }
        let r = handle_ts_revrange(&s, &cmd("TS.REVRANGE", &["rev", "0", "4000"])).unwrap();
        if let Frame::Array(pts) = r {
            assert_eq!(pts.len(), 5);
            // First element should be the latest timestamp.
            if let Frame::Array(pair) = &pts[0] {
                assert_eq!(pair[0], Frame::Integer(4000));
            }
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_ts_mget_with_labels() {
        let s = store();
        let c1 = cmd("TS.CREATE", &["s1", "LABELS", "sensor", "temp", "room", "hall"]);
        handle_ts_create(&s, &c1).unwrap();
        handle_ts_add(&s, &cmd("TS.ADD", &["s1", "1000", "22.0"])).unwrap();

        let c2 = cmd("TS.CREATE", &["s2", "LABELS", "sensor", "humidity"]);
        handle_ts_create(&s, &c2).unwrap();
        handle_ts_add(&s, &cmd("TS.ADD", &["s2", "1000", "55.0"])).unwrap();

        let r = handle_ts_mget(&s, &cmd("TS.MGET", &["FILTER", "sensor=temp"])).unwrap();
        if let Frame::Array(rows) = r {
            assert_eq!(rows.len(), 1);
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_ts_queryindex() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["k1", "LABELS", "env", "prod"])).unwrap();
        handle_ts_create(&s, &cmd("TS.CREATE", &["k2", "LABELS", "env", "dev"])).unwrap();
        handle_ts_create(&s, &cmd("TS.CREATE", &["k3", "LABELS", "env", "prod"])).unwrap();

        let r = handle_ts_queryindex(&s, &cmd("TS.QUERYINDEX", &["env=prod"])).unwrap();
        if let Frame::Array(keys) = r {
            assert_eq!(keys.len(), 2);
        } else {
            panic!("expected array");
        }
    }

    #[test]
    fn test_ts_createrule_and_deleterule() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["src"])).unwrap();
        handle_ts_create(&s, &cmd("TS.CREATE", &["dst"])).unwrap();

        let r = handle_ts_createrule(
            &s,
            &cmd("TS.CREATERULE", &["src", "dst", "AGGREGATION", "avg", "60000"]),
        )
        .unwrap();
        assert!(matches!(r, Frame::SimpleString(_)));

        let r2 = handle_ts_deleterule(&s, &cmd("TS.DELETERULE", &["src", "dst"])).unwrap();
        assert!(matches!(r2, Frame::SimpleString(_)));
    }

    #[test]
    fn test_ts_del_series() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["del_me"])).unwrap();
        handle_ts_add(&s, &cmd("TS.ADD", &["del_me", "1000", "1.0"])).unwrap();
        handle_ts_del(&s, &cmd("TS.DEL", &["del_me"])).unwrap();
        let r = handle_ts_info(&s, &cmd("TS.INFO", &["del_me"]));
        assert!(r.is_err());
    }

    #[test]
    fn test_ts_del_range() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["dr"])).unwrap();
        for i in 0..10i64 {
            handle_ts_add(&s, &cmd("TS.ADD", &["dr", &(i * 1000).to_string(), &i.to_string()])).unwrap();
        }
        let r = handle_ts_del(&s, &cmd("TS.DEL", &["dr", "2000", "5000"])).unwrap();
        assert_eq!(r, Frame::Integer(4));
    }

    #[test]
    fn test_ts_alter() {
        let s = store();
        handle_ts_create(&s, &cmd("TS.CREATE", &["alter_me"])).unwrap();
        let r = handle_ts_alter(
            &s,
            &cmd("TS.ALTER", &["alter_me", "RETENTION", "120000"]),
        )
        .unwrap();
        assert!(matches!(r, Frame::SimpleString(_)));

        let info = handle_ts_info(&s, &cmd("TS.INFO", &["alter_me"])).unwrap();
        if let Frame::Array(items) = info {
            // Find "retentionTime" index.
            let mut i = 0;
            while i + 1 < items.len() {
                if let Frame::BulkString(ref k) = items[i] {
                    if k == "retentionTime" {
                        assert_eq!(items[i + 1], Frame::Integer(120000));
                        return;
                    }
                }
                i += 1;
            }
            panic!("retentionTime not found in TS.INFO response");
        }
    }

    #[test]
    fn test_ts_duplicate_policy_block_via_command() {
        let s = store();
        handle_ts_create(
            &s,
            &cmd("TS.CREATE", &["dup", "DUPLICATE_POLICY", "BLOCK"]),
        )
        .unwrap();
        handle_ts_add(&s, &cmd("TS.ADD", &["dup", "1000", "5.0"])).unwrap();
        // Attempt to add same timestamp → should fail.
        let r = handle_ts_add(&s, &cmd("TS.ADD", &["dup", "1000", "6.0"]));
        assert!(r.is_err(), "expected error for BLOCK duplicate policy");
    }
}
