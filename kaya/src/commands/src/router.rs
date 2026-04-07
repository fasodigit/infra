//! Command router: dispatches commands to the right handler.

use std::sync::Arc;

use kaya_protocol::{Command, Frame};

use crate::handler::CommandHandler;
use crate::{CommandContext, CommandError};

/// Routes incoming commands to the appropriate handler.
pub struct CommandRouter {
    ctx: Arc<CommandContext>,
}

impl CommandRouter {
    pub fn new(ctx: Arc<CommandContext>) -> Self {
        Self { ctx }
    }

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

            // -- bloom filter commands -----------------------------------------
            "BF.ADD" => handler.bf_add(cmd),
            "BF.EXISTS" => handler.bf_exists(cmd),
            "BF.RESERVE" => handler.bf_reserve(cmd),
            "BF.MADD" => handler.bf_madd(cmd),
            "BF.MEXISTS" => handler.bf_mexists(cmd),

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
}
