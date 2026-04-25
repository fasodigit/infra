// SPDX-License-Identifier: AGPL-3.0-or-later
//! CORS filter — pre-flight short-circuit + per-cluster origin policy.
//!
//! Per-cluster configuration is looked up using [`RequestCtx::cluster`]
//! (populated by the router filter).  A request whose cluster has no
//! explicit entry falls back to the special `"default"` cluster, or, if no
//! default is configured, the filter is a pass-through.
//!
//! ## Pre-flight
//!
//! `OPTIONS` requests bearing `Origin` + `Access-Control-Request-Method`
//! are short-circuited with a `204 No Content` reply carrying the
//! `Access-Control-*` headers.  The filter never touches the upstream for
//! pre-flight traffic.
//!
//! ## Simple requests
//!
//! For non-pre-flight requests, the filter inspects the response on the way
//! back to the client and injects `Access-Control-Allow-Origin`,
//! `Access-Control-Allow-Credentials` (if configured and the origin is
//! allowed), `Vary: Origin`, and `Access-Control-Expose-Headers`.
//!
//! ## Safety: wildcard `*` + `allow_credentials = true`
//!
//! This combination is forbidden by the Fetch spec and is explicitly
//! rejected by [`CorsConfig::validate`].  Construction helpers that
//! silently accept it are a bug.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use http::Method;
use pingora::http::ResponseHeader;

use crate::pingora::ctx::RequestCtx;
use crate::pingora::filters::{Decision, ForgeFilter};

/// Per-cluster CORS policy.
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins.  Use `"*"` for wildcard (incompatible with
    /// `allow_credentials = true`).
    pub allowed_origins: Vec<String>,
    /// Allowed request methods for pre-flight echo.
    pub allowed_methods: Vec<Method>,
    /// Allowed request headers for pre-flight echo.
    pub allowed_headers: Vec<String>,
    /// Response headers exposed to JS (`Access-Control-Expose-Headers`).
    pub expose_headers: Vec<String>,
    /// Emit `Access-Control-Allow-Credentials: true`.
    pub allow_credentials: bool,
    /// `Access-Control-Max-Age` value in seconds.
    pub max_age: u32,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: Vec::new(),
            allowed_methods: vec![Method::GET, Method::POST, Method::OPTIONS],
            allowed_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
            expose_headers: Vec::new(),
            allow_credentials: false,
            max_age: 600,
        }
    }
}

/// Reason a [`CorsConfig`] failed validation.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CorsConfigError {
    /// `"*"` + `allow_credentials = true` violates the Fetch spec.
    #[error("wildcard origin \"*\" is incompatible with allow_credentials = true")]
    WildcardWithCredentials,
}

impl CorsConfig {
    /// Validate invariants.  Call at config-load time.
    pub fn validate(&self) -> Result<(), CorsConfigError> {
        if self.allow_credentials && self.allowed_origins.iter().any(|o| o == "*") {
            return Err(CorsConfigError::WildcardWithCredentials);
        }
        Ok(())
    }

    /// Whether `origin` is accepted by this policy.
    ///
    /// Both the configured origins and the request origin are normalised
    /// (default ports stripped) before comparison so that
    /// `https://app.faso.dev:443` matches `https://app.faso.dev`.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        let norm = normalize_origin(origin);
        self.allowed_origins
            .iter()
            .any(|o| o == "*" || normalize_origin(o) == norm)
    }

    /// Echo header value for `Access-Control-Allow-Origin`:
    ///   - if wildcard is allowed and credentials disabled, return `"*"`;
    ///   - else, return the verbatim origin (assumes caller already checked
    ///     [`Self::is_origin_allowed`]).
    fn allow_origin_value(&self, origin: &str) -> String {
        if !self.allow_credentials && self.allowed_origins.iter().any(|o| o == "*") {
            "*".to_string()
        } else {
            origin.to_string()
        }
    }
}

/// Bundle of per-cluster CORS configs.
#[derive(Debug, Clone, Default)]
pub struct CorsConfigMap {
    /// Cluster → config.  The special key `"default"` is used as a fallback
    /// when no cluster-specific entry exists.
    pub by_cluster: HashMap<String, CorsConfig>,
}

impl CorsConfigMap {
    /// Build a new config map, validating every entry.
    pub fn new(
        entries: impl IntoIterator<Item = (String, CorsConfig)>,
    ) -> Result<Self, CorsConfigError> {
        let mut by_cluster = HashMap::new();
        for (k, v) in entries {
            v.validate()?;
            by_cluster.insert(k, v);
        }
        Ok(Self { by_cluster })
    }

