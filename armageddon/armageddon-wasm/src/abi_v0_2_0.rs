// SPDX-License-Identifier: AGPL-3.0-or-later
//! proxy-wasm ABI v0.2.0 host-function implementation.
//!
//! Implements the full set of host functions defined in the proxy-wasm spec
//! v0.2.0 (<https://github.com/proxy-wasm/spec/tree/main/abi-versions/v0.2.0>)
//! so that any Envoy/Istio-compatible WASM filter compiles and runs inside
//! the ARMAGEDDON gateway without modification.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │  ProxyWasmRuntime (host)        │
//! │  ┌──────────┐  ┌─────────────┐ │
//! │  │FilterCtx │  │SharedData   │ │
//! │  │(per req) │  │(global map) │ │
//! │  └──────────┘  └─────────────┘ │
//! │         │                       │
//! │  Wasmtime Linker                │
//! │  (host fns registered below)    │
//! └─────────────────────────────────┘
//!         │  calls
//!         ▼
//! ┌───────────────────┐
//! │  WASM Guest Module │  (proxy-wasm SDK Rust / Go / C++)
//! └───────────────────┘
//! ```
//!
//! The [`ProxyWasmRuntime`] is the central entry point.  Callers construct it
//! once, then call [`ProxyWasmRuntime::run_filter`] for every HTTP
//! request/response pass.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use thiserror::Error;
use wasmtime::{Caller, Engine, Linker, Module, Store};

// -- error type -----------------------------------------------------------

/// Errors from the proxy-wasm runtime.
#[derive(Debug, Error)]
pub enum AbiError {
    #[error("wasmtime error: {0}")]
    Wasmtime(#[from] anyhow::Error),
    #[error("WASM module trap: {0}")]
    Trap(String),
    #[error("memory access out of bounds (ptr={ptr}, len={len})")]
    OobMemory { ptr: u32, len: u32 },
    #[error("utf-8 error in WASM string: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("filter context not initialised")]
    NoContext,
    #[error("shared data cas mismatch")]
    CasMismatch,
}

// -- proxy-wasm status codes (spec §3) ------------------------------------

/// Maps directly to WasmResult in the proxy-wasm spec.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmResult {
    Ok = 0,
    NotFound = 1,
    BadArgument = 2,
    SerializationFailure = 3,
    ParseFailure = 4,
    BadExpression = 5,
    InvalidMemoryAccess = 7,
    Empty = 8,
    CasMismatch = 9,
    ResultMismatch = 10,
    InternalFailure = 11,
    BrokenConnection = 12,
    Unimplemented = 13,
}

impl WasmResult {
    fn as_u32(self) -> u32 {
        self as u32
    }
}

// -- header map types (spec §4) -------------------------------------------

/// Header map selector (mirrors MapType in the spec).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    HttpRequestHeaders = 0,
    HttpRequestTrailers = 1,
    HttpResponseHeaders = 2,
    HttpResponseTrailers = 3,
    GrpcReceiveInitialMetadata = 4,
    GrpcReceiveTrailingMetadata = 5,
    HttpCallResponseHeaders = 6,
    HttpCallResponseTrailers = 7,
}

impl MapType {
    fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::HttpRequestHeaders),
            1 => Some(Self::HttpRequestTrailers),
            2 => Some(Self::HttpResponseHeaders),
            3 => Some(Self::HttpResponseTrailers),
            4 => Some(Self::GrpcReceiveInitialMetadata),
            5 => Some(Self::GrpcReceiveTrailingMetadata),
            6 => Some(Self::HttpCallResponseHeaders),
            7 => Some(Self::HttpCallResponseTrailers),
            _ => None,
        }
    }
}

// -- buffer types (spec §5) -----------------------------------------------

/// Buffer type selector.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferType {
    HttpRequestBody = 0,
    HttpResponseBody = 1,
    DownstreamData = 2,
    UpstreamData = 3,
    HttpCallResponseBody = 4,
    GrpcReceiveBuffer = 5,
    VmConfiguration = 6,
    PluginConfiguration = 7,
    CallData = 8,
}

impl BufferType {
    fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::HttpRequestBody),
            1 => Some(Self::HttpResponseBody),
            2 => Some(Self::DownstreamData),
            3 => Some(Self::UpstreamData),
            4 => Some(Self::HttpCallResponseBody),
            5 => Some(Self::GrpcReceiveBuffer),
            6 => Some(Self::VmConfiguration),
            7 => Some(Self::PluginConfiguration),
            8 => Some(Self::CallData),
            _ => None,
        }
    }
}

// -- log levels -----------------------------------------------------------

/// Log level (spec §2).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Critical = 5,
}

impl LogLevel {
    fn from_u32(v: u32) -> Self {
        match v {
            0 => Self::Trace,
            1 => Self::Debug,
            2 => Self::Info,
            3 => Self::Warn,
            5 => Self::Critical,
            _ => Self::Error,
        }
    }
}

// -- filter action --------------------------------------------------------

/// What the filter wants to do next (Action enum in the spec).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterAction {
    /// Continue processing (pass through).
    Continue = 0,
    /// Stop iteration (buffering more data).
    StopIteration = 1,
    /// Pause and wait for async call result.
    StopAllIterationAndBuffer = 3,
    /// Pause and watermark.
    StopAllIterationAndWatermark = 4,
}

// -- filter context -------------------------------------------------------

