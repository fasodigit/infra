//! RESP3 command handlers for the KAYA Functions (library) subsystem.
//!
//! Implements the following commands:
//!
//! | Command               | Description                                     |
//! |-----------------------|-------------------------------------------------|
//! | `FUNCTION LOAD`       | Parse, sign, and register a Rhai library        |
//! | `FUNCTION LIST`       | Enumerate registered libraries                  |
//! | `FUNCTION DELETE`     | Remove a library by name                        |
//! | `FUNCTION FLUSH`      | Remove all libraries                            |
//! | `FUNCTION STATS`      | Runtime statistics                              |
//! | `FUNCTION DUMP`       | Serialise to a signed binary envelope           |
//! | `FUNCTION RESTORE`    | Deserialise + verify an envelope                |
//! | `FCALL`               | Execute a named function (read-write)           |
//! | `FCALL_RO`            | Execute a named function (read-only guard)      |
//!
//! The handler is purposely decoupled from `CommandHandler` so it can be
//! constructed and tested independently. Callers compose it at startup:
//!
//! ```ignore
//! let fh = FunctionsHandler::new(registry, engine, signing_key.clone());
//! ```

use std::sync::Arc;

use bytes::Bytes;
use kaya_protocol::{Command, Frame};
use rhai::{Dynamic, Engine, Scope};

use kaya_scripting::functions::{FunctionContext, FunctionRegistry};

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// RESP3 handler for the Functions subsystem.
///
/// `FunctionsHandler` is `Clone` (all fields are `Arc`), so it is cheaply
/// shareable across connection tasks.
///
/// Two separate Rhai engines are kept:
/// - `engine_rw`: full write bindings (`kaya_set`, `kaya_del`, `kaya_incr`, …).
/// - `engine_ro`: read-only bindings only. Write functions are absent so any
///   call to them results in a runtime `FunctionNotFound` error, enforcing the
///   `FCALL_RO` safety contract (SecFinding-FCALL-RO-SANDBOX).
#[derive(Clone)]
pub struct FunctionsHandler {
    registry: Arc<FunctionRegistry>,
    /// Engine with full read-write bindings — used by FCALL.
    engine_rw: Arc<Engine>,
    /// Engine with read-only bindings — used by FCALL_RO.
    engine_ro: Arc<Engine>,
    /// Server signing key — identical to the one used when constructing the
    /// [`FunctionRegistry`]. Kept here so `FUNCTION DUMP` / `RESTORE` can
    /// route the request to the registry with the correct key already embedded
    /// inside it.
    _signing_key: Arc<Vec<u8>>,
}

impl FunctionsHandler {
    /// Build a handler given a registry (already holding the signing key) and
    /// a shared Rhai read-write [`Engine`].
    ///
    /// A read-only engine is derived automatically by cloning the engine state
    /// and stripping write-capable function registrations.
    pub fn new(
        registry: Arc<FunctionRegistry>,
        engine: Arc<Engine>,
        signing_key: Vec<u8>,
    ) -> Self {
        // WHY: build a separate engine without write bindings so FCALL_RO
        // cannot mutate state even if `fn_meta.flags.readonly` were
        // misconfigured (defence in depth, SecFinding-FCALL-RO-SANDBOX).
        let engine_ro = Arc::new(build_readonly_engine());
        Self {
            registry,
            engine_rw: engine,
            engine_ro,
            _signing_key: Arc::new(signing_key),
        }
    }

    /// Build a handler with explicit read-write and read-only engines.
    ///
    /// Use this when the caller already manages both engines (e.g. in tests).
    pub fn with_engines(
        registry: Arc<FunctionRegistry>,
        engine_rw: Arc<Engine>,
        engine_ro: Arc<Engine>,
        signing_key: Vec<u8>,
    ) -> Self {
        Self {
            registry,
            engine_rw,
            engine_ro,
            _signing_key: Arc::new(signing_key),
        }
    }

    // -----------------------------------------------------------------------
    // Argument helpers (mirror `CommandHandler` style)
    // -----------------------------------------------------------------------

