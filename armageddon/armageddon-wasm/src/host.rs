//! Host functions exposed to WASM plugins.

/// Host function IDs exposed to WASM guest modules.
pub enum HostFunction {
    /// Log a message from the plugin.
    Log,
    /// Read a request header by name.
    GetHeader,
    /// Read the request URI.
    GetUri,
    /// Read the request method.
    GetMethod,
    /// Read the request body (if permitted).
    GetBody,
    /// Return a decision to the host.
    SetDecision,
}

impl HostFunction {
    /// Get the function name as exported to WASM.
    pub fn export_name(&self) -> &'static str {
        match self {
            HostFunction::Log => "armageddon_log",
            HostFunction::GetHeader => "armageddon_get_header",
            HostFunction::GetUri => "armageddon_get_uri",
            HostFunction::GetMethod => "armageddon_get_method",
            HostFunction::GetBody => "armageddon_get_body",
            HostFunction::SetDecision => "armageddon_set_decision",
        }
    }
}