    /// Look up the effective config for a cluster: exact match → `default`
    /// → `None`.
    pub fn lookup(&self, cluster: &str) -> Option<&CorsConfig> {
        self.by_cluster
            .get(cluster)
            .or_else(|| self.by_cluster.get("default"))
    }
}

/// CORS filter — per-cluster origin policy enforcement with hot-reload.
pub struct CorsFilter {
    config: Arc<ArcSwap<CorsConfigMap>>,
}

impl std::fmt::Debug for CorsFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorsFilter")
            .field("clusters", &self.config.load().by_cluster.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CorsFilter {
    /// Build a new filter from a pre-validated config map.
    pub fn new(map: CorsConfigMap) -> Self {
        Self {
            config: Arc::new(ArcSwap::from_pointee(map)),
        }
    }

    /// Hot-reload the config map (xDS push).
    pub fn update(&self, map: CorsConfigMap) {
        self.config.store(Arc::new(map));
        tracing::info!("pingora cors config hot-reloaded");
    }

    /// Snapshot the active config map.
    pub fn snapshot(&self) -> Arc<CorsConfigMap> {
        self.config.load_full()
    }

    /// Build a 204 pre-flight response for a given origin + config.
    fn build_preflight(
        config: &CorsConfig,
        origin: &str,
    ) -> Result<Box<ResponseHeader>, ()> {
        let mut resp = ResponseHeader::build(204u16, Some(8)).map_err(|_| ())?;
        resp.insert_header("Access-Control-Allow-Origin", config.allow_origin_value(origin))
            .map_err(|_| ())?;
        if config.allow_credentials {
            resp.insert_header("Access-Control-Allow-Credentials", "true")
                .map_err(|_| ())?;
        }
        let methods = config
            .allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        resp.insert_header("Access-Control-Allow-Methods", methods)
            .map_err(|_| ())?;
        if !config.allowed_headers.is_empty() {
            resp.insert_header(
                "Access-Control-Allow-Headers",
                config.allowed_headers.join(", "),
            )
            .map_err(|_| ())?;
        }
        resp.insert_header("Access-Control-Max-Age", config.max_age.to_string())
            .map_err(|_| ())?;
        resp.insert_header("Vary", "Origin")
            .map_err(|_| ())?;
        resp.insert_header("Content-Length", "0")
            .map_err(|_| ())?;
        Ok(Box::new(resp))
    }
}

/// Normalise an origin by stripping the default port for its scheme.
///
/// `https://app.faso.dev:443` becomes `https://app.faso.dev` and
/// `http://localhost:80` becomes `http://localhost`.  Origins without a
/// port or with non-default ports are returned unchanged.
fn normalize_origin(origin: &str) -> String {
    if let Some(rest) = origin.strip_prefix("https://") {
        rest.strip_suffix(":443")
            .map(|h| format!("https://{h}"))
            .unwrap_or_else(|| origin.to_string())
    } else if let Some(rest) = origin.strip_prefix("http://") {
        rest.strip_suffix(":80")
            .map(|h| format!("http://{h}"))
            .unwrap_or_else(|| origin.to_string())
    } else {
        origin.to_string()
    }
}

/// Stash the request origin between `on_request` and `on_response` using the
/// typed [`RequestCtx::cors_origin`] slot (M1 consolidation — replaces the
/// previous stringly-typed `"cors:origin:"` prefix in `feature_flags`).
fn stash_origin(ctx: &mut RequestCtx, origin: &str) {
    ctx.cors_origin = Some(origin.to_string());
}

fn stashed_origin(ctx: &RequestCtx) -> Option<&str> {
    ctx.cors_origin.as_deref()
}

#[async_trait::async_trait]
impl ForgeFilter for CorsFilter {
    fn name(&self) -> &'static str {
        "cors"
    }

    async fn on_request(
        &self,
        session: &mut pingora_proxy::Session,
        ctx: &mut RequestCtx,
    ) -> Decision {
        let map = self.config.load();
        let config = match map.lookup(&ctx.cluster) {
            Some(c) => c,
            None => return Decision::Continue,
        };

        let req = session.req_header();
        let method = req.method.clone();
        let headers = &req.headers;

        let origin = match headers.get("origin").and_then(|v| v.to_str().ok()) {
            Some(o) => o.to_string(),
            None => return Decision::Continue,
        };

        // Pre-flight: OPTIONS + Origin + Access-Control-Request-Method.
        let is_preflight =
            method == Method::OPTIONS && headers.contains_key("access-control-request-method");

        if is_preflight {
            if !config.is_origin_allowed(&origin) {
                // Spec says: return 204 without ACAO; the browser will then
                // reject.  We return a plain 204 with no CORS headers.
                return match ResponseHeader::build(204u16, Some(1)) {
                    Ok(mut r) => {
                        let _ = r.insert_header("Content-Length", "0");
                        Decision::ShortCircuit(Box::new(r))
                    }
                    Err(_) => Decision::Continue,
                };
            }
            return match Self::build_preflight(config, &origin) {
                Ok(resp) => Decision::ShortCircuit(resp),
                Err(()) => {
                    tracing::warn!("cors: failed to build preflight — continuing");
                    Decision::Continue
                }
            };
        }

        // Simple request: stash the origin for on_response.
        stash_origin(ctx, &origin);
        Decision::Continue
    }

    async fn on_response(
        &self,
        _session: &mut pingora_proxy::Session,
        res: &mut ResponseHeader,
        ctx: &mut RequestCtx,
    ) -> Decision {
        let map = self.config.load();
        let config = match map.lookup(&ctx.cluster) {
            Some(c) => c,
            None => return Decision::Continue,
        };

        let origin = match stashed_origin(ctx) {
            Some(o) => o.to_string(),
            None => return Decision::Continue,
        };

        if !config.is_origin_allowed(&origin) {
            return Decision::Continue;
        }

        let _ = res.insert_header(
            "Access-Control-Allow-Origin",
            config.allow_origin_value(&origin),
        );
        if config.allow_credentials {
            let _ = res.insert_header("Access-Control-Allow-Credentials", "true");
        }
        if !config.expose_headers.is_empty() {
            let _ = res.insert_header(
                "Access-Control-Expose-Headers",
                config.expose_headers.join(", "),
            );
        }
        // Always mark the response as varying on Origin so downstream
        // caches don't serve a leaked ACAO to a different origin.
        let _ = res.append_header("Vary", "Origin");

        Decision::Continue
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_credentialed_allowlist() -> CorsConfig {
        CorsConfig {
            allowed_origins: vec!["https://app.faso.dev".to_string()],
            allowed_methods: vec![Method::GET, Method::POST, Method::OPTIONS],
            allowed_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
            expose_headers: vec!["X-Forge-Id".to_string()],
            allow_credentials: true,
            max_age: 600,
        }
    }

    fn cfg_wildcard_no_creds() -> CorsConfig {
        CorsConfig {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![Method::GET, Method::POST],
            allowed_headers: vec![],
            expose_headers: vec![],
            allow_credentials: false,
            max_age: 86400,
        }
    }

    // --- config validation ---------------------------------------------------

    #[test]
    fn validate_rejects_wildcard_with_credentials() {
        let mut c = cfg_wildcard_no_creds();
        c.allow_credentials = true;
        assert_eq!(c.validate(), Err(CorsConfigError::WildcardWithCredentials));
    }

    #[test]
    fn validate_accepts_wildcard_without_credentials() {
        assert!(cfg_wildcard_no_creds().validate().is_ok());
    }

    #[test]
    fn validate_accepts_credentialed_allowlist() {
        assert!(cfg_credentialed_allowlist().validate().is_ok());
    }

    #[test]
    fn config_map_rejects_invalid_entry() {
        let mut bad = cfg_wildcard_no_creds();
        bad.allow_credentials = true;
        let res = CorsConfigMap::new([("api".to_string(), bad)]);
        assert!(matches!(res, Err(CorsConfigError::WildcardWithCredentials)));
    }

    // --- origin allow-list ---------------------------------------------------

    #[test]
    fn is_origin_allowed_exact() {
        let c = cfg_credentialed_allowlist();
        assert!(c.is_origin_allowed("https://app.faso.dev"));
        assert!(!c.is_origin_allowed("https://evil.example"));
    }

    #[test]
    fn is_origin_allowed_wildcard() {
        let c = cfg_wildcard_no_creds();
        assert!(c.is_origin_allowed("https://anywhere.test"));
        assert!(c.is_origin_allowed("http://localhost:3000"));
    }

    // --- allow_origin_value -------------------------------------------------

    #[test]
    fn allow_origin_echoes_origin_for_credentialed_allowlist() {
        let c = cfg_credentialed_allowlist();
        assert_eq!(c.allow_origin_value("https://app.faso.dev"), "https://app.faso.dev");
    }

    #[test]
    fn allow_origin_is_wildcard_when_no_credentials() {
        let c = cfg_wildcard_no_creds();
        assert_eq!(c.allow_origin_value("https://foo.test"), "*");
    }

    // --- preflight response builder -----------------------------------------

    #[test]
    fn build_preflight_204_with_allowed_origin() {
        let c = cfg_credentialed_allowlist();
        let resp = CorsFilter::build_preflight(&c, "https://app.faso.dev")
            .expect("preflight builds");
        assert_eq!(resp.status.as_u16(), 204);
        let hdrs = &resp.headers;
        assert_eq!(
            hdrs.get("access-control-allow-origin").unwrap(),
            "https://app.faso.dev"
        );
        assert_eq!(hdrs.get("access-control-allow-credentials").unwrap(), "true");
        assert!(hdrs.get("access-control-allow-methods").is_some());
        assert!(hdrs.get("vary").is_some());
        assert_eq!(hdrs.get("content-length").unwrap(), "0");
    }

    #[test]
    fn build_preflight_wildcard_when_no_credentials() {
        let c = cfg_wildcard_no_creds();
        let resp = CorsFilter::build_preflight(&c, "https://foo.test").unwrap();
        assert_eq!(resp.headers.get("access-control-allow-origin").unwrap(), "*");
        assert!(resp.headers.get("access-control-allow-credentials").is_none());
    }

    // --- per-cluster config map lookup --------------------------------------

    #[test]
    fn lookup_falls_back_to_default_cluster() {
        let map = CorsConfigMap::new([
            ("default".to_string(), cfg_wildcard_no_creds()),
            ("api".to_string(), cfg_credentialed_allowlist()),
        ])
        .unwrap();
        // exact hit
        assert!(map.lookup("api").is_some());
        assert!(map.lookup("api").unwrap().allow_credentials);
        // fallback
        assert!(map.lookup("unknown-cluster").is_some());
        assert!(!map.lookup("unknown-cluster").unwrap().allow_credentials);
    }

    #[test]
    fn lookup_returns_none_when_no_default() {
        let map = CorsConfigMap::new([("api".to_string(), cfg_credentialed_allowlist())]).unwrap();
        assert!(map.lookup("unknown").is_none());
    }

    // --- filter construction + hot-reload -----------------------------------

    #[test]
    fn filter_construction_and_hot_reload() {
        let map1 = CorsConfigMap::new([("api".to_string(), cfg_credentialed_allowlist())]).unwrap();
        let filter = CorsFilter::new(map1);
        assert_eq!(filter.name(), "cors");
        let snap = filter.snapshot();
        assert!(snap.lookup("api").unwrap().allow_credentials);

        let map2 = CorsConfigMap::new([("api".to_string(), cfg_wildcard_no_creds())]).unwrap();
        filter.update(map2);
        let snap2 = filter.snapshot();
        assert!(!snap2.lookup("api").unwrap().allow_credentials);
    }

    // --- ctx origin stash helper --------------------------------------------

    // --- origin normalization -------------------------------------------------

    #[test]
    fn normalize_strips_default_https_port() {
        assert_eq!(normalize_origin("https://app.faso.dev:443"), "https://app.faso.dev");
    }

    #[test]
    fn normalize_strips_default_http_port() {
        assert_eq!(normalize_origin("http://localhost:80"), "http://localhost");
    }

    #[test]
    fn normalize_keeps_non_default_port() {
        assert_eq!(normalize_origin("https://app.faso.dev:8443"), "https://app.faso.dev:8443");
    }

    #[test]
    fn normalize_keeps_no_port() {
        assert_eq!(normalize_origin("https://app.faso.dev"), "https://app.faso.dev");
    }

    #[test]
    fn is_origin_allowed_normalizes_port() {
        let c = cfg_credentialed_allowlist(); // allows "https://app.faso.dev"
        assert!(c.is_origin_allowed("https://app.faso.dev:443"),
            "https://app.faso.dev:443 must match configured https://app.faso.dev");
    }

    #[test]
    fn origin_stash_round_trip() {
        let mut ctx = RequestCtx::default();
        stash_origin(&mut ctx, "https://a.test");
        assert_eq!(stashed_origin(&ctx), Some("https://a.test"));
        // Overwrite-semantics — second stash replaces first.
        stash_origin(&mut ctx, "https://b.test");
        assert_eq!(stashed_origin(&ctx), Some("https://b.test"));
        // feature_flags is not polluted by the origin stash.
        assert!(
            ctx.feature_flags.is_empty(),
            "cors_origin uses a typed slot, not feature_flags"
        );
    }
}