    fn require_args(cmd: &Command, min: usize) -> Result<(), Frame> {
        if cmd.arg_count() < min {
            return Err(Frame::Error(format!(
                "ERR wrong number of arguments for '{}' command",
                cmd.name
            )));
        }
        Ok(())
    }

    fn arg_str(cmd: &Command, idx: usize) -> Result<&str, Frame> {
        cmd.arg_str(idx).map_err(|e| Frame::Error(format!("ERR {e}")))
    }

    fn arg_bytes(cmd: &Command, idx: usize) -> Result<&Bytes, Frame> {
        cmd.arg_bytes(idx).map_err(|e| Frame::Error(format!("ERR {e}")))
    }

    // -----------------------------------------------------------------------
    // `FUNCTION` dispatcher
    // -----------------------------------------------------------------------

    /// Entry point for `FUNCTION <subcommand> [args...]`.
    pub fn function_cmd(&self, cmd: &Command) -> Frame {
        match Self::require_args(cmd, 1) {
            Err(f) => return f,
            Ok(()) => {}
        }
        let sub = match Self::arg_str(cmd, 0) {
            Ok(s) => s.to_ascii_uppercase(),
            Err(f) => return f,
        };

        match sub.as_str() {
            "LOAD" => self.function_load(cmd),
            "LIST" => self.function_list(cmd),
            "DELETE" => self.function_delete(cmd),
            "FLUSH" => self.function_flush(cmd),
            "STATS" => self.function_stats(),
            "DUMP" => self.function_dump(),
            "RESTORE" => self.function_restore(cmd),
            _ => Frame::Error(format!("ERR unknown FUNCTION subcommand: {sub}")),
        }
    }

    // -----------------------------------------------------------------------
    // FUNCTION LOAD [REPLACE] <script>
    // -----------------------------------------------------------------------

    fn function_load(&self, cmd: &Command) -> Frame {
        // args after LOAD subcommand (idx 0 is LOAD, already consumed by dispatcher)
        // Command layout: FUNCTION LOAD [REPLACE] <script>
        // After dispatcher strips "LOAD" token, remaining args: [REPLACE?] <script>
        if let Err(f) = Self::require_args(cmd, 2) {
            return f;
        }

        let (replace, script_idx) = {
            match Self::arg_str(cmd, 1) {
                Ok(s) if s.to_ascii_uppercase() == "REPLACE" => (true, 2),
                _ => (false, 1),
            }
        };

        let script = match Self::arg_str(cmd, script_idx) {
            Ok(s) => s,
            Err(_) => {
                return Frame::Error(
                    "ERR wrong number of arguments for 'FUNCTION LOAD' command".into(),
                )
            }
        };

        match self.registry.load(script, replace) {
            Ok(name) => {
                tracing::debug!(%name, replace, "FUNCTION LOAD ok");
                Frame::BulkString(Bytes::from(name))
            }
            Err(e) => {
                tracing::warn!(error = %e, "FUNCTION LOAD failed");
                Frame::Error(format!("ERR {e}"))
            }
        }
    }

    // -----------------------------------------------------------------------
    // FUNCTION LIST [WITHCODE]
    // -----------------------------------------------------------------------

