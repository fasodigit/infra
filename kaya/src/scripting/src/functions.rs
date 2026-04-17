//! KAYA Functions library runtime — a sovereign persistent scripting facility
//! inspired by the RESP3 function model.
//!
//! A *library* is a named bundle of Rhai functions shipped with a mandatory
//! `#!rhai name=<lib> engine=rhai` shebang. Libraries are:
//!
//! * signed with HMAC-SHA-256 at `LOAD` time using a server-side key (loaded
//!   from Vault / env var);
//! * stored in-memory via a [`FunctionRegistry`];
//! * invoked by name with `FCALL <fn> <numkeys> <keys...> <args...>`;
//! * serialisable through `DUMP` / `RESTORE` for Raft replication or backup.
//!
//! The module deliberately exposes *no* `unwrap` on runtime paths; every
//! fallible operation returns a [`FunctionError`].

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use dashmap::DashMap;
use hmac::{Hmac, Mac};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::FunctionError;
use crate::parser::{extract_functions, parse_header};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Execution engine a library targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineKind {
    Rhai,
    Wasm,
    LuaCompat,
}

impl EngineKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rhai => "rhai",
            Self::Wasm => "wasm",
            Self::LuaCompat => "lua",
        }
    }
}

/// Per-function flags used by the router to enforce safety properties.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionFlags {
    /// The function only reads data — callable through `FCALL_RO`.
    pub readonly: bool,
    /// The function must not be routed to another shard.
    pub no_cluster: bool,
    /// Callable from a stale replica.
    pub allow_stale: bool,
}

/// Metadata exposed for a single function inside a library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMeta {
    pub name: String,
    pub flags: FunctionFlags,
}

/// A fully registered library, signed and ready to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionLibrary {
    pub name: String,
    pub engine: EngineKind,
    pub code: String,
    /// SHA-256 of `code` (integrity).
    pub sha256: [u8; 32],
    /// HMAC-SHA-256(hmac_key, sha256) proving the library was loaded by a
    /// principal that owned the server signing key (authenticity).
    pub hmac: [u8; 32],
    pub functions: HashMap<String, FunctionMeta>,
    /// Unix epoch seconds of the last successful `LOAD` / `RESTORE`.
    pub loaded_at_epoch_s: u64,
}

/// Public summary surfaced by `FUNCTION LIST`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
    pub name: String,
    pub engine: String,
    pub functions: Vec<FunctionMeta>,
    pub sha256_hex: String,
    pub loaded_at_epoch_s: u64,
    pub code: Option<String>,
}

/// Result shape returned by [`FunctionRegistry::call`].
#[derive(Debug, Clone)]
pub enum FunctionResult {
    Nil,
    Integer(i64),
    Str(String),
    Bytes(Bytes),
    Bool(bool),
    Array(Vec<FunctionResult>),
    Error(String),
}

/// Context passed into the execution path. Right now it only carries the
/// readonly marker; future revisions will also carry ACL principals, the
/// active shard, etc.
#[derive(Debug, Clone, Copy, Default)]
pub struct FunctionContext {
    pub readonly: bool,
}

// ---------------------------------------------------------------------------
// Raft hook
// ---------------------------------------------------------------------------

/// Replication operations emitted by the function layer. Consumers (the Raft
/// log, or a local direct-apply path) translate each variant into a state
/// transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FunctionOp {
    Load(String),
    Delete(String),
    Flush,
}

// ---------------------------------------------------------------------------
// Wire format for DUMP / RESTORE
// ---------------------------------------------------------------------------

/// On-the-wire container for `FUNCTION DUMP`. The outer HMAC covers the JSON
/// body which itself contains per-library HMACs (defence in depth).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DumpEnvelope {
    version: u8,
    libraries: Vec<FunctionLibrary>,
    /// HMAC-SHA-256 over the canonical JSON of `libraries`.
    envelope_hmac: [u8; 32],
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

type HmacSha256 = Hmac<Sha256>;

/// Thread-safe registry of loaded libraries.
pub struct FunctionRegistry {
    libraries: DashMap<String, Arc<FunctionLibrary>>,
    /// function name -> owning library name.
    fn_index: DashMap<String, String>,
    hmac_key: RwLock<Vec<u8>>,
}

impl FunctionRegistry {
    /// Create a registry bound to an HMAC signing key (typically loaded from
    /// Vault or an environment variable at startup).
    pub fn new(hmac_key: Vec<u8>) -> Self {
        Self {
            libraries: DashMap::new(),
            fn_index: DashMap::new(),
            hmac_key: RwLock::new(hmac_key),
        }
    }

    /// Rotate the HMAC signing key. Existing libraries keep their previous
    /// signatures; a subsequent `LOAD` or `RESTORE` will use the new key.
    pub fn rotate_key(&self, new_key: Vec<u8>) {
        *self.hmac_key.write() = new_key;
    }

    /// Number of libraries currently registered.
    pub fn library_count(&self) -> usize {
        self.libraries.len()
    }

