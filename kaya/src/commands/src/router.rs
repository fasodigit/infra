//! Command router: dispatches commands to the right handler.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc;

use kaya_protocol::{Command, Frame};
use kaya_pubsub::PubSubMessage;

// Rhai engine used for constructing FunctionsHandler at call time.
use rhai::Engine as RhaiEngine;

use crate::functions::FunctionsHandler;
use crate::handler::CommandHandler;
use crate::probabilistic::ProbabilisticHandler;
use crate::tracking::TrackingCommandHandler;
use crate::{geo, CommandContext, CommandError};

/// Routes incoming commands to the appropriate handler.
pub struct CommandRouter {
    ctx: Arc<CommandContext>,
}

impl CommandRouter {
    pub fn new(ctx: Arc<CommandContext>) -> Self {
        Self { ctx }
    }

    // -----------------------------------------------------------------------
    // Session-aware async execution (Pub/Sub, Functions, CLIENT TRACKING)
    // -----------------------------------------------------------------------

    /// Execute a command that may require per-connection session state.
    ///
    /// Commands that need a push channel (SUBSCRIBE, PSUBSCRIBE, SSUBSCRIBE)
    /// or a client ID (CLIENT TRACKING, FCALL) are routed here. All other
    /// commands delegate to the synchronous [`execute`] path.
    pub async fn execute_with_session(
        &self,
        cmd: &Command,
        client_id: u64,
        push_sink: &mpsc::Sender<Frame>,
    ) -> Frame {
        match cmd.name.as_str() {
            // -- Pub/Sub commands (require push channel) ----------------------
            "SUBSCRIBE" => self.handle_subscribe(cmd, client_id, push_sink).await,
            "UNSUBSCRIBE" => self.handle_unsubscribe(cmd).await,
            "PSUBSCRIBE" => self.handle_psubscribe(cmd, client_id, push_sink).await,
            "PUNSUBSCRIBE" => self.handle_punsubscribe(cmd).await,
            "SSUBSCRIBE" => self.handle_ssubscribe(cmd, client_id, push_sink).await,
            "SUNSUBSCRIBE" => self.handle_sunsubscribe(cmd).await,
            "PUBLISH" => self.handle_publish_cmd(cmd).await,
            "SPUBLISH" => self.handle_spublish_cmd(cmd).await,
            "PUBSUB" => self.handle_pubsub_cmd(cmd).await,

            // -- Functions commands -------------------------------------------
            "FUNCTION" => self.handle_function_cmd(cmd),
            "FCALL" => self.handle_fcall(cmd),
            "FCALL_RO" => self.handle_fcall_ro(cmd),

            // -- CLIENT command (delegates tracking sub-commands) -------------
            "CLIENT" => self.handle_client_with_tracking(cmd, client_id),

            // All other commands → synchronous path.
            _ => self.execute(cmd),
        }
    }

    // -----------------------------------------------------------------------
    // SUBSCRIBE
    // -----------------------------------------------------------------------

