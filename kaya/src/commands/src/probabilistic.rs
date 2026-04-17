// SPDX-License-Identifier: AGPL-3.0-or-later
//! RESP3-compatible probabilistic command handlers.
//!
//! Implements the following command families:
//!
//! **Cuckoo / BF-compat (via CuckooFilter)**
//! - `BF.RESERVE / BF.ADD / BF.MADD / BF.EXISTS / BF.MEXISTS` (Bloom-compat via Cuckoo)
//! - `CF.RESERVE / CF.ADD / CF.ADDNX / CF.EXISTS / CF.DEL / CF.COUNT / CF.MEXISTS`
//!
//! **HyperLogLog**
//! - `PFADD / PFCOUNT / PFMERGE`
//!
//! **Count-Min Sketch**
//! - `CMS.INITBYDIM / CMS.INITBYPROB / CMS.INCRBY / CMS.QUERY / CMS.MERGE / CMS.INFO`
//!
//! **TopK**
//! - `TOPK.RESERVE / TOPK.ADD / TOPK.INCRBY / TOPK.QUERY / TOPK.COUNT / TOPK.LIST / TOPK.INFO`

use std::sync::Arc;

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use kaya_store::probabilistic::{ProbabilisticStore};

use crate::CommandError;

// ---------------------------------------------------------------------------
// Handler struct
// ---------------------------------------------------------------------------

/// Handler for all probabilistic data structure commands.
pub struct ProbabilisticHandler {
    pub prob: Arc<ProbabilisticStore>,
}

impl ProbabilisticHandler {
    pub fn new(prob: Arc<ProbabilisticStore>) -> Self {
        Self { prob }
    }

    // -----------------------------------------------------------------------
    // Utility
    // -----------------------------------------------------------------------

    fn require_args(&self, cmd: &Command, min: usize) -> Result<(), CommandError> {
        if cmd.arg_count() < min {
            return Err(CommandError::WrongArity {
                command: cmd.name.clone(),
            });
        }
        Ok(())
    }

    fn require_exact_args(&self, cmd: &Command, count: usize) -> Result<(), CommandError> {
        if cmd.arg_count() != count {
            return Err(CommandError::WrongArity {
                command: cmd.name.clone(),
            });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Cuckoo filter commands (CF.*)
    // -----------------------------------------------------------------------

    /// CF.RESERVE <key> <capacity>
    pub fn cf_reserve(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let capacity: u64 = cmd
            .arg_str(1)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid capacity".into()))?;
        self.prob.cf_reserve(key, capacity);
        Ok(Frame::ok())
    }

    /// CF.ADD <key> <item>
    pub fn cf_add(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let item = cmd.arg_bytes(1)?;
        let ok = self.prob.cf_add(key, item);
        Ok(Frame::Integer(if ok { 1 } else { 0 }))
    }

    /// CF.ADDNX <key> <item>
    pub fn cf_addnx(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let item = cmd.arg_bytes(1)?;
        let added = self.prob.cf_addnx(key, item);
        Ok(Frame::Integer(if added { 1 } else { 0 }))
    }

    /// CF.EXISTS <key> <item>
    pub fn cf_exists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let item = cmd.arg_bytes(1)?;
        let exists = self.prob.cf_exists(key, item);
        Ok(Frame::Integer(if exists { 1 } else { 0 }))
    }

    /// CF.DEL <key> <item>
    pub fn cf_del(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let item = cmd.arg_bytes(1)?;
        let removed = self.prob.cf_del(key, item);
        Ok(Frame::Integer(if removed { 1 } else { 0 }))
    }

    /// CF.COUNT <key>
    pub fn cf_count(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let count = self.prob.cf_count(key);
        Ok(Frame::Integer(count as i64))
    }

    /// CF.MEXISTS <key> <item> [item ...]
    pub fn cf_mexists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let results = self.prob.cf_mexists(key, &items);
        let frames: Vec<Frame> = results
            .into_iter()
            .map(|b| Frame::Integer(if b { 1 } else { 0 }))
            .collect();
        Ok(Frame::Array(frames))
    }

    // -----------------------------------------------------------------------
    // BF.* commands — implemented via Cuckoo filter for deletion support
    // -----------------------------------------------------------------------

