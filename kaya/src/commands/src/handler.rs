//! Command handler implementations for all supported KAYA commands.

use std::sync::Arc;

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use kaya_scripting::ScriptResult;
use kaya_streams::StreamId;
use tracing::debug;

use crate::{CommandContext, CommandError};

/// Handles individual command execution against the store.
pub struct CommandHandler {
    ctx: Arc<CommandContext>,
}

impl CommandHandler {
    pub fn new(ctx: Arc<CommandContext>) -> Self {
        Self { ctx }
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
    // Basic commands
    // -----------------------------------------------------------------------

    pub fn ping(&self, cmd: &Command) -> Result<Frame, CommandError> {
        if cmd.arg_count() > 0 {
            let msg = cmd.arg_bytes(0)?;
            Ok(Frame::BulkString(msg.clone()))
        } else {
            Ok(Frame::SimpleString("PONG".into()))
        }
    }

    pub fn echo(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let msg = cmd.arg_bytes(0)?;
        Ok(Frame::BulkString(msg.clone()))
    }

    pub fn get(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        match self.ctx.store.get(key)? {
            Some(val) => Ok(Frame::BulkString(val)),
            None => Ok(Frame::Null),
        }
    }

    pub fn set(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let value = cmd.arg_bytes(1)?;

        // Parse optional EX/PX/EXAT/PXAT arguments.
        let mut ttl: Option<u64> = None;
        let mut i = 2;
        while i < cmd.arg_count() {
            let opt = cmd.arg_str(i)?.to_ascii_uppercase();
            match opt.as_str() {
                "EX" => {
                    i += 1;
                    ttl = Some(cmd.arg_i64(i)? as u64);
                }
                "PX" => {
                    i += 1;
                    let ms = cmd.arg_i64(i)? as u64;
                    ttl = Some(ms / 1000);
                }
                "NX" | "XX" | "KEEPTTL" | "GET" => {
                    // Recognized but not fully implemented yet.
                }
                _ => {
                    return Err(CommandError::Syntax(format!(
                        "unsupported SET option: {opt}"
                    )));
                }
            }
            i += 1;
        }

        self.ctx.store.set(key, value, ttl)?;
        Ok(Frame::ok())
    }

    pub fn del(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let keys: Vec<&[u8]> = cmd.args.iter().map(|b| b.as_ref()).collect();
        let count = self.ctx.store.del(&keys);
        Ok(Frame::Integer(count as i64))
    }

    pub fn mget(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let keys: Vec<&[u8]> = cmd.args.iter().map(|b| b.as_ref()).collect();
        let values = self.ctx.store.mget(&keys);
        let frames: Vec<Frame> = values
            .into_iter()
            .map(|opt| match opt {
                Some(v) => Frame::BulkString(v),
                None => Frame::Null,
            })
            .collect();
        Ok(Frame::Array(frames))
    }

    pub fn mset(&self, cmd: &Command) -> Result<Frame, CommandError> {
        if cmd.arg_count() < 2 || cmd.arg_count() % 2 != 0 {
            return Err(CommandError::WrongArity {
                command: cmd.name.clone(),
            });
        }
        let pairs: Vec<(&[u8], &[u8])> = cmd
            .args
            .chunks(2)
            .map(|c| (c[0].as_ref(), c[1].as_ref()))
            .collect();
        self.ctx.store.mset(&pairs)?;
        Ok(Frame::ok())
    }

    pub fn exists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let keys: Vec<&[u8]> = cmd.args.iter().map(|b| b.as_ref()).collect();
        let count = self.ctx.store.exists(&keys);
        Ok(Frame::Integer(count as i64))
    }

    pub fn expire(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let seconds = cmd.arg_i64(1)? as u64;
        let result = self.ctx.store.expire(key, seconds);
        Ok(Frame::Integer(if result { 1 } else { 0 }))
    }

    pub fn ttl(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let ttl = self.ctx.store.ttl(key);
        Ok(Frame::Integer(ttl))
    }

    pub fn persist(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let result = self.ctx.store.persist(key);
        Ok(Frame::Integer(if result { 1 } else { 0 }))
    }

    pub fn incr(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let val = self.ctx.store.incr(key)?;
        Ok(Frame::Integer(val))
    }

    pub fn decr(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let val = self.ctx.store.decr(key)?;
        Ok(Frame::Integer(val))
    }

    pub fn incrby(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let delta = cmd.arg_i64(1)?;
        let val = self.ctx.store.incr_by(key, delta)?;
        Ok(Frame::Integer(val))
    }

    pub fn decrby(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let delta = cmd.arg_i64(1)?;
        let val = self.ctx.store.incr_by(key, -delta)?;
        Ok(Frame::Integer(val))
    }