    // -----------------------------------------------------------------------
    // LOAD
    // -----------------------------------------------------------------------

    /// Parse, sign, and register a library. Returns the library name on
    /// success.
    #[tracing::instrument(level = "debug", skip(self, code))]
    pub fn load(&self, code: &str, replace: bool) -> Result<String, FunctionError> {
        let header = parse_header(code)?;
        let metas = extract_functions(code)?;

        if self.libraries.contains_key(&header.name) && !replace {
            return Err(FunctionError::LibraryAlreadyExists(header.name));
        }

        let sha = sha256_bytes(code.as_bytes());
        let mac = self.compute_hmac(&sha)?;

        let mut functions = HashMap::with_capacity(metas.len());
        for m in metas {
            // Reject duplicate function names across libraries unless the
            // duplicate is in the very library we're replacing.
            if let Some(owner) = self.fn_index.get(&m.name) {
                if owner.value() != &header.name {
                    return Err(FunctionError::InvalidLibraryName(format!(
                        "function '{}' already owned by library '{}'",
                        m.name,
                        owner.value()
                    )));
                }
            }
            functions.insert(m.name.clone(), m);
        }

        let lib = Arc::new(FunctionLibrary {
            name: header.name.clone(),
            engine: header.engine,
            code: code.to_string(),
            sha256: sha,
            hmac: mac,
            functions,
            loaded_at_epoch_s: now_epoch_s(),
        });

        // On replace, remove the old fn_index entries first.
        if let Some(old) = self.libraries.insert(header.name.clone(), lib.clone()) {
            for fname in old.functions.keys() {
                self.fn_index.remove(fname);
            }
        }

        for fname in lib.functions.keys() {
            self.fn_index
                .insert(fname.clone(), header.name.clone());
        }

        tracing::info!(name = %header.name, engine = ?header.engine, fns = lib.functions.len(), "function library loaded");
        Ok(header.name)
    }

    // -----------------------------------------------------------------------
    // LIST
    // -----------------------------------------------------------------------