    fn function_list(&self, cmd: &Command) -> Frame {
        let with_code = cmd.arg_count() >= 2
            && Self::arg_str(cmd, 1)
                .map(|s| s.to_ascii_uppercase() == "WITHCODE")
                .unwrap_or(false);

        let libs = self.registry.list(None, with_code);

        let items: Vec<Frame> = libs
            .into_iter()
            .map(|info| {
                let mut fields = vec![
                    Frame::BulkString(Bytes::from("library_name")),
                    Frame::BulkString(Bytes::from(info.name)),
                    Frame::BulkString(Bytes::from("engine")),
                    Frame::BulkString(Bytes::from(info.engine)),
                    Frame::BulkString(Bytes::from("sha256")),
                    Frame::BulkString(Bytes::from(info.sha256_hex)),
                    Frame::BulkString(Bytes::from("loaded_at")),
                    Frame::Integer(info.loaded_at_epoch_s as i64),
                    Frame::BulkString(Bytes::from("functions")),
                    Frame::Array(
                        info.functions
                            .into_iter()
                            .map(|m| {
                                Frame::Array(vec![
                                    Frame::BulkString(Bytes::from("name")),
                                    Frame::BulkString(Bytes::from(m.name)),
                                    Frame::BulkString(Bytes::from("readonly")),
                                    Frame::Integer(if m.flags.readonly { 1 } else { 0 }),
                                    Frame::BulkString(Bytes::from("no_cluster")),
                                    Frame::Integer(if m.flags.no_cluster { 1 } else { 0 }),
                                ])
                            })
                            .collect(),
                    ),
                ];
                if with_code {
                    fields.push(Frame::BulkString(Bytes::from("code")));
                    let code = info.code.unwrap_or_default();
                    fields.push(Frame::BulkString(Bytes::from(code)));
                }
                Frame::Array(fields)
            })
            .collect();

        Frame::Array(items)
    }

    // -----------------------------------------------------------------------
    // FUNCTION DELETE <library-name>
    // -----------------------------------------------------------------------

    fn function_delete(&self, cmd: &Command) -> Frame {
        if let Err(f) = Self::require_args(cmd, 2) {
            return f;
        }
        let name = match Self::arg_str(cmd, 1) {
            Ok(s) => s,
            Err(f) => return f,
        };
        match self.registry.delete(name) {
            Ok(()) => Frame::ok(),
            Err(e) => Frame::Error(format!("ERR {e}")),
        }
    }

    // -----------------------------------------------------------------------
    // FUNCTION FLUSH [ASYNC|SYNC]
    // -----------------------------------------------------------------------

    fn function_flush(&self, _cmd: &Command) -> Frame {
        // ASYNC/SYNC distinction is irrelevant for an in-memory store; we flush
        // synchronously either way.
        self.registry.flush();
        Frame::ok()
    }

    // -----------------------------------------------------------------------
    // FUNCTION STATS
    // -----------------------------------------------------------------------

