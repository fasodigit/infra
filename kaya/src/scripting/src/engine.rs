//! Rhai script engine with KAYA store bindings.

use std::sync::Arc;

use rhai::{Engine, Scope, AST};

use crate::{ScriptConfig, ScriptError, ScriptResult};
use crate::cache::ScriptCache;
use kaya_store::Store;

/// The KAYA script engine powered by Rhai.
pub struct ScriptEngine {
    engine: Engine,
    cache: ScriptCache,
    config: ScriptConfig,
}

impl ScriptEngine {
    pub fn new(config: ScriptConfig, store: Arc<Store>) -> Self {
        let mut engine = Engine::new();

        // Set execution limits.
        engine.set_max_operations(config.max_execution_ms as u64 * 1000);

        // Register KAYA store functions available to scripts.
        let store_get = store.clone();
        engine.register_fn("kaya_get", move |key: &str| -> rhai::Dynamic {
            match store_get.get(key.as_bytes()) {
                Ok(Some(val)) => {
                    String::from_utf8(val.to_vec())
                        .map(rhai::Dynamic::from)
                        .unwrap_or(rhai::Dynamic::UNIT)
                }
                _ => rhai::Dynamic::UNIT,
            }
        });

        let store_set = store.clone();
        engine.register_fn("kaya_set", move |key: &str, value: &str| -> bool {
            store_set.set(key.as_bytes(), value.as_bytes(), None).is_ok()
        });

        let store_del = store.clone();
        engine.register_fn("kaya_del", move |key: &str| -> i64 {
            store_del.del(&[key.as_bytes()]) as i64
        });

        let store_exists = store.clone();
        engine.register_fn("kaya_exists", move |key: &str| -> bool {
            store_exists.exists(&[key.as_bytes()]) > 0
        });

        let store_incr = store.clone();
        engine.register_fn("kaya_incr", move |key: &str| -> i64 {
            store_incr.incr(key.as_bytes()).unwrap_or(0)
        });

        let store_sadd = store.clone();
        engine.register_fn("kaya_sadd", move |key: &str, member: &str| -> i64 {
            store_sadd
                .sadd(key.as_bytes(), &[member.as_bytes()])
                .unwrap_or(0) as i64
        });

        let store_sismember = store.clone();
        engine.register_fn(
            "kaya_sismember",
            move |key: &str, member: &str| -> bool {
                store_sismember.sismember(key.as_bytes(), member.as_bytes())
            },
        );

        Self {
            engine,
            cache: ScriptCache::new(config.cache_size),
            config,
        }
    }

    /// Evaluate a Rhai script with the given keys and args.
    pub fn eval(
        &self,
        script: &str,
        keys: &[String],
        args: &[String],
    ) -> Result<ScriptResult, ScriptError> {
        let ast = self.compile_or_cache(script)?;

        let mut scope = Scope::new();

        // Provide KEYS and ARGV arrays to the script.
        let keys_arr: Vec<rhai::Dynamic> = keys.iter().map(|k| rhai::Dynamic::from(k.clone())).collect();
        let args_arr: Vec<rhai::Dynamic> = args.iter().map(|a| rhai::Dynamic::from(a.clone())).collect();
        scope.push("KEYS", keys_arr);
        scope.push("ARGV", args_arr);

        let result = self
            .engine
            .eval_ast_with_scope::<rhai::Dynamic>(&mut scope, &ast)
            .map_err(|e| ScriptError::Execution(e.to_string()))?;

        Ok(dynamic_to_result(result))
    }

    /// Load a script by SHA (from cache).
    pub fn eval_sha(
        &self,
        sha: &str,
        keys: &[String],
        args: &[String],
    ) -> Result<ScriptResult, ScriptError> {
        let ast = self
            .cache
            .get(sha)
            .ok_or_else(|| ScriptError::NotFound(sha.into()))?;

        let mut scope = Scope::new();
        let keys_arr: Vec<rhai::Dynamic> = keys.iter().map(|k| rhai::Dynamic::from(k.clone())).collect();
        let args_arr: Vec<rhai::Dynamic> = args.iter().map(|a| rhai::Dynamic::from(a.clone())).collect();
        scope.push("KEYS", keys_arr);
        scope.push("ARGV", args_arr);

        let result = self
            .engine
            .eval_ast_with_scope::<rhai::Dynamic>(&mut scope, &ast)
            .map_err(|e| ScriptError::Execution(e.to_string()))?;

        Ok(dynamic_to_result(result))
    }

    /// Compile and cache a script, returning its SHA.
    pub fn load(&self, script: &str) -> Result<String, ScriptError> {
        let ast = self
            .engine
            .compile(script)
            .map_err(|e| ScriptError::Compilation(e.to_string()))?;
        let sha = self.cache.insert(script, ast);
        Ok(sha)
    }

    fn compile_or_cache(&self, script: &str) -> Result<AST, ScriptError> {
        let sha = ScriptCache::sha(script);
        if let Some(ast) = self.cache.get(&sha) {
            return Ok(ast);
        }
        let ast = self
            .engine
            .compile(script)
            .map_err(|e| ScriptError::Compilation(e.to_string()))?;
        self.cache.insert(script, ast.clone());
        Ok(ast)
    }
}

/// Convert a Rhai Dynamic value to a ScriptResult.
fn dynamic_to_result(val: rhai::Dynamic) -> ScriptResult {
    if val.is_unit() {
        ScriptResult::Nil
    } else if val.is_int() {
        ScriptResult::Integer(val.as_int().unwrap_or(0))
    } else if val.is_bool() {
        ScriptResult::Bool(val.as_bool().unwrap_or(false))
    } else if val.is_string() {
        ScriptResult::Str(val.into_string().unwrap_or_default())
    } else if val.is_array() {
        let arr = val.into_array().unwrap_or_default();
        ScriptResult::Array(arr.into_iter().map(dynamic_to_result).collect())
    } else {
        ScriptResult::Str(format!("{val}"))
    }
}