    pub fn dbsize(&self, _cmd: &Command) -> Result<Frame, CommandError> {
        Ok(Frame::Integer(self.ctx.store.key_count() as i64))
    }

    pub fn flushdb(&self, _cmd: &Command) -> Result<Frame, CommandError> {
        self.ctx.store.flush();
        Ok(Frame::ok())
    }

    pub fn info(&self, _cmd: &Command) -> Result<Frame, CommandError> {
        let info = format!(
            "# Server\r\nredis_version:7.0.0\r\nkaya_version:0.1.0\r\nredis_mode:standalone\r\ntcp_port:6380\r\nuptime_in_seconds:{}\r\n\r\n# Keyspace\r\ndb0:keys={}\r\n",
            self.ctx.store.uptime_secs(),
            self.ctx.store.key_count(),
        );
        Ok(Frame::BulkString(Bytes::from(info)))
    }

    pub fn command_info(&self, _cmd: &Command) -> Result<Frame, CommandError> {
        // Minimal implementation: return empty array.
        Ok(Frame::Array(vec![]))
    }

    // -----------------------------------------------------------------------
    // Set commands
    // -----------------------------------------------------------------------

    pub fn sadd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let members: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let count = self.ctx.store.sadd(key, &members)?;
        Ok(Frame::Integer(count as i64))
    }

    pub fn sismember(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let member = cmd.arg_bytes(1)?;
        let exists = self.ctx.store.sismember(key, member);
        Ok(Frame::Integer(if exists { 1 } else { 0 }))
    }

    pub fn smembers(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let members = self.ctx.store.smembers(key);
        let frames: Vec<Frame> = members
            .into_iter()
            .map(Frame::BulkString)
            .collect();
        Ok(Frame::Array(frames))
    }

    pub fn srem(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let members: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let count = self.ctx.store.srem(key, &members);
        Ok(Frame::Integer(count as i64))
    }

    pub fn scard(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let count = self.ctx.store.scard(key);
        Ok(Frame::Integer(count as i64))
    }

    // -----------------------------------------------------------------------
    // Sorted set commands
    // -----------------------------------------------------------------------

    pub fn zadd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // ZADD key [NX|XX|GT|LT] [CH] score member [score member ...]
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;

        // Simple parsing: skip options, find score-member pairs
        let mut idx = 1;

        // Skip optional flags (NX, XX, GT, LT, CH)
        while idx < cmd.arg_count() {
            let arg = cmd.arg_str(idx)?.to_ascii_uppercase();
            match arg.as_str() {
                "NX" | "XX" | "GT" | "LT" | "CH" => idx += 1,
                _ => break,
            }
        }

        let remaining = cmd.arg_count() - idx;
        if remaining < 2 || remaining % 2 != 0 {
            return Err(CommandError::WrongArity {
                command: "ZADD".into(),
            });
        }

        let mut members = Vec::new();
        while idx + 1 < cmd.arg_count() {
            let score_str = cmd.arg_str(idx)?;
            let score: f64 = score_str
                .parse()
                .map_err(|_| CommandError::Syntax("invalid score".into()))?;
            let member = cmd.arg_bytes(idx + 1)?;
            members.push((score, member.as_ref()));
            idx += 2;
        }

        let count = self.ctx.store.zadd(key, &members);
        Ok(Frame::Integer(count as i64))
    }

    pub fn zrem(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let members: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let count = self.ctx.store.zrem(key, &members);
        Ok(Frame::Integer(count as i64))
    }

    pub fn zscore(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let key = cmd.arg_bytes(0)?;
        let member = cmd.arg_bytes(1)?;
        match self.ctx.store.zscore(key, member) {
            Some(score) => Ok(Frame::BulkString(Bytes::from(score.to_string()))),
            None => Ok(Frame::Null),
        }
    }

    pub fn zcard(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;
        let count = self.ctx.store.zcard(key);
        Ok(Frame::Integer(count as i64))
    }

    pub fn zrange(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        let start = cmd.arg_i64(1)?;
        let stop = cmd.arg_i64(2)?;

        let withscores = cmd.arg_count() > 3
            && cmd
                .arg_str(3)
                .map(|s| s.to_ascii_uppercase() == "WITHSCORES")
                .unwrap_or(false);

        let results = self.ctx.store.zrange(key, start, stop);

        if withscores {
            let frames: Vec<Frame> = results
                .into_iter()
                .flat_map(|(score, member)| {
                    vec![
                        Frame::BulkString(member),
                        Frame::BulkString(Bytes::from(score.to_string())),
                    ]
                })
                .collect();
            Ok(Frame::Array(frames))
        } else {
            let frames: Vec<Frame> = results
                .into_iter()
                .map(|(_, member)| Frame::BulkString(member))
                .collect();
            Ok(Frame::Array(frames))
        }
    }

    pub fn zrangebyscore(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        let min_str = cmd.arg_str(1)?;
        let max_str = cmd.arg_str(2)?;

        let min: f64 = if min_str == "-inf" {
            f64::NEG_INFINITY
        } else {
            min_str
                .parse()
                .map_err(|_| CommandError::Syntax("invalid min score".into()))?
        };
        let max: f64 = if max_str == "+inf" {
            f64::INFINITY
        } else {
            max_str
                .parse()
                .map_err(|_| CommandError::Syntax("invalid max score".into()))?
        };

        let mut withscores = false;
        let mut limit: Option<usize> = None;
        let mut i = 3;
        while i < cmd.arg_count() {
            let arg = cmd.arg_str(i)?.to_ascii_uppercase();
            match arg.as_str() {
                "WITHSCORES" => withscores = true,
                "LIMIT" => {
                    i += 1; // skip offset (we ignore offset for simplicity)
                    i += 1;
                    limit = Some(cmd.arg_i64(i)? as usize);
                }
                _ => {}
            }
            i += 1;
        }

        let results = self.ctx.store.zrangebyscore(key, min, max, limit);

        if withscores {
            let frames: Vec<Frame> = results
                .into_iter()
                .flat_map(|(score, member)| {
                    vec![
                        Frame::BulkString(member),
                        Frame::BulkString(Bytes::from(score.to_string())),
                    ]
                })
                .collect();
            Ok(Frame::Array(frames))
        } else {
            let frames: Vec<Frame> = results
                .into_iter()
                .map(|(_, member)| Frame::BulkString(member))
                .collect();
            Ok(Frame::Array(frames))
        }
    }

    // -----------------------------------------------------------------------
    // ZREVRANGE key start stop [WITHSCORES]
    //
    // Legacy command (Redis 6.2+ deprecated). Delegates to the REV path of
    // the underlying sorted-set store — zero extra allocation.
    // -----------------------------------------------------------------------

    /// ZREVRANGE key start stop [WITHSCORES]
    ///
    /// Returns the members in descending score order for the index range
    /// [start, stop]. Equivalent to `ZRANGE key start stop REV BYINDEX`.
    pub fn zrevrange(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        let start = cmd.arg_i64(1)?;
        let stop = cmd.arg_i64(2)?;

        let withscores = cmd.arg_count() > 3
            && cmd
                .arg_str(3)
                .map(|s| s.eq_ignore_ascii_case("WITHSCORES"))
                .unwrap_or(false);

        debug!(key = ?key, start, stop, withscores, "ZREVRANGE");

        let results = self.ctx.store.zrevrange(key, start, stop);

        if withscores {
            let frames: Vec<Frame> = results
                .into_iter()
                .flat_map(|(score, member)| {
                    [
                        Frame::BulkString(member),
                        Frame::BulkString(Bytes::from(score.to_string())),
                    ]
                })
                .collect();
            Ok(Frame::Array(frames))
        } else {
            let frames: Vec<Frame> = results
                .into_iter()
                .map(|(_, member)| Frame::BulkString(member))
                .collect();
            Ok(Frame::Array(frames))
        }
    }

    // -----------------------------------------------------------------------
    // ZREVRANGEBYSCORE key max min [WITHSCORES] [LIMIT offset count]
    //
    // Legacy command (Redis 6.2+ deprecated). Note: argument order is max THEN
    // min (opposite of ZRANGEBYSCORE). Delegates REV path — zero extra alloc.
    // -----------------------------------------------------------------------

    /// ZREVRANGEBYSCORE key max min [WITHSCORES] [LIMIT offset count]
    ///
    /// Returns members in descending score order where score is in [min, max].
    /// Equivalent to `ZRANGE key max min BYSCORE REV [LIMIT offset count]`.
    pub fn zrevrangebyscore(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        // NOTE: legacy ZREVRANGEBYSCORE has max before min.
        let max_str = cmd.arg_str(1)?;
        let min_str = cmd.arg_str(2)?;

        let max: f64 = if max_str.eq_ignore_ascii_case("+inf") {
            f64::INFINITY
        } else if max_str.eq_ignore_ascii_case("-inf") {
            f64::NEG_INFINITY
        } else {
            max_str
                .parse()
                .map_err(|_| CommandError::Syntax("invalid max score".into()))?
        };
        let min: f64 = if min_str.eq_ignore_ascii_case("-inf") {
            f64::NEG_INFINITY
        } else if min_str.eq_ignore_ascii_case("+inf") {
            f64::INFINITY
        } else {
            min_str
                .parse()
                .map_err(|_| CommandError::Syntax("invalid min score".into()))?
        };

        let mut withscores = false;
        let mut offset: usize = 0;
        let mut limit: Option<usize> = None;
        let mut i = 3;
        while i < cmd.arg_count() {
            let arg = cmd.arg_str(i)?.to_ascii_uppercase();
            match arg.as_str() {
                "WITHSCORES" => withscores = true,
                "LIMIT" => {
                    i += 1;
                    offset = cmd.arg_i64(i)? as usize;
                    i += 1;
                    limit = Some(cmd.arg_i64(i)? as usize);
                }
                _ => {}
            }
            i += 1;
        }

        debug!(key = ?key, min, max, offset, ?limit, withscores, "ZREVRANGEBYSCORE");

        let results = self.ctx.store.zrevrangebyscore(key, min, max, offset, limit);

        if withscores {
            let frames: Vec<Frame> = results
                .into_iter()
                .flat_map(|(score, member)| {
                    [
                        Frame::BulkString(member),
                        Frame::BulkString(Bytes::from(score.to_string())),
                    ]
                })
                .collect();
            Ok(Frame::Array(frames))
        } else {
            let frames: Vec<Frame> = results
                .into_iter()
                .map(|(_, member)| Frame::BulkString(member))
                .collect();
            Ok(Frame::Array(frames))
        }
    }

    // -----------------------------------------------------------------------
    // ZREVRANGEBYLEX key max min [LIMIT offset count]
    //
    // Legacy command (Redis 6.2+ deprecated). Arguments: max lex THEN min lex
    // (opposite of ZRANGEBYLEX). Delegates REV lex path — zero extra alloc.
    // -----------------------------------------------------------------------

    /// ZREVRANGEBYLEX key max min [LIMIT offset count]
    ///
    /// Returns members lexicographically in the range [min, max], descending.
    /// Equivalent to `ZRANGE key max min BYLEX REV [LIMIT offset count]`.
    pub fn zrevrangebylex(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let key = cmd.arg_bytes(0)?;
        // NOTE: legacy ZREVRANGEBYLEX has max before min.
        let max_lex = cmd.arg_bytes(1)?;
        let min_lex = cmd.arg_bytes(2)?;

        let mut offset: usize = 0;
        let mut limit: Option<usize> = None;
        let mut i = 3;
        while i < cmd.arg_count() {
            let arg = cmd.arg_str(i)?.to_ascii_uppercase();
            if arg == "LIMIT" {
                i += 1;
                offset = cmd.arg_i64(i)? as usize;
                i += 1;
                limit = Some(cmd.arg_i64(i)? as usize);
            }
            i += 1;
        }

        debug!(key = ?key, offset, ?limit, "ZREVRANGEBYLEX");

        let results = self.ctx.store.zrevrangebylex(
            key,
            max_lex.as_ref(),
            min_lex.as_ref(),
            offset,
            limit,
        );

        let frames: Vec<Frame> = results
            .into_iter()
            .map(Frame::BulkString)
            .collect();
        Ok(Frame::Array(frames))
    }

    // -----------------------------------------------------------------------
    // AUTH command
    // -----------------------------------------------------------------------

    pub fn auth(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let password = cmd.arg_str(0)?;

        match &self.ctx.password {
            None => {
                // No password configured -- AUTH is not needed
                Ok(Frame::err("ERR Client sent AUTH, but no password is set"))
            }
            Some(expected) => {
                if password == expected {
                    Ok(Frame::ok())
                } else {
                    Ok(Frame::err("ERR invalid password"))
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // CONFIG command (minimal)
    // -----------------------------------------------------------------------

    pub fn config(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let subcmd = cmd.arg_str(0)?.to_ascii_uppercase();

        match subcmd.as_str() {
            "GET" => {
                // Return empty map for unknown configs -- enough for client handshake
                self.require_args(cmd, 2)?;
                let param = cmd.arg_str(1)?;

                let (key, value) = match param {
                    "save" | "appendonly" => (param.to_string(), "".to_string()),
                    "databases" => ("databases".to_string(), "16".to_string()),
                    _ => (param.to_string(), "".to_string()),
                };

                Ok(Frame::Array(vec![
                    Frame::BulkString(Bytes::from(key)),
                    Frame::BulkString(Bytes::from(value)),
                ]))
            }
            "SET" => Ok(Frame::ok()),
            "RESETSTAT" => Ok(Frame::ok()),
            _ => Err(CommandError::Syntax(format!(
                "unknown CONFIG subcommand: {subcmd}"
            ))),
        }
    }

    // -----------------------------------------------------------------------
    // TYPE command
    // -----------------------------------------------------------------------

    pub fn type_cmd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let key = cmd.arg_bytes(0)?;

        // Check string/KV
        if self.ctx.store.get(key).ok().flatten().is_some() {
            return Ok(Frame::SimpleString("string".into()));
        }
        // Check set
        if self.ctx.store.scard(key) > 0 {
            return Ok(Frame::SimpleString("set".into()));
        }
        // Check sorted set
        if self.ctx.store.zcard(key) > 0 {
            return Ok(Frame::SimpleString("zset".into()));
        }

        Ok(Frame::SimpleString("none".into()))
    }

    // -----------------------------------------------------------------------
    // CLIENT command (minimal)
    // -----------------------------------------------------------------------

    pub fn client_cmd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        if cmd.arg_count() == 0 {
            return Ok(Frame::ok());
        }
        let subcmd = cmd.arg_str(0)?.to_ascii_uppercase();
        match subcmd.as_str() {
            "SETNAME" => Ok(Frame::ok()),
            "GETNAME" => Ok(Frame::Null),
            "ID" => Ok(Frame::Integer(1)),
            "INFO" => Ok(Frame::BulkString(Bytes::from("id=1 addr=127.0.0.1:0 fd=0 name= db=0\r\n"))),
            "LIST" => Ok(Frame::BulkString(Bytes::from("id=1 addr=127.0.0.1:0 fd=0 name= db=0\r\n"))),
            _ => Ok(Frame::ok()),
        }
    }

    // -----------------------------------------------------------------------
    // HELLO command (RESP3 protocol negotiation)
    //
    // NOTE: In production the HELLO command is intercepted at the network
    // layer (Connection::handle_hello) so it can mutate per-connection state.
    // This handler exists as a fallback for unit tests / the synchronous
    // execute path. It honours the requested protocol version.
    // -----------------------------------------------------------------------

    pub fn hello(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // Determine requested protocol version from first arg.
        let proto_version = match cmd.args.first().map(|b| b.as_ref()) {
            Some(b"3") => 3i64,
            Some(b"2") | None => 2i64,
            Some(other) => {
                let s = String::from_utf8_lossy(other);
                return Ok(Frame::err(format!(
                    "NOPROTO sorry, this protocol version is not supported: {s}"
                )));
            }
        };

        if proto_version == 3 {
            Ok(Frame::Map(vec![
                (
                    Frame::BulkString(Bytes::from_static(b"server")),
                    Frame::BulkString(Bytes::from_static(b"kaya")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"version")),
                    Frame::BulkString(Bytes::from_static(b"0.1.0")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"proto")),
                    Frame::Integer(3),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"id")),
                    Frame::Integer(1),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"mode")),
                    Frame::BulkString(Bytes::from_static(b"standalone")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"role")),
                    Frame::BulkString(Bytes::from_static(b"master")),
                ),
                (
                    Frame::BulkString(Bytes::from_static(b"modules")),
                    Frame::Array(vec![]),
                ),
            ]))
        } else {
            Ok(Frame::Array(vec![
                Frame::BulkString(Bytes::from_static(b"server")),
                Frame::BulkString(Bytes::from_static(b"kaya")),
                Frame::BulkString(Bytes::from_static(b"version")),
                Frame::BulkString(Bytes::from_static(b"0.1.0")),
                Frame::BulkString(Bytes::from_static(b"proto")),
                Frame::Integer(2),
                Frame::BulkString(Bytes::from_static(b"id")),
                Frame::Integer(1),
                Frame::BulkString(Bytes::from_static(b"mode")),
                Frame::BulkString(Bytes::from_static(b"standalone")),
                Frame::BulkString(Bytes::from_static(b"role")),
                Frame::BulkString(Bytes::from_static(b"master")),
                Frame::BulkString(Bytes::from_static(b"modules")),
                Frame::Array(vec![]),
            ]))
        }
    }

    // -----------------------------------------------------------------------
    // SELECT command (only db 0)
    // -----------------------------------------------------------------------

    pub fn select_cmd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let db = cmd.arg_i64(0)?;
        if db != 0 {
            return Ok(Frame::err("ERR DB index is out of range"));
        }
        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // KEYS command (pattern matching, simple implementation)
    // -----------------------------------------------------------------------

    pub fn keys_cmd(&self, _cmd: &Command) -> Result<Frame, CommandError> {
        // For now return empty array -- full KEYS * is dangerous anyway
        Ok(Frame::Array(vec![]))
    }

    // -----------------------------------------------------------------------
    // Scripting commands: EVAL, EVALSHA, SCRIPT
    // -----------------------------------------------------------------------

    pub fn eval(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let script = cmd.arg_str(0)?;
        let numkeys = cmd.arg_i64(1)? as usize;

        let engine = self
            .ctx
            .scripting
            .as_ref()
            .ok_or_else(|| CommandError::Script("scripting not enabled".into()))?;

        let mut keys = Vec::with_capacity(numkeys);
        let mut args = Vec::new();

        for i in 0..numkeys {
            keys.push(cmd.arg_str(2 + i)?.to_string());
        }
        for i in (2 + numkeys)..cmd.arg_count() {
            args.push(cmd.arg_str(i)?.to_string());
        }

        let result = engine
            .eval(script, &keys, &args)
            .map_err(|e| CommandError::Script(e.to_string()))?;

        Ok(script_result_to_frame(result))
    }

    pub fn evalsha(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let sha = cmd.arg_str(0)?;
        let numkeys = cmd.arg_i64(1)? as usize;

        let engine = self
            .ctx
            .scripting
            .as_ref()
            .ok_or_else(|| CommandError::Script("scripting not enabled".into()))?;

        let mut keys = Vec::with_capacity(numkeys);
        let mut args = Vec::new();

        for i in 0..numkeys {
            keys.push(cmd.arg_str(2 + i)?.to_string());
        }
        for i in (2 + numkeys)..cmd.arg_count() {
            args.push(cmd.arg_str(i)?.to_string());
        }

        let result = engine
            .eval_sha(sha, &keys, &args)
            .map_err(|e| CommandError::Script(e.to_string()))?;

        Ok(script_result_to_frame(result))
    }

    pub fn script(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 1)?;
        let subcmd = cmd.arg_str(0)?.to_ascii_uppercase();

        let engine = self
            .ctx
            .scripting
            .as_ref()
            .ok_or_else(|| CommandError::Script("scripting not enabled".into()))?;

        match subcmd.as_str() {
            "LOAD" => {
                self.require_args(cmd, 2)?;
                let script_src = cmd.arg_str(1)?;
                let sha = engine
                    .load(script_src)
                    .map_err(|e| CommandError::Script(e.to_string()))?;
                Ok(Frame::BulkString(Bytes::from(sha)))
            }
            "EXISTS" => {
                // Not fully implemented -- return 0 for all
                let mut results = Vec::new();
                for _ in 1..cmd.arg_count() {
                    results.push(Frame::Integer(0));
                }
                Ok(Frame::Array(results))
            }
            "FLUSH" => Ok(Frame::ok()),
            _ => Err(CommandError::Syntax(format!(
                "unknown SCRIPT subcommand: {subcmd}"
            ))),
        }
    }

    // -----------------------------------------------------------------------
    // Bloom filter commands
    // -----------------------------------------------------------------------

    pub fn bf_reserve(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let name = cmd.arg_str(0)?;
        let fp_rate: f64 = cmd
            .arg_str(1)?
            .parse()
            .map_err(|_| CommandError::Syntax("invalid fp_rate".into()))?;
        let capacity: usize = if cmd.arg_count() > 2 {
            cmd.arg_str(2)?
                .parse()
                .map_err(|_| CommandError::Syntax("invalid capacity".into()))?
        } else {
            10_000
        };
        self.ctx.blooms.reserve(name, capacity, fp_rate);
        Ok(Frame::ok())
    }

    pub fn bf_add(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let name = cmd.arg_str(0)?;
        let item = cmd.arg_bytes(1)?;
        let is_new = self.ctx.blooms.add(name, item);
        Ok(Frame::Integer(if is_new { 1 } else { 0 }))
    }

    pub fn bf_exists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 2)?;
        let name = cmd.arg_str(0)?;
        let item = cmd.arg_bytes(1)?;
        let exists = self.ctx.blooms.exists(name, item);
        Ok(Frame::Integer(if exists { 1 } else { 0 }))
    }

    pub fn bf_madd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let name = cmd.arg_str(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let results = self.ctx.blooms.madd(name, &items);
        let frames: Vec<Frame> = results
            .into_iter()
            .map(|b| Frame::Integer(if b { 1 } else { 0 }))
            .collect();
        Ok(Frame::Array(frames))
    }

    pub fn bf_mexists(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 2)?;
        let name = cmd.arg_str(0)?;
        let items: Vec<&[u8]> = cmd.args[1..].iter().map(|b| b.as_ref()).collect();
        let results = self.ctx.blooms.mexists(name, &items);
        let frames: Vec<Frame> = results
            .into_iter()
            .map(|b| Frame::Integer(if b { 1 } else { 0 }))
            .collect();
        Ok(Frame::Array(frames))
    }

    // -----------------------------------------------------------------------
    // Stream commands
    // -----------------------------------------------------------------------

    pub fn xadd(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // XADD key [MAXLEN ~ count] id|* field value [field value ...]
        self.require_args(cmd, 3)?;
        let stream_name = cmd.arg_str(0)?;

        // Simple parsing: skip MAXLEN for now, find * or explicit ID.
        let mut idx = 1;
        let id_hint = cmd.arg_str(idx)?;
        idx += 1;

        // Remaining args are field-value pairs.
        if (cmd.arg_count() - idx) % 2 != 0 {
            return Err(CommandError::WrongArity {
                command: "XADD".into(),
            });
        }

        let mut fields = Vec::new();
        while idx + 1 < cmd.arg_count() {
            let field = cmd.arg_bytes(idx)?.clone();
            let value = cmd.arg_bytes(idx + 1)?.clone();
            fields.push((field, value));
            idx += 2;
        }

        let id = self
            .ctx
            .streams
            .xadd(stream_name, Some(id_hint), fields)?;
        Ok(Frame::BulkString(Bytes::from(id.to_string())))
    }

    pub fn xlen(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_exact_args(cmd, 1)?;
        let stream_name = cmd.arg_str(0)?;
        let len = self.ctx.streams.xlen(stream_name)?;
        Ok(Frame::Integer(len as i64))
    }

    pub fn xread(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // XREAD [COUNT count] STREAMS key [key ...] id [id ...]
        self.require_args(cmd, 3)?;

        let mut count: Option<usize> = None;
        let mut idx = 0;

        // Parse optional COUNT.
        if cmd.arg_str(idx)?.to_ascii_uppercase() == "COUNT" {
            idx += 1;
            count = Some(cmd.arg_i64(idx)? as usize);
            idx += 1;
        }

        // Expect STREAMS keyword.
        let kw = cmd.arg_str(idx)?.to_ascii_uppercase();
        if kw != "STREAMS" {
            return Err(CommandError::Syntax("expected STREAMS keyword".into()));
        }
        idx += 1;

        // Remaining args: N stream names followed by N IDs.
        let remaining = cmd.arg_count() - idx;
        if remaining % 2 != 0 {
            return Err(CommandError::WrongArity {
                command: "XREAD".into(),
            });
        }
        let n = remaining / 2;
        let mut streams = Vec::with_capacity(n);
        for i in 0..n {
            let name = cmd.arg_str(idx + i)?.to_string();
            let id_str = cmd.arg_str(idx + n + i)?;
            let id = StreamId::parse(id_str).map_err(|e| CommandError::Syntax(e.to_string()))?;
            streams.push((name, id));
        }

        let results = self.ctx.streams.xread(&streams, count)?;

        // Build response: array of [stream_name, entries_array]
        let frames: Vec<Frame> = results
            .into_iter()
            .map(|(name, entries)| {
                let entry_frames: Vec<Frame> = entries
                    .into_iter()
                    .map(|e| {
                        let field_frames: Vec<Frame> = e
                            .fields
                            .into_iter()
                            .flat_map(|(k, v)| {
                                vec![Frame::BulkString(k), Frame::BulkString(v)]
                            })
                            .collect();
                        Frame::Array(vec![
                            Frame::BulkString(Bytes::from(e.id.to_string())),
                            Frame::Array(field_frames),
                        ])
                    })
                    .collect();
                Frame::Array(vec![
                    Frame::BulkString(Bytes::from(name)),
                    Frame::Array(entry_frames),
                ])
            })
            .collect();

        Ok(Frame::Array(frames))
    }

    pub fn xrange(&self, cmd: &Command) -> Result<Frame, CommandError> {
        self.require_args(cmd, 3)?;
        let stream_name = cmd.arg_str(0)?;
        let start_str = cmd.arg_str(1)?;
        let end_str = cmd.arg_str(2)?;

        let start = if start_str == "-" {
            StreamId::ZERO
        } else {
            StreamId::parse(start_str).map_err(|e| CommandError::Syntax(e.to_string()))?
        };
        let end = if end_str == "+" {
            StreamId::new(u64::MAX, u64::MAX)
        } else {
            StreamId::parse(end_str).map_err(|e| CommandError::Syntax(e.to_string()))?
        };

        let count = if cmd.arg_count() > 4 && cmd.arg_str(3)?.to_ascii_uppercase() == "COUNT" {
            Some(cmd.arg_i64(4)? as usize)
        } else {
            None
        };

        let entries = self.ctx.streams.xrange(stream_name, start, end, count)?;

        let frames: Vec<Frame> = entries
            .into_iter()
            .map(|e| {
                let field_frames: Vec<Frame> = e
                    .fields
                    .into_iter()
                    .flat_map(|(k, v)| vec![Frame::BulkString(k), Frame::BulkString(v)])
                    .collect();
                Frame::Array(vec![
                    Frame::BulkString(Bytes::from(e.id.to_string())),
                    Frame::Array(field_frames),
                ])
            })
            .collect();

        Ok(Frame::Array(frames))
    }

    pub fn xreadgroup(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // XREADGROUP GROUP group consumer [COUNT count] STREAMS key id
        self.require_args(cmd, 6)?;

        let kw = cmd.arg_str(0)?.to_ascii_uppercase();
        if kw != "GROUP" {
            return Err(CommandError::Syntax("expected GROUP keyword".into()));
        }
        let group = cmd.arg_str(1)?;
        let consumer = cmd.arg_str(2)?;

        let mut idx = 3;
        let mut count: Option<usize> = None;

        if cmd.arg_str(idx)?.to_ascii_uppercase() == "COUNT" {
            idx += 1;
            count = Some(cmd.arg_i64(idx)? as usize);
            idx += 1;
        }

        if cmd.arg_str(idx)?.to_ascii_uppercase() != "STREAMS" {
            return Err(CommandError::Syntax("expected STREAMS keyword".into()));
        }
        idx += 1;

        let stream_name = cmd.arg_str(idx)?;

        let entries = self
            .ctx
            .streams
            .xreadgroup(stream_name, group, consumer, count)?;

        let entry_frames: Vec<Frame> = entries
            .into_iter()
            .map(|e| {
                let field_frames: Vec<Frame> = e
                    .fields
                    .into_iter()
                    .flat_map(|(k, v)| vec![Frame::BulkString(k), Frame::BulkString(v)])
                    .collect();
                Frame::Array(vec![
                    Frame::BulkString(Bytes::from(e.id.to_string())),
                    Frame::Array(field_frames),
                ])
            })
            .collect();

        Ok(Frame::Array(entry_frames))
    }

    pub fn xack(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // XACK key group id [id ...]
        self.require_args(cmd, 3)?;
        let stream_name = cmd.arg_str(0)?;
        let group = cmd.arg_str(1)?;

        let mut ids = Vec::new();
        for i in 2..cmd.arg_count() {
            let id_str = cmd.arg_str(i)?;
            let id = StreamId::parse(id_str).map_err(|e| CommandError::Syntax(e.to_string()))?;
            ids.push(id);
        }

        let count = self.ctx.streams.xack(stream_name, group, &ids)?;
        Ok(Frame::Integer(count as i64))
    }

    pub fn xtrim(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // XTRIM key MAXLEN [~] count
        self.require_args(cmd, 3)?;
        let stream_name = cmd.arg_str(0)?;

        let mut idx = 1;
        let kw = cmd.arg_str(idx)?.to_ascii_uppercase();
        if kw != "MAXLEN" {
            return Err(CommandError::Syntax("expected MAXLEN".into()));
        }
        idx += 1;

        // Skip optional ~
        if cmd.arg_str(idx)? == "~" {
            idx += 1;
        }

        let max_len = cmd.arg_i64(idx)? as usize;
        let trimmed = self.ctx.streams.xtrim(stream_name, max_len)?;
        Ok(Frame::Integer(trimmed as i64))
    }

    pub fn xgroup(&self, cmd: &Command) -> Result<Frame, CommandError> {
        // XGROUP CREATE key group id
        self.require_args(cmd, 3)?;
        let subcmd = cmd.arg_str(0)?.to_ascii_uppercase();

        match subcmd.as_str() {
            "CREATE" => {
                self.require_args(cmd, 4)?;
                let stream_name = cmd.arg_str(1)?;
                let group_name = cmd.arg_str(2)?;
                let id_str = cmd.arg_str(3)?;
                let start_id = if id_str == "$" {
                    // $ means start from the last entry
                    StreamId::new(u64::MAX, u64::MAX)
                } else {
                    StreamId::parse(id_str)
                        .map_err(|e| CommandError::Syntax(e.to_string()))?
                };
                self.ctx
                    .streams
                    .xgroup_create(stream_name, group_name, start_id)?;
                Ok(Frame::ok())
            }
            "DELCONSUMER" => {
                self.require_args(cmd, 4)?;
                let stream_name = cmd.arg_str(1)?;
                let group_name = cmd.arg_str(2)?;
                let consumer_name = cmd.arg_str(3)?;
                let pending = self
                    .ctx
                    .streams
                    .xgroup_delconsumer(stream_name, group_name, consumer_name)?;
                Ok(Frame::Integer(pending as i64))
            }
            _ => Err(CommandError::Syntax(format!(
                "unknown XGROUP subcommand: {subcmd}"
            ))),
        }
    }
}

/// Convert a ScriptResult to a RESP3 Frame.
fn script_result_to_frame(result: ScriptResult) -> Frame {
    match result {
        ScriptResult::Nil => Frame::Null,
        ScriptResult::Integer(n) => Frame::Integer(n),
        ScriptResult::Str(s) => Frame::BulkString(Bytes::from(s)),
        ScriptResult::Bool(b) => Frame::Integer(if b { 1 } else { 0 }),
        ScriptResult::Array(items) => {
            Frame::Array(items.into_iter().map(script_result_to_frame).collect())
        }
        ScriptResult::Error(e) => Frame::Error(e),
    }
}
