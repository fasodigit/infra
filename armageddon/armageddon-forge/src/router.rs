//! Request routing: path-prefix based matching to upstream clusters.
//!
//! Routes incoming requests by:
//! 1. Path prefix (longest prefix wins)
//! 2. Exact path
//! 3. Header matching
//! 4. Method filtering
//!
//! Special routes:
//! - /api/graphql -> DGS Gateway cluster
//! - content-type: application/grpc -> gRPC backend cluster

use armageddon_common::types::Route;
use std::collections::HashMap;

/// Routes incoming requests to upstream clusters.
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new(mut routes: Vec<Route>) -> Self {
        // Sort routes: exact paths first, then by longest prefix (descending)
        routes.sort_by(|a, b| {
            let a_exact = a.match_rule.path.is_some();
            let b_exact = b.match_rule.path.is_some();

            if a_exact && !b_exact {
                return std::cmp::Ordering::Less;
            }
            if !a_exact && b_exact {
                return std::cmp::Ordering::Greater;
            }

            // Among prefix routes, longer prefix wins (sort descending)
            let a_len = a.match_rule.prefix.as_ref().map_or(0, |p| p.len());
            let b_len = b.match_rule.prefix.as_ref().map_or(0, |p| p.len());
            b_len.cmp(&a_len)
        });

        Self { routes }
    }

    /// Match a request to a route, returning the matched route.
    ///
    /// Matching priority:
    /// 1. Exact path matches
    /// 2. Longest prefix matches
    /// 3. Header/method filters applied on top
    pub fn match_route(
        &self,
        method: &str,
        path: &str,
        headers: &HashMap<String, String>,
    ) -> Option<&Route> {
        self.routes.iter().find(|route| {
            let method_match = route.match_rule.methods.is_empty()
                || route
                    .match_rule
                    .methods
                    .iter()
                    .any(|m| m.eq_ignore_ascii_case(method));

            let path_match = if let Some(exact) = &route.match_rule.path {
                path == exact
            } else if let Some(prefix) = &route.match_rule.prefix {
                path.starts_with(prefix)
            } else {
                true
            };

            let header_match = route.match_rule.headers.iter().all(|(k, v)| {
                headers
                    .get(k)
                    .map_or(false, |hv| hv.eq_ignore_ascii_case(v))
            });

            method_match && path_match && header_match
        })
    }

    /// Detect if a request is gRPC (content-type: application/grpc).
    pub fn is_grpc(headers: &HashMap<String, String>) -> bool {
        headers
            .get("content-type")
            .map_or(false, |ct| ct.starts_with("application/grpc"))
    }

    /// Detect if a request is GraphQL (path = /api/graphql).
    pub fn is_graphql(path: &str) -> bool {
        path == "/api/graphql" || path.starts_with("/api/graphql?")
    }

    /// Update routes dynamically (from xDS RDS).
    pub fn update_routes(&mut self, routes: Vec<Route>) {
        let router = Router::new(routes);
        self.routes = router.routes; // re-sorted
        tracing::info!("FORGE router updated with {} routes", self.routes.len());
    }

    /// Number of configured routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::types::RouteMatch;

    fn make_route(name: &str, prefix: Option<&str>, path: Option<&str>, cluster: &str) -> Route {
        Route {
            name: name.to_string(),
            match_rule: RouteMatch {
                prefix: prefix.map(|s| s.to_string()),
                path: path.map(|s| s.to_string()),
                regex: None,
                headers: HashMap::new(),
                methods: vec![],
            },
            cluster: cluster.to_string(),
            timeout_ms: 30000,
            retry_policy: None,
        }
    }

    #[test]
    fn test_exact_path_takes_priority() {
        let routes = vec![
            make_route("api-catch-all", Some("/api/"), None, "api-cluster"),
            make_route("graphql", None, Some("/api/graphql"), "dgs-gateway"),
        ];
        let router = Router::new(routes);
        let headers = HashMap::new();

        let matched = router.match_route("POST", "/api/graphql", &headers).unwrap();
        assert_eq!(matched.cluster, "dgs-gateway");
    }

    #[test]
    fn test_longest_prefix_wins() {
        let routes = vec![
            make_route("api", Some("/api/"), None, "api-cluster"),
            make_route("api-v2", Some("/api/v2/"), None, "api-v2-cluster"),
        ];
        let router = Router::new(routes);
        let headers = HashMap::new();

        let matched = router.match_route("GET", "/api/v2/users", &headers).unwrap();
        assert_eq!(matched.cluster, "api-v2-cluster");

        let matched = router.match_route("GET", "/api/v1/users", &headers).unwrap();
        assert_eq!(matched.cluster, "api-cluster");
    }

    #[test]
    fn test_no_match() {
        let routes = vec![make_route("api", Some("/api/"), None, "api-cluster")];
        let router = Router::new(routes);
        let headers = HashMap::new();

        assert!(router.match_route("GET", "/other/path", &headers).is_none());
    }

    #[test]
    fn test_is_grpc() {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/grpc".to_string());
        assert!(Router::is_grpc(&headers));

        headers.insert(
            "content-type".to_string(),
            "application/grpc+proto".to_string(),
        );
        assert!(Router::is_grpc(&headers));

        headers.insert("content-type".to_string(), "application/json".to_string());
        assert!(!Router::is_grpc(&headers));
    }

    #[test]
    fn test_is_graphql() {
        assert!(Router::is_graphql("/api/graphql"));
        assert!(Router::is_graphql("/api/graphql?query=..."));
        assert!(!Router::is_graphql("/api/rest"));
    }
}