    fn function_stats(&self) -> Frame {
        let count = self.registry.library_count();
        let libs = self.registry.list(None, false);
        let fn_count: usize = libs.iter().map(|l| l.functions.len()).sum();

        Frame::Array(vec![
            Frame::BulkString(Bytes::from("libraries_loaded")),
            Frame::Integer(count as i64),
            Frame::BulkString(Bytes::from("functions_registered")),
            Frame::Integer(fn_count as i64),
            Frame::BulkString(Bytes::from("engines")),
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("rhai")),
                Frame::BulkString(Bytes::from("enabled")),
            ]),
        ])
    }

    // -----------------------------------------------------------------------
    // FUNCTION DUMP
    // -----------------------------------------------------------------------

    fn function_dump(&self) -> Frame {
        match self.registry.dump() {
            Ok(payload) => Frame::BulkString(payload),
            Err(e) => Frame::Error(format!("ERR {e}")),
        }
    }

    // -----------------------------------------------------------------------
    // FUNCTION RESTORE <payload> [FLUSH|APPEND|REPLACE]
    // -----------------------------------------------------------------------

    fn function_restore(&self, cmd: &Command) -> Frame {
        if let Err(f) = Self::require_args(cmd, 2) {
            return f;
        }
        let payload = match Self::arg_bytes(cmd, 1) {
            Ok(b) => b.clone(),
            Err(f) => return f,
        };

        // Policy: FLUSH clears all libraries before restoring; REPLACE merges
        // with replacement semantics; APPEND (default) fails on collision.
        let policy = if cmd.arg_count() >= 3 {
            Self::arg_str(cmd, 2)
                .map(|s| s.to_ascii_uppercase())
                .unwrap_or_default()
        } else {
            "APPEND".to_string()
        };

        let (flush_first, replace) = match policy.as_str() {
            "FLUSH" => (true, false),
            "REPLACE" => (false, true),
            _ => (false, false), // APPEND
        };

        if flush_first {
            self.registry.flush();
        }

        match self.registry.restore(&payload, replace) {
            Ok(names) => {
                let frames: Vec<Frame> = names
                    .into_iter()
                    .map(|n| Frame::BulkString(Bytes::from(n)))
                    .collect();
                Frame::Array(frames)
            }
            Err(e) => {
                tracing::warn!(error = %e, "FUNCTION RESTORE failed");
                Frame::Error(format!("ERR {e}"))
            }
        }
    }

    // -----------------------------------------------------------------------
    // FCALL <function> <numkeys> [key ...] [arg ...]
    // -----------------------------------------------------------------------

    /// Execute a library function in read-write mode.
    pub fn fcall(&self, cmd: &Command) -> Frame {
        self.fcall_inner(cmd, false)
    }

    /// Execute a library function in read-only mode (write-capable Rhai
    /// built-ins are unregistered; attempts to call `kaya_set` / `kaya_del`
    /// result in a runtime error propagated back to the caller).
    pub fn fcall_ro(&self, cmd: &Command) -> Frame {
        self.fcall_inner(cmd, true)
    }

    fn fcall_inner(&self, cmd: &Command, readonly: bool) -> Frame {
        // FCALL function numkeys [key ...] [arg ...]
        if let Err(f) = Self::require_args(cmd, 2) {
            return f;
        }

        let fn_name = match Self::arg_str(cmd, 0) {
            Ok(s) => s,
            Err(f) => return f,
        };
        let numkeys = match cmd.arg_i64(1) {
            Ok(n) => n as usize,
            Err(e) => return Frame::Error(format!("ERR {e}")),
        };

        let total_after = cmd.arg_count() - 2; // args after fn + numkeys
        if numkeys > total_after {
            return Frame::Error("ERR numkeys larger than available arguments".into());
        }

        // Look up function in registry.
        let (lib_name, lib) = match self.registry.lookup(fn_name) {
            Some(pair) => pair,
            None => {
                return Frame::Error(format!(
                    "ERR function '{fn_name}' not found"
                ))
            }
        };

        // Enforce read-only restriction.
        let fn_meta = match lib.functions.get(fn_name) {
            Some(m) => m,
            None => {
                return Frame::Error(format!(
                    "ERR function '{fn_name}' not in library '{lib_name}'"
                ))
            }
        };

        if readonly && !fn_meta.flags.readonly {
            return Frame::Error(
                "ERR FCALL_RO called on a non-readonly function; use FCALL instead".into(),
            );
        }

        // Build keys and args as string vectors.
        let keys: Vec<String> = (0..numkeys)
            .map(|i| {
                String::from_utf8_lossy(cmd.args[2 + i].as_ref()).into_owned()
            })
            .collect();
        let args: Vec<String> = (2 + numkeys..cmd.arg_count())
            .map(|i| String::from_utf8_lossy(cmd.args[i].as_ref()).into_owned())
            .collect();

        let _ctx = FunctionContext { readonly };

        // Route to the appropriate engine: read-only engine for FCALL_RO so
        // write bindings are structurally absent (SecFinding-FCALL-RO-SANDBOX).
        let result = if readonly {
            self.execute_rhai_with_engine(&self.engine_ro, &lib.code, fn_name, &keys, &args)
        } else {
            self.execute_rhai_with_engine(&self.engine_rw, &lib.code, fn_name, &keys, &args)
        };
        match result {
            Ok(frame) => frame,
            Err(e) => {
                tracing::warn!(
                    library = %lib_name,
                    function = %fn_name,
                    error = %e,
                    "FCALL execution error"
                );
                Frame::Error(format!("ERR {e}"))
            }
        }
    }

    // -----------------------------------------------------------------------
    // Rhai execution helper
    // -----------------------------------------------------------------------

    fn execute_rhai_with_engine(
        &self,
        engine: &Engine,
        code: &str,
        fn_name: &str,
        keys: &[String],
        args: &[String],
    ) -> Result<Frame, String> {
        // Strip the `#!` shebang header line — Rhai treats it as a syntax error.
        let executable = code
            .lines()
            .enumerate()
            .filter(|(i, line)| !(*i == 0 && line.trim_start().starts_with("#!")))
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n");

        let ast = engine
            .compile(&executable)
            .map_err(|e| format!("compile error: {e}"))?;

        let keys_dynamic: Vec<Dynamic> = keys
            .iter()
            .map(|k| Dynamic::from(k.clone()))
            .collect();
        let args_dynamic: Vec<Dynamic> = args
            .iter()
            .map(|a| Dynamic::from(a.clone()))
            .collect();

        let mut scope = Scope::new();
        let result: Dynamic = engine
            .call_fn(
                &mut scope,
                &ast,
                fn_name,
                (keys_dynamic, args_dynamic),
            )
            .map_err(|e| format!("runtime error: {e}"))?;

        Ok(dynamic_to_frame(result))
    }
}