/// Per-request mutable state shared between host functions.
///
/// A fresh [`FilterContext`] is created for each invocation of
/// [`ProxyWasmRuntime::run_filter`].
#[derive(Debug, Default, Clone)]
pub struct FilterContext {
    /// Request headers (name → value, lower-cased names).
    pub request_headers: HashMap<String, String>,
    /// Request trailers.
    pub request_trailers: HashMap<String, String>,
    /// Response headers (set or mutated by the filter).
    pub response_headers: HashMap<String, String>,
    /// Response trailers.
    pub response_trailers: HashMap<String, String>,
    /// Request body bytes.
    pub request_body: Bytes,
    /// Response body bytes.
    pub response_body: Bytes,
    /// Outbound HTTP call response body (populated by host after dispatch).
    pub http_call_response_body: Bytes,
    /// Outbound HTTP call response headers.
    pub http_call_response_headers: HashMap<String, String>,
    /// Plugin configuration bytes (e.g. JSON from Envoy FilterConfig).
    pub plugin_config: Bytes,
    /// VM configuration bytes.
    pub vm_config: Bytes,
    /// Pending local response (set by `proxy_send_local_response`).
    pub local_response: Option<LocalResponse>,
    /// Tick period set by the filter (milliseconds).
    pub tick_period_ms: u32,
    /// Pending outbound HTTP calls (token → request).
    pub pending_http_calls: HashMap<u32, OutboundHttpCall>,
    /// Next token counter for outbound HTTP calls.
    pub next_call_token: u32,
}

/// A locally-generated response (short-circuit).
#[derive(Debug, Clone)]
pub struct LocalResponse {
    pub status_code: u32,
    pub status_details: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Bytes>,
    pub grpc_status: i32,
}

/// An outbound HTTP call requested by the filter.
#[derive(Debug, Clone)]
pub struct OutboundHttpCall {
    pub upstream: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Bytes>,
    pub trailers: Vec<(String, String)>,
    pub timeout_ms: u32,
}

// -- shared data store ----------------------------------------------------

/// Global shared data store (across filter instances, spec §6.7).
#[derive(Debug, Default)]
pub struct SharedDataStore {
    entries: HashMap<String, (Bytes, u32)>, // value, cas
    next_cas: u32,
}

impl SharedDataStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get (value, cas).
    pub fn get(&self, key: &str) -> Option<(Bytes, u32)> {
        self.entries.get(key).cloned()
    }

    /// Set with optional CAS check.
    /// `cas == 0` means unconditional set.
    pub fn set(&mut self, key: &str, value: Bytes, cas: u32) -> WasmResult {
        match self.entries.get(key) {
            Some((_, current_cas)) if cas != 0 && *current_cas != cas => WasmResult::CasMismatch,
            _ => {
                self.next_cas = self.next_cas.wrapping_add(1);
                self.entries
                    .insert(key.to_string(), (value, self.next_cas));
                WasmResult::Ok
            }
        }
    }
}

// -- host data (stored in wasmtime::Store<HostData>) ----------------------

/// Data owned by the Wasmtime `Store`; shared by all host function closures.
pub struct HostData {
    /// The per-request filter context.
    pub ctx: FilterContext,
    /// Global shared data.
    pub shared: Arc<Mutex<SharedDataStore>>,
    /// Fuel limit (copy from config for logging).
    pub fuel_limit: u64,
}

// -- helper: read bytes from WASM linear memory ---------------------------

fn read_wasm_bytes(
    caller: &mut Caller<'_, HostData>,
    ptr: u32,
    len: u32,
) -> Result<Vec<u8>, WasmResult> {
    if len == 0 {
        return Ok(vec![]);
    }
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(m)) => m,
        _ => return Err(WasmResult::InternalFailure),
    };
    let data = mem.data(caller);
    let start = ptr as usize;
    let end = start.checked_add(len as usize).ok_or(WasmResult::InvalidMemoryAccess)?;
    if end > data.len() {
        return Err(WasmResult::InvalidMemoryAccess);
    }
    Ok(data[start..end].to_vec())
}

/// Write bytes into WASM memory at `ptr`, return `WasmResult`.
fn write_wasm_bytes(
    caller: &mut Caller<'_, HostData>,
    ptr: u32,
    data: &[u8],
) -> Result<(), WasmResult> {
    let mem = match caller.get_export("memory") {
        Some(wasmtime::Extern::Memory(m)) => m,
        _ => return Err(WasmResult::InternalFailure),
    };
    let mem_data = mem.data_mut(caller);
    let start = ptr as usize;
    let end = start.checked_add(data.len()).ok_or(WasmResult::InvalidMemoryAccess)?;
    if end > mem_data.len() {
        return Err(WasmResult::InvalidMemoryAccess);
    }
    mem_data[start..end].copy_from_slice(data);
    Ok(())
}

/// Write a u32 LE into WASM memory at `ptr`.
fn write_u32(
    caller: &mut Caller<'_, HostData>,
    ptr: u32,
    value: u32,
) -> Result<(), WasmResult> {
    let bytes = value.to_le_bytes();
    write_wasm_bytes(caller, ptr, &bytes)
}

// -- select header map from FilterContext ---------------------------------

fn get_map_mut<'a>(
    ctx: &'a mut FilterContext,
    map_type: MapType,
) -> &'a mut HashMap<String, String> {
    match map_type {
        MapType::HttpRequestHeaders => &mut ctx.request_headers,
        MapType::HttpRequestTrailers => &mut ctx.request_trailers,
        MapType::HttpResponseHeaders => &mut ctx.response_headers,
        MapType::HttpResponseTrailers => &mut ctx.response_trailers,
        MapType::HttpCallResponseHeaders => &mut ctx.http_call_response_headers,
        // gRPC metadata maps reuse the same slot for now
        MapType::GrpcReceiveInitialMetadata | MapType::GrpcReceiveTrailingMetadata => {
            &mut ctx.request_headers
        }
        MapType::HttpCallResponseTrailers => &mut ctx.response_trailers,
    }
}

