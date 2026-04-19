// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Shared state plumbed into the admin API router.

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::providers::{ClusterProvider, ConfigDumper, HealthProvider, RuntimeProvider, StatsProvider};

/// Static server-info carried by `AdminApiState`.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub version: String,
    pub build_sha: String,
    pub hostname: String,
    pub started_at: DateTime<Utc>,
}

impl ServerInfo {
    /// Default server info sourced from `CARGO_PKG_VERSION` and the
    /// `ARMAGEDDON_BUILD_SHA` env var (falling back to "unknown").
    pub fn from_env() -> Self {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let build_sha = std::env::var("ARMAGEDDON_BUILD_SHA")
            .unwrap_or_else(|_| "unknown".to_string());
        let hostname = hostname_or_unknown();
        Self {
            version,
            build_sha,
            hostname,
            started_at: Utc::now(),
        }
    }
}

fn hostname_or_unknown() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Shared state injected into axum handlers.
pub struct AdminApiState {
    pub stats: Arc<dyn StatsProvider>,
    pub clusters: Arc<dyn ClusterProvider>,
    pub config: Arc<dyn ConfigDumper>,
    pub runtime: Arc<dyn RuntimeProvider>,
    pub health: Arc<dyn HealthProvider>,

    pub server_info: Arc<ServerInfo>,

    /// Current log-level string. Protected by a `parking_lot::Mutex`
    /// because we only need mutual exclusion for swap/read, no await
    /// points are held across it.
    log_level: Mutex<String>,
}

impl AdminApiState {
    /// Build a new `AdminApiState` with all providers wired in.
    pub fn new(
        stats: Arc<dyn StatsProvider>,
        clusters: Arc<dyn ClusterProvider>,
        config: Arc<dyn ConfigDumper>,
        runtime: Arc<dyn RuntimeProvider>,
        health: Arc<dyn HealthProvider>,
        server_info: ServerInfo,
        initial_log_level: impl Into<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            stats,
            clusters,
            config,
            runtime,
            health,
            server_info: Arc::new(server_info),
            log_level: Mutex::new(initial_log_level.into()),
        })
    }

    /// Current log level.
    pub fn current_log_level(&self) -> String {
        self.log_level.lock().clone()
    }

    /// Swap the stored log level, returning the previous value.
    ///
    /// NOTE: this updates the admin-api record only. Actual re-wiring of
    /// the `tracing_subscriber` reload handle is TODO and must happen in
    /// the main binary.
    pub fn swap_log_level(&self, new_level: String) -> Option<String> {
        let mut guard = self.log_level.lock();
        let previous = std::mem::replace(&mut *guard, new_level);
        Some(previous)
    }
}
