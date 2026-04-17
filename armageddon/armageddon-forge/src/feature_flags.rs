// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Feature flags gateway middleware. Evaluates GrowthBook flags via KAYA cache
// (TTL 30s) and injects `X-Faso-Flags: flag1=true,flag2=false` header before
// forwarding upstream.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arc_swap::ArcSwap;
use parking_lot::Mutex;
use tracing::{debug, warn};

/// Cached feature-flag snapshot. Refreshed from KAYA every `ttl`.
#[derive(Clone, Default)]
pub struct FlagSnapshot {
    pub flags: HashMap<String, bool>,
    pub fetched_at: Option<Instant>,
}

pub struct FeatureFlagService {
    cache: ArcSwap<FlagSnapshot>,
    kaya_key: String,
    ttl: Duration,
    stale_ttl: Duration,
    /// Circuit: last fetch error timestamp, to avoid hammering KAYA when down.
    last_error: Mutex<Option<Instant>>,
}

impl FeatureFlagService {
    pub fn new(kaya_key: impl Into<String>) -> Self {
        Self {
            cache: ArcSwap::from_pointee(FlagSnapshot::default()),
            kaya_key: kaya_key.into(),
            ttl: Duration::from_secs(30),
            stale_ttl: Duration::from_secs(3600),
            last_error: Mutex::new(None),
        }
    }

    /// Evaluate a flag with a default. Returns cached value if still fresh;
    /// otherwise schedules a background refresh (fire-and-forget).
    pub fn is_enabled(self: &Arc<Self>, flag: &str, default: bool) -> bool {
        let snap = self.cache.load();
        let value = snap.flags.get(flag).copied().unwrap_or(default);

        let needs_refresh = snap
            .fetched_at
            .map(|t| t.elapsed() > self.ttl)
            .unwrap_or(true);
        if needs_refresh {
            let me = Arc::clone(self);
            tokio::spawn(async move { me.refresh().await });
        }
        value
    }

    /// Build the `X-Faso-Flags` header value.
    pub fn build_header(&self, flags_of_interest: &[&str]) -> String {
        let snap = self.cache.load();
        flags_of_interest
            .iter()
            .map(|f| format!("{}={}", f, snap.flags.get(*f).copied().unwrap_or(false)))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Pull fresh flags from KAYA. Placeholder — wire to `armageddon-nexus::kaya::KayaClient` in production.
    async fn refresh(self: Arc<Self>) {
        if let Some(t) = *self.last_error.lock() {
            if t.elapsed() < Duration::from_secs(5) {
                return; // circuit: cool down.
            }
        }
        match self.fetch_from_kaya().await {
            Ok(snap) => {
                self.cache.store(Arc::new(snap));
                *self.last_error.lock() = None;
                debug!("feature-flags refreshed");
            }
            Err(e) => {
                warn!(error = %e, "feature-flag refresh failed, keeping cached snapshot");
                *self.last_error.lock() = Some(Instant::now());
            }
        }
    }

    async fn fetch_from_kaya(&self) -> Result<FlagSnapshot, anyhow::Error> {
        // TODO: integrate with armageddon-nexus::kaya::KayaClient::get::<String>(&self.kaya_key).await
        // For now return empty snapshot (feature-flags not yet wired to GrowthBook cache).
        Ok(FlagSnapshot {
            flags: HashMap::new(),
            fetched_at: Some(Instant::now()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_when_missing() {
        let svc = Arc::new(FeatureFlagService::new("faso:flags:prod"));
        assert!(!svc.is_enabled("new-order-flow", false));
        assert!(svc.is_enabled("enable-2fa", true));
    }

    #[tokio::test]
    async fn header_format() {
        let svc = Arc::new(FeatureFlagService::new("faso:flags:prod"));
        let header = svc.build_header(&["flag1", "flag2"]);
        assert_eq!(header, "flag1=false,flag2=false");
    }
}