    async fn handle_subscribe(
        &self,
        cmd: &Command,
        client_id: u64,
        push_sink: &mpsc::Sender<Frame>,
    ) -> Frame {
        let broker = match &self.ctx.pubsub {
            Some(b) => b.clone(),
            None => return Frame::err("ERR pub/sub not enabled"),
        };
        if cmd.arg_count() < 1 {
            return Frame::err("ERR wrong number of arguments for 'SUBSCRIBE' command");
        }
        let channels: Vec<&[u8]> = (0..cmd.arg_count())
            .filter_map(|i| cmd.args.get(i).map(|b| b.as_ref()))
            .collect();

        // Build a PubSubMessage sender from the push Frame sender.
        let (msg_tx, mut msg_rx) = mpsc::channel::<PubSubMessage>(256);

        // Spawn a relay task that converts PubSubMessages into push Frames.
        let push_sink_clone = push_sink.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let frame = crate::pubsub::message_to_push_frame(&msg);
                if push_sink_clone.send(frame).await.is_err() {
                    break;
                }
            }
        });

        let frames =
            crate::pubsub::handle_subscribe(&broker, client_id, &channels, msg_tx).await;

        // Send all ack frames to the push channel; return the last ack
        // (or Null if empty) as the direct response.
        let mut last = Frame::Null;
        for f in frames {
            last = f.clone();
            let _ = push_sink.send(f).await;
        }
        last
    }

    // -----------------------------------------------------------------------
    // UNSUBSCRIBE
    // -----------------------------------------------------------------------

    async fn handle_unsubscribe(&self, _cmd: &Command) -> Frame {
        // Full unsubscribe bookkeeping lives in per-connection state that we
        // don't have here. Return the null-channel ack as per RESP3 spec.
        let broker = match &self.ctx.pubsub {
            Some(b) => b.clone(),
            None => return Frame::err("ERR pub/sub not enabled"),
        };
        let frames = crate::pubsub::handle_unsubscribe(&broker, &[], 0);
        frames.into_iter().next().unwrap_or(Frame::Null)
    }

    // -----------------------------------------------------------------------
    // PSUBSCRIBE
    // -----------------------------------------------------------------------

    async fn handle_psubscribe(
        &self,
        cmd: &Command,
        client_id: u64,
        push_sink: &mpsc::Sender<Frame>,
    ) -> Frame {
        let broker = match &self.ctx.pubsub {
            Some(b) => b.clone(),
            None => return Frame::err("ERR pub/sub not enabled"),
        };
        if cmd.arg_count() < 1 {
            return Frame::err("ERR wrong number of arguments for 'PSUBSCRIBE' command");
        }
        let patterns: Vec<&[u8]> = (0..cmd.arg_count())
            .filter_map(|i| cmd.args.get(i).map(|b| b.as_ref()))
            .collect();

        let (msg_tx, mut msg_rx) = mpsc::channel::<PubSubMessage>(256);

        let push_sink_clone = push_sink.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let frame = crate::pubsub::message_to_push_frame(&msg);
                if push_sink_clone.send(frame).await.is_err() {
                    break;
                }
            }
        });

        let frames =
            crate::pubsub::handle_psubscribe(&broker, client_id, &patterns, msg_tx).await;

        let mut last = Frame::Null;
        for f in frames {
            last = f.clone();
            let _ = push_sink.send(f).await;
        }
        last
    }

    // -----------------------------------------------------------------------
    // PUNSUBSCRIBE
    // -----------------------------------------------------------------------

    async fn handle_punsubscribe(&self, _cmd: &Command) -> Frame {
        let broker = match &self.ctx.pubsub {
            Some(b) => b.clone(),
            None => return Frame::err("ERR pub/sub not enabled"),
        };
        let frames = crate::pubsub::handle_punsubscribe(&broker, &[], 0);
        frames.into_iter().next().unwrap_or(Frame::Null)
    }

    // -----------------------------------------------------------------------
    // SSUBSCRIBE
    // -----------------------------------------------------------------------

    async fn handle_ssubscribe(
        &self,
        cmd: &Command,
        client_id: u64,
        push_sink: &mpsc::Sender<Frame>,
    ) -> Frame {
        let sharded = match &self.ctx.sharded_pubsub {
            Some(s) => s.clone(),
            None => return Frame::err("ERR sharded pub/sub not enabled"),
        };
        if cmd.arg_count() < 1 {
            return Frame::err("ERR wrong number of arguments for 'SSUBSCRIBE' command");
        }
        let channel = match cmd.args.first() {
            Some(b) => b.as_ref(),
            None => return Frame::err("ERR wrong number of arguments for 'SSUBSCRIBE' command"),
        };

        let (msg_tx, mut msg_rx) = mpsc::channel::<PubSubMessage>(256);

        let push_sink_clone = push_sink.clone();
        tokio::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                let frame = crate::pubsub::push_smessage(&msg.channel, &msg.payload);
                if push_sink_clone.send(frame).await.is_err() {
                    break;
                }
            }
        });

        let frame =
            crate::pubsub::handle_ssubscribe(&sharded, client_id, channel, msg_tx, 1);
        let _ = push_sink.send(frame.clone()).await;
        frame
    }

    // -----------------------------------------------------------------------
    // SUNSUBSCRIBE
    // -----------------------------------------------------------------------

    async fn handle_sunsubscribe(&self, _cmd: &Command) -> Frame {
        let sharded = match &self.ctx.sharded_pubsub {
            Some(s) => s.clone(),
            None => return Frame::err("ERR sharded pub/sub not enabled"),
        };
        let frames = crate::pubsub::handle_sunsubscribe(&sharded, &[], 0);
        frames.into_iter().next().unwrap_or(Frame::Null)
    }

    // -----------------------------------------------------------------------
    // PUBLISH
    // -----------------------------------------------------------------------

    async fn handle_publish_cmd(&self, cmd: &Command) -> Frame {
        let broker = match &self.ctx.pubsub {
            Some(b) => b.clone(),
            None => return Frame::err("ERR pub/sub not enabled"),
        };
        if cmd.arg_count() < 2 {
            return Frame::err("ERR wrong number of arguments for 'PUBLISH' command");
        }
        let channel = match cmd.args.first() {
            Some(b) => b.as_ref(),
            None => return Frame::err("ERR wrong number of arguments for 'PUBLISH' command"),
        };
        let message = match cmd.args.get(1) {
            Some(b) => b.as_ref(),
            None => return Frame::err("ERR wrong number of arguments for 'PUBLISH' command"),
        };
        crate::pubsub::handle_publish(&broker, channel, message).await
    }

    // -----------------------------------------------------------------------
    // SPUBLISH
    // -----------------------------------------------------------------------

    async fn handle_spublish_cmd(&self, cmd: &Command) -> Frame {
        let sharded = match &self.ctx.sharded_pubsub {
            Some(s) => s.clone(),
            None => return Frame::err("ERR sharded pub/sub not enabled"),
        };
        if cmd.arg_count() < 2 {
            return Frame::err("ERR wrong number of arguments for 'SPUBLISH' command");
        }
        let channel = match cmd.args.first() {
            Some(b) => b.as_ref(),
            None => return Frame::err("ERR wrong number of arguments for 'SPUBLISH' command"),
        };
        let message = match cmd.args.get(1) {
            Some(b) => b.as_ref(),
            None => return Frame::err("ERR wrong number of arguments for 'SPUBLISH' command"),
        };
        crate::pubsub::handle_spublish(&sharded, channel, message).await
    }

    // -----------------------------------------------------------------------
    // PUBSUB <subcommand> [args...]
    // -----------------------------------------------------------------------

    async fn handle_pubsub_cmd(&self, cmd: &Command) -> Frame {
        if cmd.arg_count() < 1 {
            return Frame::err(
                "ERR wrong number of arguments for 'PUBSUB' command",
            );
        }
        let sub = match cmd.arg_str(0) {
            Ok(s) => s.to_ascii_uppercase(),
            Err(e) => return Frame::err(format!("ERR {e}")),
        };

        match sub.as_str() {
            "CHANNELS" => {
                let broker = match &self.ctx.pubsub {
                    Some(b) => b.clone(),
                    None => return Frame::err("ERR pub/sub not enabled"),
                };
                let pattern = cmd.args.get(1).map(|b| b.as_ref());
                let channel_names = broker.channel_names();
                crate::pubsub::handle_pubsub_channels_from(&channel_names, pattern)
            }
            "NUMSUB" => {
                let broker = match &self.ctx.pubsub {
                    Some(b) => b.clone(),
                    None => return Frame::err("ERR pub/sub not enabled"),
                };
                let channels: Vec<&[u8]> = (1..cmd.arg_count())
                    .filter_map(|i| cmd.args.get(i).map(|b| b.as_ref()))
                    .collect();
                crate::pubsub::handle_pubsub_numsub(&broker, &channels)
            }
            "NUMPAT" => {
                let broker = match &self.ctx.pubsub {
                    Some(b) => b.clone(),
                    None => return Frame::err("ERR pub/sub not enabled"),
                };
                crate::pubsub::handle_pubsub_numpat(&broker)
            }
            "SHARDCHANNELS" => {
                let sharded = match &self.ctx.sharded_pubsub {
                    Some(s) => s.clone(),
                    None => return Frame::err("ERR sharded pub/sub not enabled"),
                };
                let pattern = cmd.args.get(1).map(|b| b.as_ref());
                let channels = sharded.channel_names();
                crate::pubsub::handle_pubsub_shardchannels_from(&channels, pattern)
            }
            "SHARDNUMSUB" => {
                let sharded = match &self.ctx.sharded_pubsub {
                    Some(s) => s.clone(),
                    None => return Frame::err("ERR sharded pub/sub not enabled"),
                };
                let channels: Vec<&[u8]> = (1..cmd.arg_count())
                    .filter_map(|i| cmd.args.get(i).map(|b| b.as_ref()))
                    .collect();
                crate::pubsub::handle_pubsub_shardnumsub(&sharded, &channels)
            }
            other => Frame::err(format!("ERR unknown PUBSUB subcommand: {other}")),
        }
    }

    // -----------------------------------------------------------------------
    // FUNCTION <subcommand> [args...]
    // -----------------------------------------------------------------------

    fn handle_function_cmd(&self, cmd: &Command) -> Frame {
        match self.build_functions_handler() {
            Ok(h) => h.function_cmd(cmd),
            Err(f) => f,
        }
    }

    fn handle_fcall(&self, cmd: &Command) -> Frame {
        match self.build_functions_handler() {
            Ok(h) => h.fcall(cmd),
            Err(f) => f,
        }
    }

    fn handle_fcall_ro(&self, cmd: &Command) -> Frame {
        match self.build_functions_handler() {
            Ok(h) => h.fcall_ro(cmd),
            Err(f) => f,
        }
    }

    /// Build a `FunctionsHandler` from the context, or return an error Frame
    /// if the functions registry is not available.
    fn build_functions_handler(&self) -> Result<FunctionsHandler, Frame> {
        let registry = match &self.ctx.functions {
            Some(r) => r.clone(),
            None => return Err(Frame::err("ERR FUNCTION commands not enabled")),
        };
        // Build a minimal Rhai Engine (sandboxed, no I/O).
        let engine = Arc::new(RhaiEngine::new());
        // Signing key: empty vec (no signing at runtime, only at load time).
        Ok(FunctionsHandler::new(registry, engine, vec![]))
    }

    // -----------------------------------------------------------------------
    // CLIENT command with tracking sub-command delegation
    // -----------------------------------------------------------------------

    /// Handle the CLIENT command, routing tracking-related sub-commands to the
    /// [`TrackingCommandHandler`] when a tracking table is available.
    fn handle_client_with_tracking(&self, cmd: &Command, client_id: u64) -> Frame {
        if cmd.arg_count() == 0 {
            // No subcommand — delegate to base handler.
            let handler = CommandHandler::new(self.ctx.clone());
            return handler.client_cmd(cmd).unwrap_or_else(|e| e.to_frame());
        }

        let subcmd = match cmd.arg_str(0) {
            Ok(s) => s.to_ascii_uppercase(),
            Err(e) => return Frame::err(format!("ERR {e}")),
        };

        // Tracking sub-commands are only available when a TrackingTable is set.
        if let Some(table) = &self.ctx.tracking {
            let tracking_handler = TrackingCommandHandler::new(client_id, table.clone());
            // rest = args after the sub-command token.
            let rest: Vec<Bytes> = cmd.args[1..].to_vec();
            if let Some(result) = tracking_handler.dispatch(&subcmd, &rest) {
                return match result {
                    Ok(frame) => frame,
                    Err(e) => e.to_frame(),
                };
            }
        }

        // Fall through to the standard CLIENT handler for non-tracking
        // sub-commands (GETNAME, SETNAME, ID, INFO, LIST, etc.).
        let handler = CommandHandler::new(self.ctx.clone());
        handler.client_cmd(cmd).unwrap_or_else(|e| e.to_frame())
    }

    // -----------------------------------------------------------------------
    // Synchronous execute (all non-session commands)
    // -----------------------------------------------------------------------

    /// Execute a parsed command and return the response frame.
    pub fn execute(&self, cmd: &Command) -> Frame {
        let handler = CommandHandler::new(self.ctx.clone());

        let result = match cmd.name.as_str() {
            // -- connection commands -------------------------------------------
            "PING" => handler.ping(cmd),
            "ECHO" => handler.echo(cmd),
            "AUTH" => handler.auth(cmd),
            "HELLO" => handler.hello(cmd),
            "SELECT" => handler.select_cmd(cmd),
            "CLIENT" => handler.client_cmd(cmd),
            "QUIT" => Ok(Frame::ok()),
            "RESET" => Ok(Frame::ok()),

            // -- string commands -----------------------------------------------
            "GET" => handler.get(cmd),
            "SET" => handler.set(cmd),
            "DEL" => handler.del(cmd),
            "MGET" => handler.mget(cmd),
            "MSET" => handler.mset(cmd),
            "EXISTS" => handler.exists(cmd),
            "EXPIRE" => handler.expire(cmd),
            "TTL" => handler.ttl(cmd),
            "PERSIST" => handler.persist(cmd),
            "INCR" => handler.incr(cmd),
            "DECR" => handler.decr(cmd),
            "INCRBY" => handler.incrby(cmd),
            "DECRBY" => handler.decrby(cmd),
            "DBSIZE" => handler.dbsize(cmd),
            "FLUSHDB" | "FLUSHALL" => handler.flushdb(cmd),
            "INFO" => handler.info(cmd),
            "COMMAND" => handler.command_info(cmd),
            "CONFIG" => handler.config(cmd),
            "TYPE" => handler.type_cmd(cmd),
            "KEYS" => handler.keys_cmd(cmd),

            // -- set commands --------------------------------------------------
            "SADD" => handler.sadd(cmd),
            "SISMEMBER" => handler.sismember(cmd),
            "SMEMBERS" => handler.smembers(cmd),
            "SREM" => handler.srem(cmd),
            "SCARD" => handler.scard(cmd),

            // -- sorted set commands -------------------------------------------
            "ZADD" => handler.zadd(cmd),
            "ZREM" => handler.zrem(cmd),
            "ZSCORE" => handler.zscore(cmd),
            "ZCARD" => handler.zcard(cmd),
            "ZRANGE" => handler.zrange(cmd),
            "ZRANGEBYSCORE" => handler.zrangebyscore(cmd),
            // Legacy compatibility: ZREVRANGE / ZREVRANGEBYSCORE / ZREVRANGEBYLEX
            // are deprecated since Redis 6.2 but still emitted by Spring Data Redis
            // (Lettuce/Jedis). They delegate to the REV-flag code path with zero
            // extra allocation — no format conversion, no string copies.
            "ZREVRANGE" => handler.zrevrange(cmd),
            "ZREVRANGEBYSCORE" => handler.zrevrangebyscore(cmd),
            "ZREVRANGEBYLEX" => handler.zrevrangebylex(cmd),

            // -- bloom filter commands -----------------------------------------
            "BF.ADD" => handler.bf_add(cmd),
            "BF.EXISTS" => handler.bf_exists(cmd),
            "BF.RESERVE" => handler.bf_reserve(cmd),
            "BF.MADD" => handler.bf_madd(cmd),
            "BF.MEXISTS" => handler.bf_mexists(cmd),

            // -- probabilistic data structures (Cuckoo, HLL, CMS, TopK) --------
            "CF.RESERVE" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_reserve(cmd),
            "CF.ADD" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_add(cmd),
            "CF.ADDNX" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_addnx(cmd),
            "CF.EXISTS" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_exists(cmd),
            "CF.DEL" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_del(cmd),
            "CF.COUNT" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_count(cmd),
            "CF.MEXISTS" => ProbabilisticHandler::new(self.ctx.prob.clone()).cf_mexists(cmd),
            "PFADD" => ProbabilisticHandler::new(self.ctx.prob.clone()).pf_add(cmd),
            "PFCOUNT" => ProbabilisticHandler::new(self.ctx.prob.clone()).pf_count(cmd),
            "PFMERGE" => ProbabilisticHandler::new(self.ctx.prob.clone()).pf_merge(cmd),
            "CMS.INITBYDIM" => ProbabilisticHandler::new(self.ctx.prob.clone()).cms_initbydim(cmd),
            "CMS.INITBYPROB" => ProbabilisticHandler::new(self.ctx.prob.clone()).cms_initbyprob(cmd),
            "CMS.INCRBY" => ProbabilisticHandler::new(self.ctx.prob.clone()).cms_incrby(cmd),
            "CMS.QUERY" => ProbabilisticHandler::new(self.ctx.prob.clone()).cms_query(cmd),
            "CMS.MERGE" => ProbabilisticHandler::new(self.ctx.prob.clone()).cms_merge(cmd),
            "CMS.INFO" => ProbabilisticHandler::new(self.ctx.prob.clone()).cms_info(cmd),
            "TOPK.RESERVE" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_reserve(cmd),
            "TOPK.ADD" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_add(cmd),
            "TOPK.INCRBY" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_incrby(cmd),
            "TOPK.QUERY" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_query(cmd),
            "TOPK.COUNT" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_count(cmd),
            "TOPK.LIST" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_list(cmd),
            "TOPK.INFO" => ProbabilisticHandler::new(self.ctx.prob.clone()).topk_info(cmd),

            // -- geo commands --------------------------------------------------
            "GEOADD" => geo::handle_geoadd(&self.ctx, cmd),
            "GEOPOS" => geo::handle_geopos(&self.ctx, cmd),
            "GEODIST" => geo::handle_geodist(&self.ctx, cmd),
            "GEOSEARCH" => geo::handle_geosearch(&self.ctx, cmd),
            "GEOSEARCHSTORE" => geo::handle_geosearchstore(&self.ctx, cmd),
            "GEORADIUS" => geo::handle_georadius(&self.ctx, cmd),
            "GEORADIUSBYMEMBER" => geo::handle_georadiusbymember(&self.ctx, cmd),
            "GEOHASH" => geo::handle_geohash(&self.ctx, cmd),
            "GEOREM" => geo::handle_georem(&self.ctx, cmd),

            // -- stream commands -----------------------------------------------
            "XADD" => handler.xadd(cmd),
            "XLEN" => handler.xlen(cmd),
            "XREAD" => handler.xread(cmd),
            "XRANGE" => handler.xrange(cmd),
            "XREADGROUP" => handler.xreadgroup(cmd),
            "XACK" => handler.xack(cmd),
            "XTRIM" => handler.xtrim(cmd),
            "XGROUP" => handler.xgroup(cmd),

            // -- scripting commands --------------------------------------------
            "EVAL" => handler.eval(cmd),
            "EVALSHA" => handler.evalsha(cmd),
            "SCRIPT" => handler.script(cmd),

            // -- MULTI/EXEC (handled at connection level, but stub here) -------
            "MULTI" | "EXEC" | "DISCARD" => {
                // These are handled by the connection state machine.
                // If they arrive here, it means they were not intercepted.
                Ok(Frame::err("ERR MULTI/EXEC must be used in a transaction context"))
            }

            // -- JSON commands -------------------------------------------------
            "JSON.SET" => self.route_json(|s| crate::json::handle_json_set(s, cmd)),
            "JSON.GET" => self.route_json(|s| crate::json::handle_json_get(s, cmd)),
            "JSON.DEL" => self.route_json(|s| crate::json::handle_json_del(s, cmd)),
            "JSON.FORGET" => self.route_json(|s| crate::json::handle_json_forget(s, cmd)),
            "JSON.TYPE" => self.route_json(|s| crate::json::handle_json_type(s, cmd)),
            "JSON.NUMINCRBY" => self.route_json(|s| crate::json::handle_json_numincrby(s, cmd)),
            "JSON.NUMMULTBY" => self.route_json(|s| crate::json::handle_json_nummultby(s, cmd)),
            "JSON.STRAPPEND" => self.route_json(|s| crate::json::handle_json_strappend(s, cmd)),
            "JSON.STRLEN" => self.route_json(|s| crate::json::handle_json_strlen(s, cmd)),
            "JSON.ARRAPPEND" => self.route_json(|s| crate::json::handle_json_arrappend(s, cmd)),
            "JSON.ARRLEN" => self.route_json(|s| crate::json::handle_json_arrlen(s, cmd)),
            "JSON.ARRPOP" => self.route_json(|s| crate::json::handle_json_arrpop(s, cmd)),
            "JSON.ARRINDEX" => self.route_json(|s| crate::json::handle_json_arrindex(s, cmd)),
            "JSON.ARRINSERT" => self.route_json(|s| crate::json::handle_json_arrinsert(s, cmd)),
            "JSON.ARRTRIM" => self.route_json(|s| crate::json::handle_json_arrtrim(s, cmd)),
            "JSON.OBJKEYS" => self.route_json(|s| crate::json::handle_json_objkeys(s, cmd)),
            "JSON.OBJLEN" => self.route_json(|s| crate::json::handle_json_objlen(s, cmd)),
            "JSON.TOGGLE" => self.route_json(|s| crate::json::handle_json_toggle(s, cmd)),
            "JSON.CLEAR" => self.route_json(|s| crate::json::handle_json_clear(s, cmd)),
            "JSON.MGET" => self.route_json(|s| crate::json::handle_json_mget(s, cmd)),
            "JSON.DEBUG" => self.route_json(|s| crate::json::handle_json_debug(s, cmd)),
            "JSON.RESP" => self.route_json(|s| crate::json::handle_json_resp(s, cmd)),

            // -- TimeSeries commands -------------------------------------------
            "TS.CREATE" => self.route_ts(|s| crate::timeseries::handle_ts_create(s, cmd)),
            "TS.ALTER" => self.route_ts(|s| crate::timeseries::handle_ts_alter(s, cmd)),
            "TS.DEL" => self.route_ts(|s| crate::timeseries::handle_ts_del(s, cmd)),
            "TS.ADD" => self.route_ts(|s| crate::timeseries::handle_ts_add(s, cmd)),
            "TS.MADD" => self.route_ts(|s| crate::timeseries::handle_ts_madd(s, cmd)),
            "TS.INCRBY" => self.route_ts(|s| crate::timeseries::handle_ts_incrby(s, cmd)),
            "TS.DECRBY" => self.route_ts(|s| crate::timeseries::handle_ts_decrby(s, cmd)),
            "TS.GET" => self.route_ts(|s| crate::timeseries::handle_ts_get(s, cmd)),
            "TS.MGET" => self.route_ts(|s| crate::timeseries::handle_ts_mget(s, cmd)),
            "TS.RANGE" => self.route_ts(|s| crate::timeseries::handle_ts_range(s, cmd)),
            "TS.REVRANGE" => self.route_ts(|s| crate::timeseries::handle_ts_revrange(s, cmd)),
            "TS.MRANGE" => self.route_ts(|s| crate::timeseries::handle_ts_mrange(s, cmd)),
            "TS.MREVRANGE" => self.route_ts(|s| crate::timeseries::handle_ts_mrevrange(s, cmd)),
            "TS.CREATERULE" => self.route_ts(|s| crate::timeseries::handle_ts_createrule(s, cmd)),
            "TS.DELETERULE" => self.route_ts(|s| crate::timeseries::handle_ts_deleterule(s, cmd)),
            "TS.QUERYINDEX" => self.route_ts(|s| crate::timeseries::handle_ts_queryindex(s, cmd)),
            "TS.INFO" => self.route_ts(|s| crate::timeseries::handle_ts_info(s, cmd)),

            // -- FT.* : vector HNSW first, full-text Tantivy fallback ---------
            name if name.starts_with("FT.") => {
                if let Some(vec_store) = &self.ctx.vector {
                    if let Some(frame) = crate::vector::dispatch_vector_command(vec_store, cmd) {
                        return frame;
                    }
                }
                if let Some(ft_store) = &self.ctx.fulltext {
                    if let Some(frame) = crate::fulltext::dispatch_fulltext_command(ft_store, cmd) {
                        return frame;
                    }
                }
                Err(CommandError::UnknownCommand(name.to_string()))
            }

            _ => Err(CommandError::UnknownCommand(cmd.name.clone())),
        };

        match result {
            Ok(frame) => frame,
            Err(e) => e.to_frame(),
        }
    }

    /// Execute a batch of commands atomically (for MULTI/EXEC).
    /// Returns an array of response frames.
    pub fn execute_multi(&self, commands: &[Command]) -> Frame {
        let mut responses = Vec::with_capacity(commands.len());
        for cmd in commands {
            responses.push(self.execute(cmd));
        }
        Frame::Array(responses)
    }

    // -----------------------------------------------------------------------
    // Helpers: Option<Arc<Store>> → Result<Frame, CommandError>
    // -----------------------------------------------------------------------

    fn route_json<F>(&self, f: F) -> Result<Frame, CommandError>
    where
        F: FnOnce(&kaya_json::JsonStore) -> Result<Frame, CommandError>,
    {
        match &self.ctx.json {
            Some(store) => f(store),
            None => Ok(Frame::err("ERR JSON support not enabled")),
        }
    }

    fn route_ts<F>(&self, f: F) -> Result<Frame, CommandError>
    where
        F: FnOnce(&kaya_timeseries::TimeSeriesStore) -> Result<Frame, CommandError>,
    {
        match &self.ctx.timeseries {
            Some(store) => f(store),
            None => Ok(Frame::err("ERR TimeSeries support not enabled")),
        }
    }
}