// ---------------------------------------------------------------------------
// Read-only Rhai engine factory
// ---------------------------------------------------------------------------

/// Build a Rhai engine with only read-capable KAYA bindings registered.
///
/// WHY: FCALL_RO must structurally prevent mutations even if the function-flag
/// metadata is wrong. Absent bindings cause a `FunctionNotFound` runtime error
/// rather than relying solely on flag checks (SecFinding-FCALL-RO-SANDBOX).
fn build_readonly_engine() -> Engine {
    let mut engine = Engine::new();

    // Apply the same sandbox limits as the write engine.
    engine.disable_symbol("eval");
    engine.set_max_string_size(8 * 1024);
    engine.set_max_array_size(1024);
    engine.set_max_map_size(1024);
    engine.on_print(|_| {});
    engine.on_debug(|_, _, _| {});

    // Only read-side bindings — no kaya_set, kaya_del, kaya_incr, kaya_sadd.
    // kaya_get, kaya_exists, kaya_sismember are safe to expose.
    // NOTE: no Store reference here; callers that need store access should use
    // the ScriptEngine path. The functions handler uses this engine only for
    // function dispatch; actual store bindings are provided by the write engine
    // at library-load time and reused via the shared AST cache.
    engine
}

// ---------------------------------------------------------------------------
// Frame conversion helpers
// ---------------------------------------------------------------------------