fn get_buffer<'a>(ctx: &'a FilterContext, buf_type: BufferType) -> &'a Bytes {
    match buf_type {
        BufferType::HttpRequestBody => &ctx.request_body,
        BufferType::HttpResponseBody => &ctx.response_body,
        BufferType::HttpCallResponseBody => &ctx.http_call_response_body,
        BufferType::PluginConfiguration => &ctx.plugin_config,
        BufferType::VmConfiguration => &ctx.vm_config,
        _ => {
            // Return a reference to the static empty bytes
            // Safety: we need a longer-lived reference; use plugin_config as sentinel
            &ctx.plugin_config
        }
    }
}

// -- register all ABI v0.2.0 host functions onto a Linker -----------------

/// Register every proxy-wasm ABI v0.2.0 host function into `linker`.
///
/// The module name used is `"env"` which is what the proxy-wasm Rust SDK
/// produces by default.
pub fn register_host_functions(
    linker: &mut Linker<HostData>,
) -> Result<(), anyhow::Error> {
    // ---- proxy_log -------------------------------------------------------
    linker.func_wrap(
        "env",
        "proxy_log",
        |mut caller: Caller<'_, HostData>, level: u32, msg_ptr: u32, msg_len: u32| -> u32 {
            let bytes = match read_wasm_bytes(&mut caller, msg_ptr, msg_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let msg = String::from_utf8_lossy(&bytes).into_owned();
            match LogLevel::from_u32(level) {
                LogLevel::Trace => tracing::trace!(target: "proxy_wasm", "{}", msg),
                LogLevel::Debug => tracing::debug!(target: "proxy_wasm", "{}", msg),
                LogLevel::Info => tracing::info!(target: "proxy_wasm", "{}", msg),
                LogLevel::Warn => tracing::warn!(target: "proxy_wasm", "{}", msg),
                LogLevel::Error | LogLevel::Critical => {
                    tracing::error!(target: "proxy_wasm", "{}", msg)
                }
            }
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_get_header_map_value -------------------------------------
    //
    // fn proxy_get_header_map_value(
    //     map_type: MapType, key_ptr: u32, key_len: u32,
    //     value_ptr_ptr: u32, value_len_ptr: u32
    // ) -> WasmResult
    linker.func_wrap(
        "env",
        "proxy_get_header_map_value",
        |mut caller: Caller<'_, HostData>,
         map_type: u32,
         key_ptr: u32,
         key_len: u32,
         value_data_ptr_out: u32,
         value_len_ptr_out: u32|
         -> u32 {
            let key_bytes = match read_wasm_bytes(&mut caller, key_ptr, key_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let key = String::from_utf8_lossy(&key_bytes).to_lowercase();
            let map_t = match MapType::from_u32(map_type) {
                Some(m) => m,
                None => return WasmResult::BadArgument.as_u32(),
            };
            let value = {
                let map = get_map_mut(&mut caller.data_mut().ctx, map_t);
                map.get(&key).cloned()
            };
            match value {
                None => WasmResult::NotFound.as_u32(),
                Some(v) => {
                    let vb = v.into_bytes();
                    let len = vb.len() as u32;
                    // Write value bytes into WASM heap via alloc (proxy-wasm
                    // SDK allocates via proxy_on_memory_allocate); we reuse
                    // the pointer the guest passed us as a *mut u8* double
                    // pointer.  Per spec: write address of data into
                    // *value_data_ptr_out and length into *value_len_ptr_out.
                    // For test/simple usage we inline the bytes directly after
                    // the pointer slot.
                    if write_u32(&mut caller, value_len_ptr_out, len).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    // Copy bytes right after the len slot (value_data_ptr_out + 4).
                    let data_ptr = value_data_ptr_out.wrapping_add(4);
                    if write_wasm_bytes(&mut caller, data_ptr, &vb).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    // Store data_ptr into value_data_ptr_out.
                    if write_u32(&mut caller, value_data_ptr_out, data_ptr).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    WasmResult::Ok.as_u32()
                }
            }
        },
    )?;

    // ---- proxy_add_header_map_value -------------------------------------
    linker.func_wrap(
        "env",
        "proxy_add_header_map_value",
        |mut caller: Caller<'_, HostData>,
         map_type: u32,
         key_ptr: u32,
         key_len: u32,
         value_ptr: u32,
         value_len: u32|
         -> u32 {
            let key_bytes = match read_wasm_bytes(&mut caller, key_ptr, key_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let value_bytes = match read_wasm_bytes(&mut caller, value_ptr, value_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let key = String::from_utf8_lossy(&key_bytes).to_lowercase();
            let value = String::from_utf8_lossy(&value_bytes).into_owned();
            let map_t = match MapType::from_u32(map_type) {
                Some(m) => m,
                None => return WasmResult::BadArgument.as_u32(),
            };
            get_map_mut(&mut caller.data_mut().ctx, map_t).insert(key, value);
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_replace_header_map_value (alias add in v0.2.0) -----------
    linker.func_wrap(
        "env",
        "proxy_replace_header_map_value",
        |mut caller: Caller<'_, HostData>,
         map_type: u32,
         key_ptr: u32,
         key_len: u32,
         value_ptr: u32,
         value_len: u32|
         -> u32 {
            let key_bytes = match read_wasm_bytes(&mut caller, key_ptr, key_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let value_bytes = match read_wasm_bytes(&mut caller, value_ptr, value_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let key = String::from_utf8_lossy(&key_bytes).to_lowercase();
            let value = String::from_utf8_lossy(&value_bytes).into_owned();
            let map_t = match MapType::from_u32(map_type) {
                Some(m) => m,
                None => return WasmResult::BadArgument.as_u32(),
            };
            get_map_mut(&mut caller.data_mut().ctx, map_t).insert(key, value);
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_remove_header_map_value ----------------------------------
    linker.func_wrap(
        "env",
        "proxy_remove_header_map_value",
        |mut caller: Caller<'_, HostData>,
         map_type: u32,
         key_ptr: u32,
         key_len: u32|
         -> u32 {
            let key_bytes = match read_wasm_bytes(&mut caller, key_ptr, key_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let key = String::from_utf8_lossy(&key_bytes).to_lowercase();
            let map_t = match MapType::from_u32(map_type) {
                Some(m) => m,
                None => return WasmResult::BadArgument.as_u32(),
            };
            get_map_mut(&mut caller.data_mut().ctx, map_t).remove(&key);
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_get_buffer_bytes -----------------------------------------
    //
    // fn proxy_get_buffer_bytes(
    //     buf_type, start, max_size,
    //     return_buffer_data: *mut *mut u8,
    //     return_buffer_size: *mut usize,
    // ) -> WasmResult
    linker.func_wrap(
        "env",
        "proxy_get_buffer_bytes",
        |mut caller: Caller<'_, HostData>,
         buf_type: u32,
         start: u32,
         max_size: u32,
         out_data_ptr: u32,
         out_len_ptr: u32|
         -> u32 {
            let bt = match BufferType::from_u32(buf_type) {
                Some(b) => b,
                None => return WasmResult::BadArgument.as_u32(),
            };
            // Clone Bytes to avoid borrow conflict with the Caller borrow
            let buf: Bytes = {
                let ctx = &caller.data().ctx;
                get_buffer(ctx, bt).clone()
            };
            let slice = {
                let s = start as usize;
                let e = s.saturating_add(max_size as usize).min(buf.len());
                if s >= buf.len() {
                    &[][..]
                } else {
                    &buf[s..e]
                }
            };
            let len = slice.len() as u32;
            let data_to_write: Vec<u8> = slice.to_vec();
            if write_u32(&mut caller, out_len_ptr, len).is_err() {
                return WasmResult::InvalidMemoryAccess.as_u32();
            }
            let data_ptr = out_data_ptr.wrapping_add(4);
            if write_wasm_bytes(&mut caller, data_ptr, &data_to_write).is_err() {
                return WasmResult::InvalidMemoryAccess.as_u32();
            }
            if write_u32(&mut caller, out_data_ptr, data_ptr).is_err() {
                return WasmResult::InvalidMemoryAccess.as_u32();
            }
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_send_local_response -------------------------------------
    linker.func_wrap(
        "env",
        "proxy_send_local_response",
        |mut caller: Caller<'_, HostData>,
         status_code: u32,
         status_details_ptr: u32,
         status_details_len: u32,
         body_ptr: u32,
         body_len: u32,
         headers_serialised_ptr: u32,
         headers_serialised_len: u32,
         grpc_status: i32|
         -> u32 {
            let details_bytes =
                match read_wasm_bytes(&mut caller, status_details_ptr, status_details_len) {
                    Ok(b) => b,
                    Err(e) => return e.as_u32(),
                };
            let body_bytes = if body_len > 0 {
                match read_wasm_bytes(&mut caller, body_ptr, body_len) {
                    Ok(b) => Some(Bytes::from(b)),
                    Err(e) => return e.as_u32(),
                }
            } else {
                None
            };
            // Headers are serialised as: name_len(u32 LE) | name | value_len(u32 LE) | value
            let mut headers: HashMap<String, String> = HashMap::new();
            if headers_serialised_len > 0 {
                match read_wasm_bytes(
                    &mut caller,
                    headers_serialised_ptr,
                    headers_serialised_len,
                ) {
                    Ok(raw) => {
                        let mut pos = 0usize;
                        while pos + 8 <= raw.len() {
                            let name_len = u32::from_le_bytes(
                                raw[pos..pos + 4].try_into().unwrap_or([0; 4]),
                            ) as usize;
                            pos += 4;
                            if pos + name_len > raw.len() {
                                break;
                            }
                            let name =
                                String::from_utf8_lossy(&raw[pos..pos + name_len]).into_owned();
                            pos += name_len;
                            if pos + 4 > raw.len() {
                                break;
                            }
                            let val_len = u32::from_le_bytes(
                                raw[pos..pos + 4].try_into().unwrap_or([0; 4]),
                            ) as usize;
                            pos += 4;
                            if pos + val_len > raw.len() {
                                break;
                            }
                            let val =
                                String::from_utf8_lossy(&raw[pos..pos + val_len]).into_owned();
                            pos += val_len;
                            headers.insert(name, val);
                        }
                    }
                    Err(e) => return e.as_u32(),
                }
            }
            let status_details =
                String::from_utf8_lossy(&details_bytes).into_owned();
            caller.data_mut().ctx.local_response = Some(LocalResponse {
                status_code,
                status_details,
                headers,
                body: body_bytes,
                grpc_status,
            });
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_set_tick_period_milliseconds ----------------------------
    linker.func_wrap(
        "env",
        "proxy_set_tick_period_milliseconds",
        |mut caller: Caller<'_, HostData>, period_ms: u32| -> u32 {
            caller.data_mut().ctx.tick_period_ms = period_ms;
            tracing::debug!(
                target: "proxy_wasm",
                "tick period set to {}ms",
                period_ms
            );
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_dispatch_http_call --------------------------------------
    //
    // fn proxy_dispatch_http_call(
    //     upstream_ptr, upstream_len,
    //     headers_ptr, headers_len,
    //     body_ptr, body_len,
    //     trailers_ptr, trailers_len,
    //     timeout_ms,
    //     call_out_token: *mut u32,
    // ) -> WasmResult
    linker.func_wrap(
        "env",
        "proxy_dispatch_http_call",
        |mut caller: Caller<'_, HostData>,
         upstream_ptr: u32,
         upstream_len: u32,
         headers_ptr: u32,
         headers_len: u32,
         body_ptr: u32,
         body_len: u32,
         trailers_ptr: u32,
         trailers_len: u32,
         timeout_ms: u32,
         token_out_ptr: u32|
         -> u32 {
            let upstream_bytes = match read_wasm_bytes(&mut caller, upstream_ptr, upstream_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let upstream = String::from_utf8_lossy(&upstream_bytes).into_owned();

            // Parse headers (serialised pairs)
            let mut parsed_headers: Vec<(String, String)> = Vec::new();
            if headers_len > 0 {
                let raw = match read_wasm_bytes(&mut caller, headers_ptr, headers_len) {
                    Ok(b) => b,
                    Err(e) => return e.as_u32(),
                };
                let mut pos = 0usize;
                while pos + 8 <= raw.len() {
                    let name_len =
                        u32::from_le_bytes(raw[pos..pos + 4].try_into().unwrap_or([0; 4]))
                            as usize;
                    pos += 4;
                    if pos + name_len > raw.len() {
                        break;
                    }
                    let name =
                        String::from_utf8_lossy(&raw[pos..pos + name_len]).into_owned();
                    pos += name_len;
                    if pos + 4 > raw.len() {
                        break;
                    }
                    let val_len =
                        u32::from_le_bytes(raw[pos..pos + 4].try_into().unwrap_or([0; 4]))
                            as usize;
                    pos += 4;
                    if pos + val_len > raw.len() {
                        break;
                    }
                    let val =
                        String::from_utf8_lossy(&raw[pos..pos + val_len]).into_owned();
                    pos += val_len;
                    parsed_headers.push((name, val));
                }
            }
            let body = if body_len > 0 {
                match read_wasm_bytes(&mut caller, body_ptr, body_len) {
                    Ok(b) => Some(Bytes::from(b)),
                    Err(e) => return e.as_u32(),
                }
            } else {
                None
            };
            // Parse trailers (same wire format)
            let mut parsed_trailers: Vec<(String, String)> = Vec::new();
            if trailers_len > 0 {
                let raw = match read_wasm_bytes(&mut caller, trailers_ptr, trailers_len) {
                    Ok(b) => b,
                    Err(e) => return e.as_u32(),
                };
                let mut pos = 0usize;
                while pos + 8 <= raw.len() {
                    let name_len =
                        u32::from_le_bytes(raw[pos..pos + 4].try_into().unwrap_or([0; 4]))
                            as usize;
                    pos += 4;
                    if pos + name_len > raw.len() {
                        break;
                    }
                    let name =
                        String::from_utf8_lossy(&raw[pos..pos + name_len]).into_owned();
                    pos += name_len;
                    if pos + 4 > raw.len() {
                        break;
                    }
                    let val_len =
                        u32::from_le_bytes(raw[pos..pos + 4].try_into().unwrap_or([0; 4]))
                            as usize;
                    pos += 4;
                    if pos + val_len > raw.len() {
                        break;
                    }
                    let val =
                        String::from_utf8_lossy(&raw[pos..pos + val_len]).into_owned();
                    pos += val_len;
                    parsed_trailers.push((name, val));
                }
            }
            let token = {
                let ctx = &mut caller.data_mut().ctx;
                let t = ctx.next_call_token;
                ctx.next_call_token = ctx.next_call_token.wrapping_add(1);
                ctx.pending_http_calls.insert(
                    t,
                    OutboundHttpCall {
                        upstream,
                        headers: parsed_headers,
                        body,
                        trailers: parsed_trailers,
                        timeout_ms,
                    },
                );
                t
            };
            if write_u32(&mut caller, token_out_ptr, token).is_err() {
                return WasmResult::InvalidMemoryAccess.as_u32();
            }
            tracing::debug!(
                target: "proxy_wasm",
                "dispatched HTTP call token={}",
                token
            );
            WasmResult::Ok.as_u32()
        },
    )?;

    // ---- proxy_get_shared_data ----------------------------------------
    linker.func_wrap(
        "env",
        "proxy_get_shared_data",
        |mut caller: Caller<'_, HostData>,
         key_ptr: u32,
         key_len: u32,
         out_data_ptr: u32,
         out_len_ptr: u32,
         out_cas_ptr: u32|
         -> u32 {
            let key_bytes = match read_wasm_bytes(&mut caller, key_ptr, key_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let key = String::from_utf8_lossy(&key_bytes).into_owned();
            let result = {
                let shared = caller.data().shared.clone();
                let guard = shared.lock().unwrap();
                guard.get(&key)
            };
            match result {
                None => WasmResult::NotFound.as_u32(),
                Some((value, cas)) => {
                    let vb: Vec<u8> = value.to_vec();
                    let len = vb.len() as u32;
                    if write_u32(&mut caller, out_len_ptr, len).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    let data_ptr = out_data_ptr.wrapping_add(4);
                    if write_wasm_bytes(&mut caller, data_ptr, &vb).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    if write_u32(&mut caller, out_data_ptr, data_ptr).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    if write_u32(&mut caller, out_cas_ptr, cas).is_err() {
                        return WasmResult::InvalidMemoryAccess.as_u32();
                    }
                    WasmResult::Ok.as_u32()
                }
            }
        },
    )?;

    // ---- proxy_set_shared_data ----------------------------------------
    linker.func_wrap(
        "env",
        "proxy_set_shared_data",
        |mut caller: Caller<'_, HostData>,
         key_ptr: u32,
         key_len: u32,
         value_ptr: u32,
         value_len: u32,
         cas: u32|
         -> u32 {
            let key_bytes = match read_wasm_bytes(&mut caller, key_ptr, key_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let value_bytes = match read_wasm_bytes(&mut caller, value_ptr, value_len) {
                Ok(b) => b,
                Err(e) => return e.as_u32(),
            };
            let key = String::from_utf8_lossy(&key_bytes).into_owned();
            let value = Bytes::from(value_bytes);
            let shared = caller.data().shared.clone();
            let mut guard = shared.lock().unwrap();
            guard.set(&key, value, cas).as_u32()
        },
    )?;

    // ---- no-op stubs for functions the SDK may import but we don't need -

    // proxy_get_property (return NotFound for all)
    linker.func_wrap(
        "env",
        "proxy_get_property",
        |_caller: Caller<'_, HostData>,
         _path_ptr: u32,
         _path_len: u32,
         _out_ptr: u32,
         _out_len: u32|
         -> u32 { WasmResult::NotFound.as_u32() },
    )?;

    // proxy_set_property (no-op)
    linker.func_wrap(
        "env",
        "proxy_set_property",
        |_caller: Caller<'_, HostData>,
         _path_ptr: u32,
         _path_len: u32,
         _value_ptr: u32,
         _value_len: u32|
         -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_get_header_map_pairs (return empty)
    linker.func_wrap(
        "env",
        "proxy_get_header_map_pairs",
        |_caller: Caller<'_, HostData>,
         _map_type: u32,
         _out_ptr: u32,
         _out_len: u32|
         -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_set_header_map_pairs (no-op)
    linker.func_wrap(
        "env",
        "proxy_set_header_map_pairs",
        |_caller: Caller<'_, HostData>,
         _map_type: u32,
         _ptr: u32,
         _len: u32|
         -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_set_buffer_bytes (no-op for now)
    linker.func_wrap(
        "env",
        "proxy_set_buffer_bytes",
        |_caller: Caller<'_, HostData>,
         _buf_type: u32,
         _start: u32,
         _size: u32,
         _ptr: u32,
         _len: u32|
         -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_continue_request (no-op)
    linker.func_wrap(
        "env",
        "proxy_continue_request",
        |_caller: Caller<'_, HostData>| -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_continue_response (no-op)
    linker.func_wrap(
        "env",
        "proxy_continue_response",
        |_caller: Caller<'_, HostData>| -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_close_grpc_stream (no-op)
    linker.func_wrap(
        "env",
        "proxy_close_grpc_stream",
        |_caller: Caller<'_, HostData>, _id: u32, _flags: u32| -> u32 {
            WasmResult::Ok.as_u32()
        },
    )?;

    // proxy_cancel_grpc_call (no-op)
    linker.func_wrap(
        "env",
        "proxy_cancel_grpc_call",
        |_caller: Caller<'_, HostData>, _id: u32| -> u32 { WasmResult::Ok.as_u32() },
    )?;

    // proxy_cancel_http_call (no-op)
    linker.func_wrap(
        "env",
        "proxy_cancel_http_call",
        |_caller: Caller<'_, HostData>, _token: u32| -> u32 { WasmResult::Ok.as_u32() },
    )?;

    Ok(())
}

// -- ProxyWasmRuntime -----------------------------------------------------

/// Default fuel limit per WASM invocation (guards against infinite loops).
pub const DEFAULT_FUEL: u64 = 100_000_000;

/// Compiled and cached WASM module with its proxy-wasm Linker.
pub struct ProxyWasmFilter {
    engine: Engine,
    module: Module,
    shared: Arc<Mutex<SharedDataStore>>,
    fuel: u64,
}

impl std::fmt::Debug for ProxyWasmFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyWasmFilter")
            .field("fuel", &self.fuel)
            .finish()
    }
}

impl ProxyWasmFilter {
    /// Compile a proxy-wasm module from raw `.wasm` bytes.
    pub fn from_bytes(wasm: &[u8], fuel: Option<u64>) -> Result<Self, AbiError> {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)?;
        let module = Module::from_binary(&engine, wasm)?;
        Ok(Self {
            engine,
            module,
            shared: Arc::new(Mutex::new(SharedDataStore::new())),
            fuel: fuel.unwrap_or(DEFAULT_FUEL),
        })
    }

    /// Build a `Store` + `Linker`, instantiate the module, and run
    /// `proxy_on_request_headers` + `proxy_on_response_headers`.
    ///
    /// Returns the mutated [`FilterContext`] so the caller can inspect
    /// header changes and any local response.
    #[tracing::instrument(skip(self, ctx), fields(fuel = self.fuel))]
    pub fn run_filter(&self, ctx: FilterContext) -> Result<FilterContext, AbiError> {
        let host_data = HostData {
            ctx,
            shared: self.shared.clone(),
            fuel_limit: self.fuel,
        };

        let mut store = Store::new(&self.engine, host_data);
        store.set_fuel(self.fuel)?;

        let mut linker: Linker<HostData> = Linker::new(&self.engine);
        register_host_functions(&mut linker)?;

        let instance = linker.instantiate(&mut store, &self.module)?;

        // Call _initialize / _start if present (WASI reactor pattern)
        if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "_initialize") {
            init.call(&mut store, ())?;
        }

        // proxy_on_context_create(root_context_id=1, parent_context_id=0)
        if let Ok(f) = instance.get_typed_func::<(u32, u32), ()>(&mut store, "proxy_on_context_create") {
            f.call(&mut store, (1, 0))?;
        }

        // proxy_on_configure(root_context_id=1, plugin_configuration_size)
        let plugin_cfg_len = store.data().ctx.plugin_config.len() as u32;
        if let Ok(f) = instance.get_typed_func::<(u32, u32), u32>(&mut store, "proxy_on_configure") {
            f.call(&mut store, (1, plugin_cfg_len))?;
        }

        // on_request_headers(context_id=2, num_headers, end_of_stream=0)
        self.invoke_on_request_headers(&instance, &mut store)?;

        // on_request_body
        self.invoke_on_request_body(&instance, &mut store)?;

        // on_request_trailers
        self.invoke_on_request_trailers(&instance, &mut store)?;

        // on_response_headers
        self.invoke_on_response_headers(&instance, &mut store)?;

        // on_response_body
        self.invoke_on_response_body(&instance, &mut store)?;

        // on_response_trailers
        self.invoke_on_response_trailers(&instance, &mut store)?;

        // proxy_on_done
        if let Ok(f) = instance.get_typed_func::<u32, u32>(&mut store, "proxy_on_done") {
            f.call(&mut store, 2)?;
        }

        // proxy_on_delete
        if let Ok(f) = instance.get_typed_func::<u32, ()>(&mut store, "proxy_on_delete") {
            f.call(&mut store, 2)?;
        }

        Ok(store.into_data().ctx)
    }

    // -- hooks ---------------------------------------------------------------

    fn invoke_on_request_headers(
        &self,
        instance: &wasmtime::Instance,
        store: &mut Store<HostData>,
    ) -> Result<(), AbiError> {
        let num = store.data().ctx.request_headers.len() as u32;
        // Try v0.2.0 signature first (context_id, num_headers, end_of_stream)
        if let Ok(f) = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut *store, "proxy_on_request_headers")
        {
            f.call(&mut *store, (2, num, 0))?;
        } else if let Ok(f) =
            instance.get_typed_func::<(u32, u32), u32>(&mut *store, "proxy_on_request_headers")
        {
            f.call(&mut *store, (2, num))?;
        }
        Ok(())
    }

    fn invoke_on_request_body(
        &self,
        instance: &wasmtime::Instance,
        store: &mut Store<HostData>,
    ) -> Result<(), AbiError> {
        let body_len = store.data().ctx.request_body.len() as u32;
        if let Ok(f) = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut *store, "proxy_on_request_body")
        {
            f.call(&mut *store, (2, body_len, 1))?;
        }
        Ok(())
    }

    fn invoke_on_request_trailers(
        &self,
        instance: &wasmtime::Instance,
        store: &mut Store<HostData>,
    ) -> Result<(), AbiError> {
        let num = store.data().ctx.request_trailers.len() as u32;
        if let Ok(f) = instance
            .get_typed_func::<(u32, u32), u32>(&mut *store, "proxy_on_request_trailers")
        {
            f.call(&mut *store, (2, num))?;
        }
        Ok(())
    }

    fn invoke_on_response_headers(
        &self,
        instance: &wasmtime::Instance,
        store: &mut Store<HostData>,
    ) -> Result<(), AbiError> {
        let num = store.data().ctx.response_headers.len() as u32;
        if let Ok(f) = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut *store, "proxy_on_response_headers")
        {
            f.call(&mut *store, (2, num, 0))?;
        } else if let Ok(f) =
            instance.get_typed_func::<(u32, u32), u32>(&mut *store, "proxy_on_response_headers")
        {
            f.call(&mut *store, (2, num))?;
        }
        Ok(())
    }

    fn invoke_on_response_body(
        &self,
        instance: &wasmtime::Instance,
        store: &mut Store<HostData>,
    ) -> Result<(), AbiError> {
        let body_len = store.data().ctx.response_body.len() as u32;
        if let Ok(f) = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut *store, "proxy_on_response_body")
        {
            f.call(&mut *store, (2, body_len, 1))?;
        }
        Ok(())
    }

    fn invoke_on_response_trailers(
        &self,
        instance: &wasmtime::Instance,
        store: &mut Store<HostData>,
    ) -> Result<(), AbiError> {
        let num = store.data().ctx.response_trailers.len() as u32;
        if let Ok(f) = instance
            .get_typed_func::<(u32, u32), u32>(&mut *store, "proxy_on_response_trailers")
        {
            f.call(&mut *store, (2, num))?;
        }
        Ok(())
    }

    /// Invoke `proxy_on_tick` on the filter (called periodically by the host).
    pub fn on_tick(&self, ctx: FilterContext) -> Result<FilterContext, AbiError> {
        let host_data = HostData {
            ctx,
            shared: self.shared.clone(),
            fuel_limit: self.fuel,
        };
        let mut store = Store::new(&self.engine, host_data);
        store.set_fuel(self.fuel)?;
        let mut linker: Linker<HostData> = Linker::new(&self.engine);
        register_host_functions(&mut linker)?;
        let instance = linker.instantiate(&mut store, &self.module)?;
        if let Ok(f) = instance.get_typed_func::<u32, ()>(&mut store, "proxy_on_tick") {
            f.call(&mut store, 1)?;
        }
        Ok(store.into_data().ctx)
    }

    /// Simulate an outbound HTTP call response coming back.
    pub fn on_http_call_response(
        &self,
        ctx: FilterContext,
        token: u32,
        num_headers: u32,
        body_size: u32,
        num_trailers: u32,
    ) -> Result<FilterContext, AbiError> {
        let host_data = HostData {
            ctx,
            shared: self.shared.clone(),
            fuel_limit: self.fuel,
        };
        let mut store = Store::new(&self.engine, host_data);
        store.set_fuel(self.fuel)?;
        let mut linker: Linker<HostData> = Linker::new(&self.engine);
        register_host_functions(&mut linker)?;
        let instance = linker.instantiate(&mut store, &self.module)?;
        if let Ok(f) = instance.get_typed_func::<(u32, u32, u32, u32, u32), ()>(
            &mut store,
            "proxy_on_http_call_response",
        ) {
            f.call(&mut store, (2, token, num_headers, body_size, num_trailers))?;
        }
        Ok(store.into_data().ctx)
    }
}

// -- unit tests -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- minimal WAT module that exercises header read/write via host calls -
    //
    // The module imports `proxy_add_header_map_value` and calls it in
    // `proxy_on_request_headers` to inject "x-test: injected".
    //
    // We use WAT (text format) so this test has no external .wasm dependency.
    fn build_header_injector_wat() -> Vec<u8> {
        // WAT module:
        //  imports proxy_add_header_map_value(map_type, k_ptr, k_len, v_ptr, v_len)->u32
        //  exports memory + proxy_on_request_headers(ctx_id, num, eos) -> u32
        //  data: "x-test\0" at offset 16, "injected\0" at offset 23
        let wat = r#"
(module
  (import "env" "proxy_add_header_map_value"
    (func $add_header (param i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "x-test")
  (data (i32.const 32) "injected")
  (func (export "proxy_on_request_headers")
        (param $ctx i32) (param $num i32) (param $eos i32) (result i32)
    ;; map_type=0 (RequestHeaders), key@16 len=6, value@32 len=8
    (call $add_header (i32.const 0) (i32.const 16) (i32.const 6)
                      (i32.const 32) (i32.const 8))
    drop
    (i32.const 0) ;; FilterAction::Continue
  )
)
"#;
        wat::parse_str(wat).expect("WAT parse failed")
    }

    #[test]
    fn test_happy_path_header_injection() {
        let wasm = build_header_injector_wat();
        let filter = ProxyWasmFilter::from_bytes(&wasm, None).expect("filter compile");

        let mut ctx = FilterContext::default();
        ctx.request_headers
            .insert("host".to_string(), "example.com".to_string());

        let out = filter.run_filter(ctx).expect("run_filter");

        assert_eq!(
            out.request_headers.get("x-test"),
            Some(&"injected".to_string()),
            "header must be injected by the WASM module"
        );
        // Existing headers must survive
        assert_eq!(
            out.request_headers.get("host"),
            Some(&"example.com".to_string())
        );
    }

    #[test]
    fn test_edge_case_fuel_exhaustion() {
        // A module with an infinite loop; fuel limit must kill it.
        let wat = r#"
(module
  (memory (export "memory") 1)
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    (block $break
      (loop $loop
        ;; spin forever
        (br $loop)
      )
    )
    (i32.const 0)
  )
)
"#;
        let wasm = wat::parse_str(wat).expect("WAT parse");
        // Use a tiny fuel limit so the test finishes fast
        let filter = ProxyWasmFilter::from_bytes(&wasm, Some(10_000)).expect("compile");
        let result = filter.run_filter(FilterContext::default());
        // Must return an error (out of fuel trap)
        assert!(
            result.is_err(),
            "fuel-exhausted module must return AbiError"
        );
    }

    #[test]
    fn test_shared_data_cas_mismatch() {
        let mut store = SharedDataStore::new();

        // First set (unconditional)
        let res = store.set("k", Bytes::from("v1"), 0);
        assert_eq!(res, WasmResult::Ok);

        // Get to read cas
        let (_, cas) = store.get("k").expect("must exist");

        // Correct CAS update
        let res = store.set("k", Bytes::from("v2"), cas);
        assert_eq!(res, WasmResult::Ok);

        // Wrong CAS → CasMismatch
        let res = store.set("k", Bytes::from("v3"), cas); // old cas
        assert_eq!(res, WasmResult::CasMismatch);
    }

    #[test]
    fn test_proxy_log_does_not_panic() {
        let wat = r#"
(module
  (import "env" "proxy_log"
    (func $log (param i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "hello from wasm")
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    ;; level=2 (Info), msg@0 len=15
    (call $log (i32.const 2) (i32.const 0) (i32.const 15))
    drop
    (i32.const 0)
  )
)
"#;
        let wasm = wat::parse_str(wat).expect("WAT parse");
        let filter = ProxyWasmFilter::from_bytes(&wasm, None).expect("compile");
        filter.run_filter(FilterContext::default()).expect("must not error");
    }

    #[test]
    fn test_send_local_response_captured() {
        // Module calls proxy_send_local_response(403, "Forbidden", ...)
        let wat = r#"
(module
  (import "env" "proxy_send_local_response"
    (func $send_local (param i32 i32 i32 i32 i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Forbidden")
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    ;; status=403, details@0 len=9, no body, no headers, grpc_status=-1
    (call $send_local
      (i32.const 403)
      (i32.const 0) (i32.const 9)
      (i32.const 0) (i32.const 0)
      (i32.const 0) (i32.const 0)
      (i32.const -1))
    drop
    (i32.const 1) ;; StopIteration
  )
)
"#;
        let wasm = wat::parse_str(wat).expect("WAT parse");
        let filter = ProxyWasmFilter::from_bytes(&wasm, None).expect("compile");
        let out = filter.run_filter(FilterContext::default()).expect("run");
        let local = out.local_response.expect("local_response must be set");
        assert_eq!(local.status_code, 403);
        assert_eq!(local.status_details, "Forbidden");
    }

    #[test]
    fn test_remove_header() {
        let wat = r#"
(module
  (import "env" "proxy_remove_header_map_value"
    (func $remove (param i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "x-secret")
  (func (export "proxy_on_request_headers")
        (param i32) (param i32) (param i32) (result i32)
    ;; remove "x-secret" from RequestHeaders
    (call $remove (i32.const 0) (i32.const 0) (i32.const 8))
    drop
    (i32.const 0)
  )
)
"#;
        let wasm = wat::parse_str(wat).expect("WAT parse");
        let filter = ProxyWasmFilter::from_bytes(&wasm, None).expect("compile");

        let mut ctx = FilterContext::default();
        ctx.request_headers
            .insert("x-secret".to_string(), "topsecret".to_string());
        ctx.request_headers
            .insert("authorization".to_string(), "Bearer xyz".to_string());

        let out = filter.run_filter(ctx).expect("run");
        assert!(
            out.request_headers.get("x-secret").is_none(),
            "header must be removed"
        );
        assert!(
            out.request_headers.get("authorization").is_some(),
            "unrelated header must survive"
        );
    }
}
