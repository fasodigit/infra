// SPDX-License-Identifier: AGPL-3.0-or-later
//! header-mutator — example proxy-wasm filter for ARMAGEDDON.
//!
//! This filter demonstrates the proxy-wasm ABI v0.2.0 contract.
//! It adds `X-FASO-Filter: processed` to every proxied HTTP request and
//! logs a short info message.
//!
//! # Build
//!
//! ```sh
//! rustup target add wasm32-wasip1
//! cargo build --target wasm32-wasip1 --release
//! # output: target/wasm32-wasip1/release/header_mutator.wasm
//! ```
//!
//! # Load into ARMAGEDDON
//!
//! ```rust,ignore
//! use armageddon_wasm::ProxyWasmFilter;
//!
//! let wasm = std::fs::read("header_mutator.wasm")?;
//! let filter = ProxyWasmFilter::from_bytes(&wasm, None)?;
//! let out_ctx = filter.run_filter(ctx)?;
//! // out_ctx.request_headers["x-faso-filter"] == "processed"
//! ```

use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::{Action, ContextType, LogLevel};

// -- root context ---------------------------------------------------------

/// Root context: one per VM lifetime (handles configuration + tick).
struct FasoFilterRoot {
    context_id: u32,
}

impl RootContext for FasoFilterRoot {
    fn on_vm_start(&mut self, _vm_config_size: usize) -> bool {
        proxy_wasm::hostcalls::log(
            LogLevel::Info,
            "FASO header-mutator filter loaded",
        )
        .ok();
        true
    }

    fn on_configure(&mut self, _plugin_config_size: usize) -> bool {
        proxy_wasm::hostcalls::log(LogLevel::Debug, "FASO header-mutator configured").ok();
        true
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }

    fn create_http_context(&self, context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(FasoFilter { context_id }))
    }
}

impl Context for FasoFilterRoot {}

// -- per-request HTTP context ---------------------------------------------

/// Per-request context created by the root.
struct FasoFilter {
    context_id: u32,
}

impl HttpContext for FasoFilter {
    /// Called when all request headers have been received.
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        // Add the FASO watermark header to the upstream request.
        self.add_http_request_header("x-faso-filter", "processed");

        // Log the request path for observability.
        let path = self
            .get_http_request_header(":path")
            .unwrap_or_else(|| "/".to_string());
        proxy_wasm::hostcalls::log(
            LogLevel::Info,
            &format!(
                "FASO[ctx={}] on_request_headers path={}",
                self.context_id, path
            ),
        )
        .ok();

        Action::Continue
    }

    /// Called when all response headers have been received.
    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        // Stamp the response as well so clients can confirm the filter ran.
        self.add_http_response_header("x-faso-filter", "processed");
        Action::Continue
    }
}

impl Context for FasoFilter {}

// -- entry point ----------------------------------------------------------

/// Called by the WASM runtime at startup to register our factory.
#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_root_context(|context_id| -> Box<dyn RootContext> {
        Box::new(FasoFilterRoot { context_id })
    });
}