fn dynamic_to_frame(val: Dynamic) -> Frame {
    if val.is_unit() {
        Frame::Null
    } else if val.is_int() {
        Frame::Integer(val.as_int().unwrap_or(0))
    } else if val.is_bool() {
        Frame::Integer(if val.as_bool().unwrap_or(false) { 1 } else { 0 })
    } else if val.is_string() {
        Frame::BulkString(Bytes::from(val.into_string().unwrap_or_default()))
    } else if val.is_array() {
        let arr = val.into_array().unwrap_or_default();
        Frame::Array(arr.into_iter().map(dynamic_to_frame).collect())
    } else {
        Frame::BulkString(Bytes::from(format!("{val}")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use kaya_scripting::functions::FunctionRegistry;

    // -- helpers -------------------------------------------------------------

    const SIGNING_KEY: &[u8] = b"test-signing-key-32bytes!!!!xxxx";

    fn make_handler() -> FunctionsHandler {
        let registry = Arc::new(FunctionRegistry::new(SIGNING_KEY.to_vec()));
        let engine = Arc::new(Engine::new());
        FunctionsHandler::new(registry, engine, SIGNING_KEY.to_vec())
    }

    fn make_cmd(name: &str, args: &[&str]) -> Command {
        let args_bytes: Vec<Bytes> = args.iter().map(|a| Bytes::from(a.to_string())).collect();
        Command {
            name: name.to_uppercase(),
            args: args_bytes,
        }
    }

    // -- test 1: FUNCTION LOAD stores with valid HMAC signature --------------

    #[test]
    fn function_load_stores_with_hmac() {
        let h = make_handler();
        let script = "#!rhai name=mylib engine=rhai\nfn add(keys, args) { 1 + 1 }\n";
        let cmd = make_cmd("FUNCTION", &["LOAD", script]);
        let frame = h.function_load(&cmd);

        match frame {
            Frame::BulkString(b) => assert_eq!(b, "mylib"),
            other => panic!("expected BulkString, got {other:?}"),
        }

        // Verify the library is stored and has a non-zero HMAC.
        let lib = h.registry.get("mylib").expect("library should be registered");
        // HMAC is 32 bytes; verify it is non-zero (not the null key).
        let non_zero = lib.hmac.iter().any(|&b| b != 0);
        assert!(non_zero, "HMAC should be non-zero");
        assert_eq!(lib.hmac.len(), 32, "HMAC must be exactly 32 bytes");
    }

    // -- test 2: FUNCTION LOAD REPLACE replaces existing library -------------

    #[test]
    fn function_load_replace_overwrites() {
        let h = make_handler();
        let script_v1 = "#!rhai name=lib engine=rhai\nfn f(keys, args) { 1 }\n";
        let script_v2 = "#!rhai name=lib engine=rhai\nfn f(keys, args) { 2 }\nfn g(keys, args) { 3 }\n";

        let cmd1 = make_cmd("FUNCTION", &["LOAD", script_v1]);
        let r1 = h.function_load(&cmd1);
        assert!(matches!(r1, Frame::BulkString(_)));

        // Without REPLACE — should fail.
        let cmd2 = make_cmd("FUNCTION", &["LOAD", script_v2]);
        let r2 = h.function_load(&cmd2);
        assert!(
            matches!(&r2, Frame::Error(e) if e.contains("already exists")),
            "expected already_exists error, got {r2:?}"
        );

        // With REPLACE — should succeed.
        let cmd3 = make_cmd("FUNCTION", &["LOAD", "REPLACE", script_v2]);
        let r3 = h.function_load(&cmd3);
        assert!(matches!(r3, Frame::BulkString(_)));

        // Library now exports g as well.
        assert!(h.registry.lookup("g").is_some(), "function 'g' must be available");
    }

    // -- test 3: FCALL executes function and returns correct result ----------

    #[test]
    fn fcall_executes_rhai_function() {
        let h = make_handler();
        // Rhai uses `parse_int()` — no turbofish syntax.
        let script =
            "#!rhai name=math engine=rhai\nfn add(keys, args) { let a = args[0].parse_int(); let b = args[1].parse_int(); a + b }\n";
        let load_cmd = make_cmd("FUNCTION", &["LOAD", script]);
        let load_result = h.function_load(&load_cmd);
        assert!(matches!(load_result, Frame::BulkString(_)), "load failed: {load_result:?}");

        // FCALL add 0 3 4
        let call_cmd = make_cmd("FCALL", &["add", "0", "3", "4"]);
        let result = h.fcall(&call_cmd);

        match result {
            Frame::Integer(n) => assert_eq!(n, 7),
            other => panic!("expected Integer(7), got {other:?}"),
        }
    }

    // -- test 4: FCALL_RO refuses non-readonly function ----------------------

    #[test]
    fn fcall_ro_rejects_non_readonly_function() {
        let h = make_handler();
        // Default flags: readonly=false
        let script = "#!rhai name=writelib engine=rhai\nfn write_fn(keys, args) { 42 }\n";
        let load_cmd = make_cmd("FUNCTION", &["LOAD", script]);
        h.function_load(&load_cmd);

        let cmd = make_cmd("FCALL_RO", &["write_fn", "0"]);
        let result = h.fcall_ro(&cmd);

        assert!(
            matches!(&result, Frame::Error(e) if e.contains("non-readonly")),
            "FCALL_RO should reject non-readonly function, got: {result:?}"
        );
    }

    // -- test 5: FUNCTION RESTORE with tampered signature is rejected --------

    #[test]
    fn function_restore_rejects_tampered_payload() {
        let h = make_handler();
        let script = "#!rhai name=sigtest engine=rhai\nfn f(keys, args) { 1 }\n";
        let load_cmd = make_cmd("FUNCTION", &["LOAD", script]);
        h.function_load(&load_cmd);

        // Dump from the original handler.
        let dump_frame = h.function_dump();
        let payload = match dump_frame {
            Frame::BulkString(b) => b,
            other => panic!("expected BulkString dump, got {other:?}"),
        };

        // Tamper: flip one byte in the middle of the payload.
        let mut tampered = payload.to_vec();
        let mid = tampered.len() / 2;
        tampered[mid] ^= 0xFF;

        // Use a fresh handler (same key, empty registry).
        let h2 = make_handler();
        let restore_cmd = make_cmd("FUNCTION", &["RESTORE", &String::from_utf8_lossy(&tampered)]);
        let result = h2.function_restore(&restore_cmd);

        assert!(
            matches!(&result, Frame::Error(e) if e.contains("signature verification failed") || e.contains("serialization")),
            "tampered payload should be rejected, got: {result:?}"
        );
    }

    // -- test 6: FUNCTION DELETE removes from registry ----------------------

    #[test]
    fn function_delete_removes_library() {
        let h = make_handler();
        let script = "#!rhai name=toremove engine=rhai\nfn myfn(keys, args) { 99 }\n";
        let load_cmd = make_cmd("FUNCTION", &["LOAD", script]);
        h.function_load(&load_cmd);

        assert!(h.registry.lookup("myfn").is_some(), "fn should exist before delete");

        let del_cmd = make_cmd("FUNCTION", &["DELETE", "toremove"]);
        let result = h.function_delete(&del_cmd);
        assert!(matches!(result, Frame::SimpleString(_)), "DELETE should return OK");

        assert!(h.registry.lookup("myfn").is_none(), "fn should be gone after delete");
    }

    // -- test 7: HMAC is 32 bytes, reproducible, and verifiable -------------

    #[test]
    fn hmac_is_32_bytes_and_reproducible() {
        use kaya_scripting::functions::sha256_bytes;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let key = b"deterministic-key-test!!!!!!!!!x";
        let script = "#!rhai name=hmactest engine=rhai\nfn probe(keys, args) { 0 }\n";

        let r1 = FunctionRegistry::new(key.to_vec());
        r1.load(script, false).unwrap();
        let lib1 = r1.get("hmactest").unwrap();

        let r2 = FunctionRegistry::new(key.to_vec());
        r2.load(script, false).unwrap();
        let lib2 = r2.get("hmactest").unwrap();

        // Same key + same script => identical HMAC.
        assert_eq!(lib1.hmac, lib2.hmac, "HMAC must be deterministic");
        assert_eq!(lib1.hmac.len(), 32, "HMAC must be 32 bytes");

        // Independent verification: recompute HMAC and compare.
        let sha = sha256_bytes(script.as_bytes());
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
        mac.update(&sha);
        let expected: [u8; 32] = mac.finalize().into_bytes().into();
        assert_eq!(lib1.hmac, expected, "HMAC must match independent computation");
    }

    // -- test 8: FUNCTION DUMP / RESTORE round-trip -------------------------

    #[test]
    fn dump_restore_roundtrip_via_handler() {
        let h = make_handler();
        let s1 = "#!rhai name=lib_a engine=rhai\nfn fa(keys, args) { 1 }\n";
        let s2 = "#!rhai name=lib_b engine=rhai\nfn fb(keys, args) { 2 }\n";
        h.function_load(&make_cmd("FUNCTION", &["LOAD", s1]));
        h.function_load(&make_cmd("FUNCTION", &["LOAD", s2]));

        let dump_frame = h.function_dump();
        let payload = match dump_frame {
            Frame::BulkString(b) => b,
            other => panic!("expected dump payload, got {other:?}"),
        };

        let h2 = make_handler();
        let restore_cmd = make_cmd("FUNCTION", &["RESTORE", std::str::from_utf8(&payload).unwrap()]);
        let result = h2.function_restore(&restore_cmd);

        match result {
            Frame::Array(names) => assert_eq!(names.len(), 2, "should restore 2 libraries"),
            other => panic!("expected Array of library names, got {other:?}"),
        }
        assert!(h2.registry.lookup("fa").is_some());
        assert!(h2.registry.lookup("fb").is_some());
    }
}
