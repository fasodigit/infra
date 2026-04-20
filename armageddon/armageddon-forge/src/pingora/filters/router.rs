// SPDX-License-Identifier: AGPL-3.0-or-later
//! Router filter — resolves the downstream cluster from the request path.
//!
//! Matching order: **exact → prefix (longest first) → regex → default**.
//! The resolved cluster name is written to [`RequestCtx::cluster`] so the
//! gateway's `upstream_peer` selector can consult it without re-parsing the
//! path.
//!
//! The route table is held behind [`ArcSwap`] for lock-free hot-reload
//! (designed so that a future xDS RDS push can call [`RouterFilter::update`]
//! without quiescing the gateway).

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use regex::Regex;

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Static route table: exact match → prefix match → regex match → default.
///
/// Each entry maps to the **resolved cluster name** (the string the upstream
/// selector looks up in [`crate::pingora::gateway::UpstreamRegistry`]).
///
/// Prefix entries are sorted internally by length descending so that a
/// request like `/api/v2/users` hits `/api/v2/` rather than `/api/`.
#[derive(Debug, Clone, Default)]
pub struct RouteTable {
    /// Exact path → cluster.
    pub exact: HashMap<String, String>,
    /// (prefix, cluster) pairs — longest prefix wins.
    pub prefix: Vec<(String, String)>,
    /// (regex, cluster) pairs — evaluated in insertion order.
    pub regex: Vec<(Regex, String)>,
    /// Cluster name used when no rule matches.
    pub default_cluster: String,
}

impl RouteTable {
    /// Build a new route table, sorting prefixes longest-first.
    pub fn new(
        exact: HashMap<String, String>,
        mut prefix: Vec<(String, String)>,
        regex: Vec<(Regex, String)>,
        default_cluster: impl Into<String>,
    ) -> Self {
        prefix.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        Self {
            exact,
            prefix,
            regex,
            default_cluster: default_cluster.into(),
        }
    }

    /// Resolve a path to a cluster name.
    ///
    /// Falls back to `default_cluster` if no rule matches — never returns
    /// an empty string for a well-formed table.
    pub fn resolve(&self, path: &str) -> &str {
        if let Some(cluster) = self.exact.get(path) {
            return cluster.as_str();
        }
        for (p, cluster) in &self.prefix {
            if path.starts_with(p.as_str()) {
                return cluster.as_str();
            }
        }
        for (re, cluster) in &self.regex {
            if re.is_match(path) {
                return cluster.as_str();
            }
        }
        self.default_cluster.as_str()
    }
}

/// Router filter — populates `RequestCtx::cluster` from the active
/// [`RouteTable`].
///
/// Hot-reload: call [`RouterFilter::update`] to swap the table without
/// blocking in-flight requests (atomic `ArcSwap::store`).
#[derive(Debug)]
pub struct RouterFilter {
    table: Arc<ArcSwap<RouteTable>>,
}

impl RouterFilter {
    /// Build a new router filter from an initial [`RouteTable`].
    pub fn new(table: RouteTable) -> Self {
        Self {
            table: Arc::new(ArcSwap::from_pointee(table)),
        }
    }

    /// Replace the active route table atomically (xDS RDS push).
    pub fn update(&self, new_table: RouteTable) {
        self.table.store(Arc::new(new_table));
        tracing::info!("pingora router table hot-reloaded");
    }

    /// Return a snapshot of the current table.  Primarily for tests and
    /// admin-API introspection.
    pub fn snapshot(&self) -> Arc<RouteTable> {
        self.table.load_full()
    }
}

#[async_trait::async_trait]
impl ForgeFilter for RouterFilter {
    fn name(&self) -> &'static str {
        "router"
    }

    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        let table = self.table.load();
        let path = session.req_header().uri.path();
        let cluster = table.resolve(path);
        ctx.cluster = cluster.to_string();
        tracing::trace!(path, cluster, "router resolved cluster");
        Decision::Continue
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn prefix_vec(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn regex_vec(pairs: &[(&str, &str)]) -> Vec<(Regex, String)> {
        pairs
            .iter()
            .map(|(k, v)| (Regex::new(k).expect("test regex"), (*v).to_string()))
            .collect()
    }

    fn sample_table() -> RouteTable {
        RouteTable::new(
            exact_map(&[
                ("/api/graphql", "dgs-gateway"),
                ("/health/alive", "internal-health"),
            ]),
            prefix_vec(&[
                ("/api/v2/", "api-v2"),
                ("/api/", "api-v1"),
                ("/static/", "cdn"),
            ]),
            regex_vec(&[
                // gRPC: /<pkg>.<Svc>/<Method>  — match on a dot in the first segment
                (r"^/[a-zA-Z_][\w\.]*\.[A-Z][\w]*/[A-Z][\w]*$", "grpc-backend"),
            ]),
            "default",
        )
    }

    // --- unit: pure resolution (no Session needed) ---------------------------

    #[test]
    fn resolves_exact_match() {
        let t = sample_table();
        assert_eq!(t.resolve("/api/graphql"), "dgs-gateway");
    }

    #[test]
    fn resolves_longest_prefix_first() {
        let t = sample_table();
        assert_eq!(t.resolve("/api/v2/users"), "api-v2");
        assert_eq!(t.resolve("/api/v1/users"), "api-v1");
    }

    #[test]
    fn resolves_grpc_regex() {
        let t = sample_table();
        // Typical gRPC path: /faso.auth.v1.AuthService/Login
        assert_eq!(
            t.resolve("/faso.auth.v1.AuthService/Login"),
            "grpc-backend"
        );
    }

    #[test]
    fn grpc_web_text_path_does_not_escape_prefix() {
        // gRPC-Web hits the same /pkg.Svc/Method shape; ensure it still
        // resolves to grpc-backend and not `default`.
        let t = sample_table();
        assert_eq!(t.resolve("/pkg.Svc/Method"), "grpc-backend");
    }

    #[test]
    fn falls_back_to_default_cluster() {
        let t = sample_table();
        assert_eq!(t.resolve("/unknown/thing"), "default");
    }

    #[test]
    fn exact_wins_over_prefix_of_same_path() {
        let mut ex = HashMap::new();
        ex.insert("/api/".to_string(), "exact-cluster".to_string());
        let t = RouteTable::new(
            ex,
            prefix_vec(&[("/api/", "prefix-cluster")]),
            vec![],
            "default",
        );
        assert_eq!(t.resolve("/api/"), "exact-cluster");
    }

    // --- filter construction + hot-reload ------------------------------------

    #[test]
    fn filter_construction_and_hot_reload() {
        let filter = RouterFilter::new(sample_table());
        assert_eq!(filter.name(), "router");
        let snap1 = filter.snapshot();
        assert_eq!(snap1.resolve("/api/v2/x"), "api-v2");

        // Swap in a new table.
        let new_table = RouteTable::new(
            exact_map(&[]),
            prefix_vec(&[("/api/", "brand-new-cluster")]),
            vec![],
            "default",
        );
        filter.update(new_table);
        let snap2 = filter.snapshot();
        assert_eq!(snap2.resolve("/api/v2/x"), "brand-new-cluster");
    }
}