    /// BF.RESERVE <key> <error_rate> [capacity]
    pub fn bf_reserve(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        // error_rate is accepted but ignored; cuckoo filter uses 16-bit fingerprints.
        let capacity: u64 = if cmd.arg_count() > 2 {
            cmd.arg_str(2)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid capacity".into()))?
        } else {
            10_000
        };
        self.prob.cf_reserve(key, capacity);
        Ok(Frame::ok())
    }

    /// BF.ADD <key> <item>
    pub fn bf_add(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let item = cmd.arg_bytes(1)?;
        let is_new = self.prob.cf_add(key, item);
        Ok(Frame::Integer(if is_new { 1 } else { 0 }))
    }

    /// BF.MADD <key> <item> [item ...]
    pub fn bf_madd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let frames: Vec<Frame> = items
            .iter()
            .map(|item| {
                let ok = self.prob.cf_add(key, item);
                Frame::Integer(if ok { 1 } else { 0 })
            })
            .collect();
        Ok(Frame::Array(frames))
    }

    /// BF.EXISTS <key> <item>
    pub fn bf_exists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let item = cmd.arg_bytes(1)?;
        let exists = self.prob.cf_exists(key, item);
        Ok(Frame::Integer(if exists { 1 } else { 0 }))
    }

    /// BF.MEXISTS <key> <item> [item ...]
    pub fn bf_mexists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let results = self.prob.cf_mexists(key, &items);
        let frames: Vec<Frame> = results
            .into_iter()
            .map(|b| Frame::Integer(if b { 1 } else { 0 }))
            .collect();
        Ok(Frame::Array(frames))
    }

    // -----------------------------------------------------------------------
    // HyperLogLog commands (PF*)
    // -----------------------------------------------------------------------

    /// PFADD <key> <element> [element ...]
    pub fn pf_add(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let changed = self.prob.pf_add(key, &items);
        Ok(Frame::Integer(if changed { 1 } else { 0 }))
    }

    /// PFCOUNT <key> [key ...]
    pub fn pf_count(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let keys: Vec<&[u8]> = cmd.args.iter().map(|b| b.as_ref()).collect();
        let count = self.prob.pf_count(&keys);
        Ok(Frame::Integer(count as i64))
    }

    /// PFMERGE <dest> <src> [src ...]
    pub fn pf_merge(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let dest = cmd.arg_bytes(0)?;
        let srcs: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        self.prob.pf_merge(dest, &srcs);
        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // Count-Min Sketch commands (CMS.*)
    // -----------------------------------------------------------------------

    /// CMS.INITBYDIM <key> <width> <depth>
    pub fn cms_initbydim(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        let width: usize = cmd
            .arg_str(1)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid width".into()))?;
        let depth: usize = cmd
            .arg_str(2)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid depth".into()))?;
        self.prob
            .cms_initbydim(key, width, depth)
            .map_err(|e| CommandError::Syntax(e.to_string()))?;
        Ok(Frame::ok())
    }

    /// CMS.INITBYPROB <key> <epsilon> <delta>
    pub fn cms_initbyprob(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        let epsilon: f64 = cmd
            .arg_str(1)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid epsilon".into()))?;
        let delta: f64 = cmd
            .arg_str(2)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid delta".into()))?;
        self.prob.cms_initbyprob(key, epsilon, delta);
        Ok(Frame::ok())
    }

    /// CMS.INCRBY <key> <item> <count> [item count ...]
    pub fn cms_incrby(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;

        let remaining = cmd.arg_count() - 1;
        if remaining == 0 || remaining % 2 != 0 {
            return Err(CommandError::WrongArity {
                command: cmd.name.clone(),
            });
        }

        let mut pairs: Vec<(&[u8], u64)> = Vec::with_capacity(remaining / 2);
        let mut i = 1;
        while i + 1 < cmd.arg_count() {
            let item = cmd.arg_bytes(i)?;
            let count: u64 = cmd
                .arg_str(i + 1)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid count".into()))?;
            pairs.push((item, count));
            i += 2;
        }
        self.prob.cms_incrby(key, &pairs);
        Ok(Frame::ok())
    }

    /// CMS.QUERY <key> <item> [item ...]
    pub fn cms_query(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let counts = self.prob.cms_query(key, &items);
        let frames: Vec<Frame> = counts
            .into_iter()
            .map(|c| Frame::Integer(c as i64))
            .collect();
        Ok(Frame::Array(frames))
    }

    /// CMS.MERGE <dest> <numkeys> <src1> [src2 ...] [WEIGHTS w1 [w2 ...]]
    pub fn cms_merge(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let dest = cmd.arg_bytes(0)?;
        let numkeys: usize = cmd
            .arg_str(1)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid numkeys".into()))?;

        if cmd.arg_count() < 2 + numkeys {
            return Err(CommandError::WrongArity {
                command: cmd.name.clone(),
            });
        }

        let srcs: Vec<&[u8]> = (0..numkeys)
            .map(|i| cmd.args[2 + i].as_ref())
            .collect();

        // Optional WEIGHTS clause.
        let mut weights: Vec<f64> = vec![1.0; numkeys];
        let mut idx = 2 + numkeys;
        if idx < cmd.arg_count() {
            let kw = cmd.arg_str(idx)?.to_ascii_uppercase();
            if kw == "WEIGHTS" {
                idx += 1;
                for w in weights.iter_mut() {
                    if idx >= cmd.arg_count() {
                        break;
                    }
                    *w = cmd
                        .arg_str(idx)?
                        .parse()
                        .map_err(|_| CommandError::Syntax("invalid weight".into()))?;
                    idx += 1;
                }
            }
        }

        self.prob
            .cms_merge(dest, &srcs, &weights)
            .map_err(|e| CommandError::Syntax(e.to_string()))?;
        Ok(Frame::ok())
    }

    /// CMS.INFO <key>
    pub fn cms_info(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        match self.prob.cms_sketches.get(key.as_ref()) {
            Some(cms) => {
                let frames = vec![
                    Frame::BulkString(Bytes::from("width")),
                    Frame::Integer(cms.width() as i64),
                    Frame::BulkString(Bytes::from("depth")),
                    Frame::Integer(cms.depth() as i64),
                ];
                Ok(Frame::Array(frames))
            }
            None => Ok(Frame::Null),
        }
    }

    // -----------------------------------------------------------------------
    // TopK commands (TOPK.*)
    // -----------------------------------------------------------------------

    /// TOPK.RESERVE <key> <k> <width> <depth> <decay>
    pub fn topk_reserve(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 5)?;
        let key = cmd.arg_bytes(0)?;
        let k: usize = cmd
            .arg_str(1)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid k".into()))?;
        let width: usize = cmd
            .arg_str(2)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid width".into()))?;
        let depth: usize = cmd
            .arg_str(3)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid depth".into()))?;
        let decay: f64 = cmd
            .arg_str(4)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid decay".into()))?;
        self.prob.topk_reserve(key, k, width, depth, decay);
        Ok(Frame::ok())
    }

    /// TOPK.ADD <key> <item> [item ...]
    /// Returns array of evicted item names (or null for each slot without eviction).
    pub fn topk_add(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let evicted = self.prob.topk_add(key, &items);

        // Return one reply per item: evicted name if displaced, else Null.
        let mut evicted_iter = evicted.into_iter();
        let frames: Vec<Frame> = items
            .iter()
            .map(|_| match evicted_iter.next() {
                Some(s) if !s.is_empty() => Frame::BulkString(Bytes::from(s)),
                _ => Frame::Null,
            })
            .collect();
        Ok(Frame::Array(frames))
    }

    /// TOPK.INCRBY <key> <item> <count> [item count ...]
    pub fn topk_incrby(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;

        let remaining = cmd.arg_count() - 1;
        if remaining == 0 || remaining % 2 != 0 {
            return Err(CommandError::WrongArity {
                command: cmd.name.clone(),
            });
        }

        let mut pairs: Vec<(&[u8], u64)> = Vec::with_capacity(remaining / 2);
        let mut i = 1;
        while i + 1 < cmd.arg_count() {
            let item = cmd.arg_bytes(i)?;
            let count: u64 = cmd
                .arg_str(i + 1)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid count".into()))?;
            pairs.push((item, count));
            i += 2;
        }

        let evicted = self.prob.topk_incrby(key, &pairs);
        let mut evicted_iter = evicted.into_iter();
        let frames: Vec<Frame> = pairs
            .iter()
            .map(|_| match evicted_iter.next() {
                Some(s) if !s.is_empty() => Frame::BulkString(Bytes::from(s)),
                _ => Frame::Null,
            })
            .collect();
        Ok(Frame::Array(frames))
    }

    /// TOPK.QUERY <key> <item> [item ...]
    pub fn topk_query(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let results = self.prob.topk_query(key, &items);
        let frames: Vec<Frame> = results
            .into_iter()
            .map(|b| Frame::Integer(if b { 1 } else { 0 }))
            .collect();
        Ok(Frame::Array(frames))
    }

    /// TOPK.COUNT <key> <item> [item ...]
    pub fn topk_count(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let counts = self.prob.topk_count(key, &items);
        let frames: Vec<Frame> = counts
            .into_iter()
            .map(|c| Frame::Integer(c as i64))
            .collect();
        Ok(Frame::Array(frames))
    }

    /// TOPK.LIST <key>
    pub fn topk_list(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let list = self.prob.topk_list(key);
        let frames: Vec<Frame> = list
            .into_iter()
            .flat_map(|(name, count)| {
                vec![
                    Frame::BulkString(Bytes::from(name)),
                    Frame::Integer(count as i64),
                ]
            })
            .collect();
        Ok(Frame::Array(frames))
    }

    /// TOPK.INFO <key>
    pub fn topk_info(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        match self.prob.topks.get(key.as_ref()) {
            Some(entry) => {
                let topk = entry.read();
                let frames = vec![
                    Frame::BulkString(Bytes::from("k")),
                    Frame::Integer(topk.k() as i64),
                ];
                Ok(Frame::Array(frames))
            }
            None => Ok(Frame::Null),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_protocol::Command;

    fn handler() -> ProbabilisticHandler {
        ProbabilisticHandler::new(Arc::new(ProbabilisticStore::new()))
    }

    fn cmd(name: &str, args: &[&str]) -> Command {
        Command {
            name: name.to_uppercase(),
            args: args.iter().map(|s| Bytes::from(s.to_string())).collect(),
        }
    }

    // -----------------------------------------------------------------------
    // CF tests
    // -----------------------------------------------------------------------

    #[test]
    fn cf_add_then_exists_returns_1() {
        let h = handler();
        h.cf_add(&cmd("CF.ADD", &["mykey", "hello"])).unwrap();
        let res = h.cf_exists(&cmd("CF.EXISTS", &["mykey", "hello"])).unwrap();
        assert_eq!(res, Frame::Integer(1));
    }

    #[test]
    fn cf_del_then_exists_returns_0() {
        let h = handler();
        h.cf_add(&cmd("CF.ADD", &["mykey", "hello"])).unwrap();
        h.cf_del(&cmd("CF.DEL", &["mykey", "hello"])).unwrap();
        let res = h.cf_exists(&cmd("CF.EXISTS", &["mykey", "hello"])).unwrap();
        assert_eq!(res, Frame::Integer(0));
    }

    #[test]
    fn cf_mexists_batch() {
        let h = handler();
        h.cf_add(&cmd("CF.ADD", &["f", "a"])).unwrap();
        h.cf_add(&cmd("CF.ADD", &["f", "b"])).unwrap();
        let res = h.cf_mexists(&cmd("CF.MEXISTS", &["f", "a", "b", "c"])).unwrap();
        if let Frame::Array(frames) = res {
            assert_eq!(frames[0], Frame::Integer(1));
            assert_eq!(frames[1], Frame::Integer(1));
            assert_eq!(frames[2], Frame::Integer(0));
        } else {
            panic!("expected Array");
        }
    }

    #[test]
    fn cf_count_tracks_inserts() {
        let h = handler();
        h.cf_reserve(&cmd("CF.RESERVE", &["cnt", "1000"])).unwrap();
        h.cf_add(&cmd("CF.ADD", &["cnt", "x"])).unwrap();
        h.cf_add(&cmd("CF.ADD", &["cnt", "y"])).unwrap();
        let res = h.cf_count(&cmd("CF.COUNT", &["cnt"])).unwrap();
        assert_eq!(res, Frame::Integer(2));
    }

    // -----------------------------------------------------------------------
    // BF.* compatibility tests
    // -----------------------------------------------------------------------

    #[test]
    fn bf_add_then_exists() {
        let h = handler();
        let res = h.bf_add(&cmd("BF.ADD", &["bloom", "item1"])).unwrap();
        assert_eq!(res, Frame::Integer(1));
        let res = h.bf_exists(&cmd("BF.EXISTS", &["bloom", "item1"])).unwrap();
        assert_eq!(res, Frame::Integer(1));
    }

    #[test]
    fn bf_madd_and_mexists() {
        let h = handler();
        h.bf_madd(&cmd("BF.MADD", &["bf", "a", "b", "c"])).unwrap();
        let res = h.bf_mexists(&cmd("BF.MEXISTS", &["bf", "a", "d"])).unwrap();
        if let Frame::Array(frames) = res {
            assert_eq!(frames[0], Frame::Integer(1));
            assert_eq!(frames[1], Frame::Integer(0));
        } else {
            panic!("expected Array");
        }
    }

    // -----------------------------------------------------------------------
    // HLL tests
    // -----------------------------------------------------------------------

    #[test]
    fn pfadd_three_items_pfcount_approx_3() {
        let h = handler();
        h.pf_add(&cmd("PFADD", &["hll", "a", "b", "c"])).unwrap();
        let res = h.pf_count(&cmd("PFCOUNT", &["hll"])).unwrap();
        if let Frame::Integer(n) = res {
            assert!((n - 3).abs() <= 1, "count={n}");
        } else {
            panic!("expected Integer");
        }
    }

    #[test]
    fn pfmerge_union_correct() {
        let h = handler();
        h.pf_add(&cmd("PFADD", &["s1", "a", "b"])).unwrap();
        h.pf_add(&cmd("PFADD", &["s2", "c", "d"])).unwrap();
        h.pf_merge(&cmd("PFMERGE", &["dest", "s1", "s2"])).unwrap();
        let res = h.pf_count(&cmd("PFCOUNT", &["dest"])).unwrap();
        if let Frame::Integer(n) = res {
            assert!((n - 4).abs() <= 1, "merged count={n}");
        } else {
            panic!("expected Integer");
        }
    }

    #[test]
    fn pfadd_1000_items_within_5_percent() {
        let h = handler();
        let items: Vec<String> = (0..1000u32).map(|i| i.to_string()).collect();
        let mut args = vec!["hll1000"];
        args.extend(items.iter().map(|s| s.as_str()));
        h.pf_add(&cmd("PFADD", &args)).unwrap();
        let res = h.pf_count(&cmd("PFCOUNT", &["hll1000"])).unwrap();
        if let Frame::Integer(n) = res {
            let err = (n as f64 - 1000.0).abs() / 1000.0;
            assert!(err < 0.05, "HLL error {err:.3} > 5%, count={n}");
        } else {
            panic!("expected Integer");
        }
    }

    // -----------------------------------------------------------------------
    // CMS tests
    // -----------------------------------------------------------------------

    #[test]
    fn cms_incrby_then_query() {
        let h = handler();
        h.cms_initbydim(&cmd("CMS.INITBYDIM", &["cms1", "2048", "5"])).unwrap();
        h.cms_incrby(&cmd("CMS.INCRBY", &["cms1", "item", "100"])).unwrap();
        let res = h.cms_query(&cmd("CMS.QUERY", &["cms1", "item"])).unwrap();
        if let Frame::Array(frames) = res {
            if let Frame::Integer(c) = frames[0] {
                assert!(c >= 100, "expected >= 100, got {c}");
            } else {
                panic!("expected Integer");
            }
        } else {
            panic!("expected Array");
        }
    }

    #[test]
    fn cms_merge_sums_counts() {
        let h = handler();
        h.cms_initbydim(&cmd("CMS.INITBYDIM", &["ma", "512", "4"])).unwrap();
        h.cms_initbydim(&cmd("CMS.INITBYDIM", &["mb", "512", "4"])).unwrap();
        h.cms_incrby(&cmd("CMS.INCRBY", &["ma", "key", "10"])).unwrap();
        h.cms_incrby(&cmd("CMS.INCRBY", &["mb", "key", "20"])).unwrap();
        h.cms_merge(&cmd("CMS.MERGE", &["ma", "1", "mb"])).unwrap();
        let res = h.cms_query(&cmd("CMS.QUERY", &["ma", "key"])).unwrap();
        if let Frame::Array(frames) = res {
            if let Frame::Integer(c) = frames[0] {
                assert!(c >= 30, "expected >= 30, got {c}");
            } else {
                panic!("expected Integer");
            }
        } else {
            panic!("expected Array");
        }
    }

    // -----------------------------------------------------------------------
    // TopK tests
    // -----------------------------------------------------------------------

    #[test]
    fn topk_add_1000_zipf_top10_correct() {
        let h = handler();
        h.topk_reserve(&cmd("TOPK.RESERVE", &["tk", "10", "1024", "5", "0.9"])).unwrap();

        // Build pairs: item_0=1000, item_1=500, ..., item_19=~50
        let items_with_counts: Vec<(String, u64)> = (0..20u64)
            .map(|i| (format!("item_{i}"), 1000u64 / (i + 1)))
            .collect();

        for (item, count) in &items_with_counts {
            h.topk_incrby(&cmd(
                "TOPK.INCRBY",
                &["tk", item.as_str(), count.to_string().as_str()],
            ))
            .unwrap();
        }

        let list_res = h.topk_list(&cmd("TOPK.LIST", &["tk"])).unwrap();
        if let Frame::Array(frames) = list_res {
            // List is flat: [item, count, item, count, ...]
            assert!(!frames.is_empty());
            // The first item name should be item_0.
            if let Frame::BulkString(name) = &frames[0] {
                assert_eq!(
                    name,
                    &Bytes::from("item_0"),
                    "expected item_0 at top, got {:?}",
                    name
                );
            }
            // At most 10 items (20 fields for name+count pairs).
            assert!(
                frames.len() <= 20,
                "expected at most 20 flat frames (10 items * 2), got {}",
                frames.len()
            );
        } else {
            panic!("expected Array from TOPK.LIST");
        }
    }

    #[test]
    fn topk_query_frequent_item_in_top() {
        let h = handler();
        h.topk_reserve(&cmd("TOPK.RESERVE", &["q2", "5", "256", "4", "0.9"])).unwrap();
        for _ in 0..300 {
            h.topk_add(&cmd("TOPK.ADD", &["q2", "hot"])).unwrap();
        }
        let res = h.topk_query(&cmd("TOPK.QUERY", &["q2", "hot", "cold"])).unwrap();
        if let Frame::Array(frames) = res {
            assert_eq!(frames[0], Frame::Integer(1), "hot should be in top-k");
            assert_eq!(frames[1], Frame::Integer(0), "cold should not be in top-k");
        } else {
            panic!("expected Array");
        }
    }

    #[test]
    fn store_api_round_trip() {
        // Verify ProbabilisticStore methods work correctly end-to-end.
        let ps = ProbabilisticStore::new();

        // Cuckoo round-trip
        ps.cf_reserve(b"rt_cf", 500);
        assert!(ps.cf_add(b"rt_cf", b"kaya"));
        assert!(ps.cf_exists(b"rt_cf", b"kaya"));
        assert!(ps.cf_del(b"rt_cf", b"kaya"));
        assert!(!ps.cf_exists(b"rt_cf", b"kaya"));

        // HLL round-trip
        ps.pf_add(b"rt_hll", &[b"a" as &[u8], b"b", b"c"]);
        let c = ps.pf_count(&[b"rt_hll"]);
        assert!((c as i64 - 3).abs() <= 1);

        // CMS round-trip
        ps.cms_initbydim(b"rt_cms", 512, 4).unwrap();
        ps.cms_incrby(b"rt_cms", &[(b"item" as &[u8], 7)]);
        let q = ps.cms_query(b"rt_cms", &[b"item"]);
        assert!(q[0] >= 7);

        // TopK round-trip
        ps.topk_reserve(b"rt_topk", 3, 64, 3, 0.9);
        ps.topk_add(b"rt_topk", &[b"hot"]);
        let in_top = ps.topk_query(b"rt_topk", &[b"hot"]);
        assert!(in_top[0]);
    }
}