// ---------------------------------------------------------------------------
// RequestHandler implementation for CommandRouter
// ---------------------------------------------------------------------------

impl kaya_network::RequestHandler for CommandRouter {
    fn handle_command(&self, cmd: Command) -> Frame {
        self.execute(&cmd)
    }

    fn handle_multi(&self, commands: &[Command]) -> Frame {
        self.execute_multi(commands)
    }

    fn handle_command_with_session(
        &self,
        cmd: Command,
        client_id: u64,
        push_sink: mpsc::Sender<Frame>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Frame> + Send + '_>> {
        Box::pin(async move {
            self.execute_with_session(&cmd, client_id, &push_sink).await
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use tokio::sync::mpsc;

    use kaya_protocol::{Command, Frame};
    use kaya_pubsub::{PubSubBroker, ShardedPubSub};
    use kaya_scripting::functions::FunctionRegistry;
    use kaya_network::tracking::TrackingTable;
    use kaya_store::{BloomManager, Store};
    use kaya_streams::StreamManager;

    use crate::{CommandContext, CommandRouter};

    // -----------------------------------------------------------------------
    // Helper: build a CommandRouter with all session-state subsystems wired.
    // -----------------------------------------------------------------------

    fn make_router() -> (CommandRouter, Arc<PubSubBroker>, Arc<TrackingTable>) {
        let store = Arc::new(Store::default());
        let streams = Arc::new(StreamManager::default());
        let blooms = Arc::new(BloomManager::new());
        let broker = Arc::new(PubSubBroker::new());
        let sharded = Arc::new(ShardedPubSub::new(4));
        let tracking = Arc::new(TrackingTable::new());

        let ctx = CommandContext::new(store, streams, blooms)
            .with_pubsub(broker.clone())
            .with_sharded_pubsub(sharded)
            .with_tracking(tracking.clone());

        (CommandRouter::new(Arc::new(ctx)), broker, tracking)
    }

    fn make_cmd(name: &str, args: &[&str]) -> Command {
        Command {
            name: name.to_uppercase(),
            args: args.iter().map(|a| Bytes::from(a.to_string())).collect(),
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: SUBSCRIBE via execute_with_session creates broker subscription
    //         and PUBLISH delivers the message via push channel.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn subscribe_via_router_then_publish_delivers_push_frame() {
        let (router, broker, _) = make_router();
        let (push_tx, mut push_rx) = mpsc::channel::<Frame>(64);

        // SUBSCRIBE to "events"
        let sub_cmd = make_cmd("SUBSCRIBE", &["events"]);
        let _ack = router
            .execute_with_session(&sub_cmd, 1, &push_tx)
            .await;

        // Drain the ack frame(s) from the push channel.
        let ack = push_rx.recv().await.expect("subscribe ack");
        match &ack {
            Frame::Push(v) => {
                assert_eq!(v[0], Frame::SimpleString("subscribe".into()));
            }
            other => panic!("expected Push ack, got {other:?}"),
        }

        // PUBLISH a message via the broker directly.
        let delivered = broker
            .publish(b"events", Bytes::from("hello-world"))
            .await;
        assert_eq!(delivered, 1, "one subscriber should receive the message");

        // The relay task should forward the PubSubMessage as a push Frame.
        let msg_frame = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            push_rx.recv(),
        )
        .await
        .expect("timeout waiting for push")
        .expect("channel closed");

        match msg_frame {
            Frame::Push(v) => {
                assert_eq!(v[0], Frame::SimpleString("message".into()));
                assert_eq!(v[1], Frame::BulkString(Bytes::from("events")));
                assert_eq!(v[2], Frame::BulkString(Bytes::from("hello-world")));
            }
            other => panic!("expected message Push frame, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 2: PUBLISH via execute_with_session returns delivery count.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn publish_via_router_returns_delivery_count() {
        let (router, broker, _) = make_router();
        let (push_tx, _push_rx) = mpsc::channel::<Frame>(64);

        // Register a subscriber directly on the broker.
        let (sub_tx, _sub_rx) = mpsc::channel(16);
        broker.subscribe(Bytes::from("news"), sub_tx);

        let pub_cmd = make_cmd("PUBLISH", &["news", "breaking"]);
        let result = router
            .execute_with_session(&pub_cmd, 99, &push_tx)
            .await;

        assert_eq!(result, Frame::Integer(1), "one subscriber");
    }

    // -----------------------------------------------------------------------
    // Test 3: FUNCTION LOAD + FCALL via execute_with_session.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn fcall_round_trip_via_router() {
        let store = Arc::new(Store::default());
        let streams = Arc::new(StreamManager::default());
        let blooms = Arc::new(BloomManager::new());
        let signing_key = b"test-key-32-bytes!!!!!!!!!!!!!!x".to_vec();
        let registry = Arc::new(FunctionRegistry::new(signing_key));

        let ctx = CommandContext::new(store, streams, blooms)
            .with_functions(registry);
        let router = CommandRouter::new(Arc::new(ctx));
        let (push_tx, _) = mpsc::channel::<Frame>(16);

        // Load a simple add function.
        let script = "#!rhai name=math engine=rhai\nfn add(keys, args) { let a = args[0].parse_int(); let b = args[1].parse_int(); a + b }\n";
        let load_cmd = make_cmd("FUNCTION", &["LOAD", script]);
        let load_result = router
            .execute_with_session(&load_cmd, 1, &push_tx)
            .await;
        assert!(
            matches!(&load_result, Frame::BulkString(b) if b.as_ref() == b"math"),
            "expected library name 'math', got {load_result:?}"
        );

        // FCALL add 0 3 5
        let fcall_cmd = make_cmd("FCALL", &["add", "0", "3", "5"]);
        let result = router
            .execute_with_session(&fcall_cmd, 1, &push_tx)
            .await;
        assert_eq!(result, Frame::Integer(8), "3 + 5 = 8");
    }

    // -----------------------------------------------------------------------
    // Test 4: CLIENT TRACKING ON via execute_with_session registers the client.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn client_tracking_on_registers_via_router() {
        let (router, _, tracking) = make_router();
        let (push_tx, _) = mpsc::channel::<Frame>(16);

        let cmd = make_cmd("CLIENT", &["TRACKING", "ON"]);
        let result = router
            .execute_with_session(&cmd, 42, &push_tx)
            .await;
        assert_eq!(result, Frame::ok());
        assert_eq!(tracking.client_count(), 1);
    }

    // -----------------------------------------------------------------------
    // Test 5: CLIENT TRACKING ON then key mutation triggers invalidation push.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn client_tracking_invalidate_triggers_push() {
        use dashmap::DashMap;
        use kaya_network::tracking::ClientId;

        let (router, _, tracking) = make_router();
        let (push_tx, mut push_rx) = mpsc::channel::<Frame>(64);

        // Enable tracking for client 55.
        let cmd = make_cmd("CLIENT", &["TRACKING", "ON"]);
        router.execute_with_session(&cmd, 55, &push_tx).await;

        // Simulate a read — tell the table that client 55 has accessed "price:BTC".
        tracking.track_read(55, b"price:BTC").unwrap();

        // Build sender map for the table's invalidate call.
        let senders: DashMap<ClientId, mpsc::Sender<Frame>> = DashMap::new();
        senders.insert(55, push_tx.clone());

        // Trigger invalidation (simulate a write by another client).
        tracking.invalidate(&[b"price:BTC"], None, &senders).await;

        let push_frame = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            push_rx.recv(),
        )
        .await
        .expect("timeout waiting for invalidation push")
        .expect("channel closed");

        match push_frame {
            Frame::Push(ref v) => {
                assert!(
                    matches!(&v[0], Frame::BulkString(b) if b.as_ref() == b"invalidate"),
                    "first element must be 'invalidate', got {push_frame:?}"
                );
            }
            other => panic!("expected Push invalidation frame, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 6: PUBSUB NUMSUB via execute_with_session.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn pubsub_numsub_via_router() {
        let (router, broker, _) = make_router();
        let (push_tx, _) = mpsc::channel::<Frame>(16);

        // Register two subscribers directly.
        for _ in 0..2 {
            let (tx, _rx) = mpsc::channel(8);
            broker.subscribe(Bytes::from("alpha"), tx);
        }

        let cmd = make_cmd("PUBSUB", &["NUMSUB", "alpha", "beta"]);
        let result = router
            .execute_with_session(&cmd, 1, &push_tx)
            .await;

        match result {
            Frame::Array(items) => {
                assert_eq!(items.len(), 4);
                assert_eq!(items[0], Frame::BulkString(Bytes::from("alpha")));
                assert_eq!(items[1], Frame::Integer(2));
                assert_eq!(items[2], Frame::BulkString(Bytes::from("beta")));
                assert_eq!(items[3], Frame::Integer(0));
            }
            other => panic!("expected Array, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 7: execute_with_session falls through to execute for standard cmds.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn standard_commands_still_work_via_session_path() {
        let (router, _, _) = make_router();
        let (push_tx, _) = mpsc::channel::<Frame>(16);

        let ping_cmd = make_cmd("PING", &[]);
        let result = router
            .execute_with_session(&ping_cmd, 1, &push_tx)
            .await;
        assert_eq!(result, Frame::SimpleString("PONG".into()));

        let set_cmd = make_cmd("SET", &["mykey", "myval"]);
        let set_result = router
            .execute_with_session(&set_cmd, 1, &push_tx)
            .await;
        assert_eq!(set_result, Frame::ok());

        let get_cmd = make_cmd("GET", &["mykey"]);
        let get_result = router
            .execute_with_session(&get_cmd, 1, &push_tx)
            .await;
        assert_eq!(
            get_result,
            Frame::BulkString(Bytes::from("myval")),
            "GET must return the stored value"
        );
    }

    // -----------------------------------------------------------------------
    // Sorted-set ZREVRANGE / ZREVRANGEBYSCORE / ZREVRANGEBYLEX tests
    // These cover Bug A: Spring Data Redis sends ZREVRANGE but KAYA only had
    // ZRANGE. Regression guard for the 7 134 errors / hour production issue.
    // -----------------------------------------------------------------------

    fn sorted_router() -> CommandRouter {
        let (router, _, _) = make_router();
        router
    }

    /// Seed key `foo` with members a(1), b(2), c(3) and return the router.
    fn router_with_foo() -> CommandRouter {
        let router = sorted_router();
        let zadd = make_cmd("ZADD", &["foo", "1", "a", "2", "b", "3", "c"]);
        let result = router.execute(&zadd);
        assert_eq!(result, Frame::Integer(3), "ZADD must add 3 members");
        router
    }

    // -- Test 8: ZREVRANGE basic descending order ----------------------------

    #[tokio::test]
    async fn test_zrevrange_basic() {
        let router = router_with_foo();
        // ZREVRANGE foo 0 -1 → [c, b, a] (highest score first)
        let cmd = make_cmd("ZREVRANGE", &["foo", "0", "-1"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("b")),
                Frame::BulkString(Bytes::from("a")),
            ]),
            "ZREVRANGE must return members in descending score order"
        );
    }

    // -- Test 9: ZREVRANGE WITHSCORES ----------------------------------------

    #[tokio::test]
    async fn test_zrevrange_withscores() {
        let router = router_with_foo();
        // ZREVRANGE foo 0 -1 WITHSCORES → [c 3 b 2 a 1]
        let cmd = make_cmd("ZREVRANGE", &["foo", "0", "-1", "WITHSCORES"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("3")),
                Frame::BulkString(Bytes::from("b")),
                Frame::BulkString(Bytes::from("2")),
                Frame::BulkString(Bytes::from("a")),
                Frame::BulkString(Bytes::from("1")),
            ]),
            "ZREVRANGE WITHSCORES must interleave member/score descending"
        );
    }

    // -- Test 10: ZREVRANGE partial range ------------------------------------

    #[tokio::test]
    async fn test_zrevrange_partial_range() {
        let router = router_with_foo();
        // ZREVRANGE foo 0 1 → [c, b] (top-2)
        let cmd = make_cmd("ZREVRANGE", &["foo", "0", "1"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("b")),
            ]),
            "ZREVRANGE 0 1 must return top-2 members"
        );
    }

    // -- Test 11: ZREVRANGEBYSCORE basic -------------------------------------

    #[tokio::test]
    async fn test_zrevrangebyscore_basic() {
        let router = router_with_foo();
        // ZREVRANGEBYSCORE foo +inf -inf → [c, b, a]
        let cmd = make_cmd("ZREVRANGEBYSCORE", &["foo", "+inf", "-inf"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("b")),
                Frame::BulkString(Bytes::from("a")),
            ]),
            "ZREVRANGEBYSCORE +inf -inf must return all members descending"
        );
    }

    // -- Test 12: ZREVRANGEBYSCORE LIMIT -------------------------------------

    #[tokio::test]
    async fn test_zrevrangebyscore_limit() {
        let router = router_with_foo();
        // ZREVRANGEBYSCORE foo +inf -inf LIMIT 0 2 → [c, b]
        let cmd = make_cmd("ZREVRANGEBYSCORE", &["foo", "+inf", "-inf", "LIMIT", "0", "2"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("b")),
            ]),
            "ZREVRANGEBYSCORE LIMIT 0 2 must return top-2 members"
        );
    }

    // -- Test 13: ZREVRANGEBYSCORE score bound filter ------------------------

    #[tokio::test]
    async fn test_zrevrangebyscore_score_filter() {
        let router = router_with_foo();
        // ZREVRANGEBYSCORE foo 2 1 → [b, a]  (scores between 1 and 2, descending)
        let cmd = make_cmd("ZREVRANGEBYSCORE", &["foo", "2", "1"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("b")),
                Frame::BulkString(Bytes::from("a")),
            ]),
            "ZREVRANGEBYSCORE 2 1 must return members with scores in [1,2] descending"
        );
    }

    // -- Test 14: ZREVRANGEBYSCORE WITHSCORES --------------------------------

    #[tokio::test]
    async fn test_zrevrangebyscore_withscores() {
        let router = router_with_foo();
        // ZREVRANGEBYSCORE foo +inf -inf WITHSCORES → [c 3 b 2 a 1]
        let cmd = make_cmd("ZREVRANGEBYSCORE", &["foo", "+inf", "-inf", "WITHSCORES"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("3")),
                Frame::BulkString(Bytes::from("b")),
                Frame::BulkString(Bytes::from("2")),
                Frame::BulkString(Bytes::from("a")),
                Frame::BulkString(Bytes::from("1")),
            ]),
            "ZREVRANGEBYSCORE WITHSCORES must interleave member/score descending"
        );
    }

    // -- Test 15: ZREVRANGE on empty key returns empty array -----------------

    #[tokio::test]
    async fn test_zrevrange_missing_key() {
        let router = sorted_router();
        let cmd = make_cmd("ZREVRANGE", &["nonexistent", "0", "-1"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![]),
            "ZREVRANGE on missing key must return empty array"
        );
    }

    // -- Test 16: ZREVRANGEBYSCORE empty range -------------------------------

    #[tokio::test]
    async fn test_zrevrangebyscore_empty_range() {
        let router = router_with_foo();
        // Request range with inverted bounds that produce no results
        let cmd = make_cmd("ZREVRANGEBYSCORE", &["foo", "0", "10"]);
        let result = router.execute(&cmd);
        // min=10 > max=0 → no member has score in [10, 0]
        assert_eq!(
            result,
            Frame::Array(vec![]),
            "ZREVRANGEBYSCORE with empty range must return empty array"
        );
    }

    // -- Test 17: ZREVRANGEBYLEX basic --------------------------------------

    #[tokio::test]
    async fn test_zrevrangebylex_basic() {
        // All members with score=0 for lex ordering test.
        let router = sorted_router();
        let zadd = make_cmd("ZADD", &["lexkey", "0", "a", "0", "b", "0", "c", "0", "d"]);
        router.execute(&zadd);
        // ZREVRANGEBYLEX lexkey + - → [d, c, b, a]
        let cmd = make_cmd("ZREVRANGEBYLEX", &["lexkey", "+", "-"]);
        let result = router.execute(&cmd);
        assert_eq!(
            result,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("d")),
                Frame::BulkString(Bytes::from("c")),
                Frame::BulkString(Bytes::from("b")),
                Frame::BulkString(Bytes::from("a")),
            ]),
            "ZREVRANGEBYLEX + - must return all members in descending lex order"
        );
    }
}