    /// List registered libraries, optionally filtering by exact name and
    /// optionally embedding the source code.
    pub fn list(&self, filter: Option<&str>, with_code: bool) -> Vec<LibraryInfo> {
        self.libraries
            .iter()
            .filter(|e| filter.map(|f| f == e.key()).unwrap_or(true))
            .map(|e| {
                let l = e.value();
                LibraryInfo {
                    name: l.name.clone(),
                    engine: l.engine.as_str().into(),
                    functions: l.functions.values().cloned().collect(),
                    sha256_hex: hex_lower(&l.sha256),
                    loaded_at_epoch_s: l.loaded_at_epoch_s,
                    code: if with_code { Some(l.code.clone()) } else { None },
                }
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // DELETE
    // -----------------------------------------------------------------------

    /// Remove a library and its exported functions from the indexes.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn delete(&self, name: &str) -> Result<(), FunctionError> {
        let (_, lib) = self
            .libraries
            .remove(name)
            .ok_or_else(|| FunctionError::LibraryNotFound(name.to_string()))?;
        for fname in lib.functions.keys() {
            self.fn_index.remove(fname);
        }
        tracing::info!(%name, "function library deleted");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // FLUSH
    // -----------------------------------------------------------------------

    /// Remove every library. Always succeeds.
    pub fn flush(&self) {
        self.libraries.clear();
        self.fn_index.clear();
        tracing::info!("function libraries flushed");
    }

    // -----------------------------------------------------------------------
    // DUMP
    // -----------------------------------------------------------------------

    /// Serialise every library into a signed binary envelope.
    #[tracing::instrument(level = "debug", skip(self))]
    pub fn dump(&self) -> Result<Bytes, FunctionError> {
        let mut libs: Vec<FunctionLibrary> =
            self.libraries.iter().map(|e| (**e.value()).clone()).collect();
        libs.sort_by(|a, b| a.name.cmp(&b.name));

        let body =
            serde_json::to_vec(&libs).map_err(|e| FunctionError::SerializationError(e.to_string()))?;
        let envelope_hmac = self.compute_hmac(&body)?;

        let envelope = DumpEnvelope {
            version: 1,
            libraries: libs,
            envelope_hmac,
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|e| FunctionError::SerializationError(e.to_string()))?;
        Ok(Bytes::from(bytes))
    }

    // -----------------------------------------------------------------------
    // RESTORE
    // -----------------------------------------------------------------------

    /// Load libraries from a previously produced dump. Verifies the envelope
    /// HMAC and each library's per-entry HMAC before installing anything.
    #[tracing::instrument(level = "debug", skip(self, dump))]
    pub fn restore(&self, dump: &[u8], replace: bool) -> Result<Vec<String>, FunctionError> {
        let envelope: DumpEnvelope = serde_json::from_slice(dump)
            .map_err(|e| FunctionError::SerializationError(e.to_string()))?;

        // Verify outer HMAC over the canonical serialisation of `libraries`.
        let body = serde_json::to_vec(&envelope.libraries)
            .map_err(|e| FunctionError::SerializationError(e.to_string()))?;
        let expected = self.compute_hmac(&body)?;
        if !constant_time_eq(&expected, &envelope.envelope_hmac) {
            return Err(FunctionError::SignatureMismatch);
        }

        // Verify each per-library signature.
        for lib in &envelope.libraries {
            let sha = sha256_bytes(lib.code.as_bytes());
            if !constant_time_eq(&sha, &lib.sha256) {
                return Err(FunctionError::SignatureMismatch);
            }
            let mac = self.compute_hmac(&sha)?;
            if !constant_time_eq(&mac, &lib.hmac) {
                return Err(FunctionError::SignatureMismatch);
            }
        }

        // Pre-check: if replace=false and any name collides, abort before any
        // mutation so the registry stays coherent.
        if !replace {
            for lib in &envelope.libraries {
                if self.libraries.contains_key(&lib.name) {
                    return Err(FunctionError::LibraryAlreadyExists(lib.name.clone()));
                }
            }
        }

        let mut names = Vec::with_capacity(envelope.libraries.len());
        for lib in envelope.libraries {
            let name = lib.name.clone();

            // Replace path: drop index entries of the library being replaced.
            if let Some(old) = self.libraries.insert(name.clone(), Arc::new(lib.clone())) {
                for fname in old.functions.keys() {
                    self.fn_index.remove(fname);
                }
            }
            for fname in lib.functions.keys() {
                self.fn_index.insert(fname.clone(), name.clone());
            }
            names.push(name);
        }

        tracing::info!(count = names.len(), "function libraries restored");
        Ok(names)
    }

    // -----------------------------------------------------------------------
    // Lookup
    // -----------------------------------------------------------------------

    /// Resolve `fn_name` to its owning library, if any.
    pub fn lookup(&self, fn_name: &str) -> Option<(String, Arc<FunctionLibrary>)> {
        let owner = self.fn_index.get(fn_name)?.value().clone();
        let lib = self.libraries.get(&owner)?.value().clone();
        Some((owner, lib))
    }

    /// Get a library by name.
    pub fn get(&self, name: &str) -> Option<Arc<FunctionLibrary>> {
        self.libraries.get(name).map(|e| e.value().clone())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn compute_hmac(&self, message: &[u8]) -> Result<[u8; 32], FunctionError> {
        let key = self.hmac_key.read();
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&key)
            .map_err(|e| FunctionError::SerializationError(format!("hmac key: {e}")))?;
        mac.update(message);
        let result = mac.finalize().into_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

pub fn sha256_bytes(message: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(message);
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn now_epoch_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn reg() -> FunctionRegistry {
        FunctionRegistry::new(b"unit-test-key".to_vec())
    }

    #[test]
    fn load_and_list() {
        let r = reg();
        let src = "#!rhai name=l engine=rhai\nfn a(keys, args) { 1 }\n";
        let name = r.load(src, false).unwrap();
        assert_eq!(name, "l");
        let libs = r.list(None, false);
        assert_eq!(libs.len(), 1);
    }

    #[test]
    fn load_duplicate_rejected() {
        let r = reg();
        let src = "#!rhai name=l engine=rhai\nfn a(keys, args) { 1 }\n";
        r.load(src, false).unwrap();
        assert!(matches!(
            r.load(src, false),
            Err(FunctionError::LibraryAlreadyExists(_))
        ));
    }

    #[test]
    fn delete_removes_fn_index() {
        let r = reg();
        r.load("#!rhai name=l engine=rhai\nfn a(keys, args) { 1 }\n", false)
            .unwrap();
        assert!(r.lookup("a").is_some());
        r.delete("l").unwrap();
        assert!(r.lookup("a").is_none());
    }

    #[test]
    fn dump_restore_roundtrip() {
        let r = reg();
        r.load("#!rhai name=a engine=rhai\nfn f1(keys, args) { 1 }\n", false)
            .unwrap();
        r.load("#!rhai name=b engine=rhai\nfn f2(keys, args) { 2 }\n", false)
            .unwrap();
        let blob = r.dump().unwrap();

        let r2 = reg();
        let restored = r2.restore(&blob, false).unwrap();
        assert_eq!(restored.len(), 2);
        assert!(r2.lookup("f1").is_some());
        assert!(r2.lookup("f2").is_some());
    }

    #[test]
    fn tampered_dump_is_rejected() {
        let r = reg();
        r.load("#!rhai name=a engine=rhai\nfn f(keys, args) { 1 }\n", false)
            .unwrap();
        let mut blob = r.dump().unwrap().to_vec();
        // flip one byte inside the code segment
        if let Some(pos) = blob.iter().position(|&b| b == b'f') {
            blob[pos] = b'g';
        }
        let r2 = reg();
        assert!(matches!(
            r2.restore(&blob, false),
            Err(FunctionError::SignatureMismatch)
        ));
    }
}
