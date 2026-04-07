//! KAYA Security: TLS configuration, ACL, and authentication.

use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("TLS configuration error: {0}")]
    Tls(String),

    #[error("authentication failed for user: {0}")]
    AuthFailed(String),

    #[error("permission denied: {user} cannot execute {command}")]
    PermissionDenied { user: String, command: String },

    #[error("ACL parse error: {0}")]
    AclParse(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// TLS config
// ---------------------------------------------------------------------------

/// TLS configuration for KAYA server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert_path: String,
    pub key_path: String,
    pub ca_path: String,
    pub require_client_auth: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_path: String::new(),
            key_path: String::new(),
            ca_path: String::new(),
            require_client_auth: false,
        }
    }
}

impl TlsConfig {
    /// Build a `rustls::ServerConfig` from this configuration.
    /// Returns `None` if TLS is disabled.
    pub fn build_server_config(&self) -> Result<Option<Arc<rustls::ServerConfig>>, SecurityError> {
        if !self.enabled {
            return Ok(None);
        }

        let cert_path = Path::new(&self.cert_path);
        let key_path = Path::new(&self.key_path);

        let cert_file =
            &mut std::io::BufReader::new(std::fs::File::open(cert_path)?);
        let key_file =
            &mut std::io::BufReader::new(std::fs::File::open(key_path)?);

        let certs: Vec<_> = rustls_pemfile::certs(cert_file)
            .filter_map(|r| r.ok())
            .collect();
        let key = rustls_pemfile::private_key(key_file)
            .map_err(|e| SecurityError::Tls(e.to_string()))?
            .ok_or_else(|| SecurityError::Tls("no private key found".into()))?;

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| SecurityError::Tls(e.to_string()))?;

        Ok(Some(Arc::new(config)))
    }
}

// ---------------------------------------------------------------------------
// ACL
// ---------------------------------------------------------------------------

/// Permission flags for a user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclPermissions {
    /// Allowed command patterns (e.g., `["GET", "SET", "DEL"]` or `["*"]` for all).
    pub allowed_commands: Vec<String>,
    /// Allowed key patterns (glob style, e.g., `["user:*", "session:*"]`).
    pub allowed_keys: Vec<String>,
    /// Whether the user can access all channels (pub/sub, streams).
    pub all_channels: bool,
}

impl Default for AclPermissions {
    fn default() -> Self {
        Self {
            allowed_commands: vec!["*".into()],
            allowed_keys: vec!["*".into()],
            all_channels: true,
        }
    }
}

/// A user entry in the ACL system.
#[derive(Debug, Clone)]
pub struct AclUser {
    pub username: String,
    /// Hashed password (SHA-256 hex). Empty means no auth required.
    pub password_hash: Option<String>,
    pub enabled: bool,
    pub permissions: AclPermissions,
}

impl AclUser {
    /// Check if this user is allowed to run a given command on a given key.
    pub fn can_execute(&self, command: &str, key: &str) -> bool {
        if !self.enabled {
            return false;
        }

        let cmd_ok = self.permissions.allowed_commands.iter().any(|p| {
            p == "*" || p.eq_ignore_ascii_case(command)
        });

        let key_ok = self.permissions.allowed_keys.iter().any(|p| {
            p == "*" || simple_glob_match(p, key)
        });

        cmd_ok && key_ok
    }
}

/// Simple glob matching supporting only trailing `*`.
fn simple_glob_match(pattern: &str, value: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}

// ---------------------------------------------------------------------------
// ACL Manager
// ---------------------------------------------------------------------------

/// Thread-safe ACL manager.
pub struct AclManager {
    users: RwLock<Vec<AclUser>>,
}

impl AclManager {
    pub fn new() -> Self {
        // Default: single "default" user with full access, no password.
        let default_user = AclUser {
            username: "default".into(),
            password_hash: None,
            enabled: true,
            permissions: AclPermissions::default(),
        };
        Self {
            users: RwLock::new(vec![default_user]),
        }
    }

    /// Authenticate a user. Returns the user if credentials match.
    pub fn authenticate(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<AclUser, SecurityError> {
        let users = self.users.read();
        let user = users
            .iter()
            .find(|u| u.username == username)
            .ok_or_else(|| SecurityError::AuthFailed(username.into()))?;

        if !user.enabled {
            return Err(SecurityError::AuthFailed(username.into()));
        }

        match &user.password_hash {
            None => Ok(user.clone()),
            Some(expected) if expected == password_hash => Ok(user.clone()),
            _ => Err(SecurityError::AuthFailed(username.into())),
        }
    }

    /// Check if the default (unauthenticated) user can run a command.
    pub fn check_default(&self, command: &str, key: &str) -> bool {
        let users = self.users.read();
        users
            .iter()
            .find(|u| u.username == "default")
            .map(|u| u.can_execute(command, key))
            .unwrap_or(false)
    }

    /// Add or replace a user.
    pub fn set_user(&self, user: AclUser) {
        let mut users = self.users.write();
        if let Some(existing) = users.iter_mut().find(|u| u.username == user.username) {
            *existing = user;
        } else {
            users.push(user);
        }
    }

    /// Remove a user by username (cannot remove "default").
    pub fn remove_user(&self, username: &str) -> Result<(), SecurityError> {
        if username == "default" {
            return Err(SecurityError::AclParse(
                "cannot remove default user".into(),
            ));
        }
        let mut users = self.users.write();
        users.retain(|u| u.username != username);
        Ok(())
    }

    /// List all user names.
    pub fn list_users(&self) -> Vec<String> {
        self.users.read().iter().map(|u| u.username.clone()).collect()
    }
}

impl Default for AclManager {
    fn default() -> Self {
        Self::new()
    }
}
