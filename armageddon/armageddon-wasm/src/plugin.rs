//! Plugin interface: defines the contract between host and WASM guest.

use serde::{Deserialize, Serialize};

/// Plugin manifest (metadata about a WASM plugin).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub entrypoint: String,
    pub permissions: PluginPermissions,
}

/// What the plugin is allowed to access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPermissions {
    pub read_headers: bool,
    pub read_body: bool,
    pub read_uri: bool,
    pub network: bool,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self {
            read_headers: true,
            read_body: true,
            read_uri: true,
            network: false,
        }
    }
}
