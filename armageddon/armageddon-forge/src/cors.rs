//! CORS handler: per-platform origin configuration.
//!
//! Checks Origin header against allowed origins per platform,
//! and builds Access-Control-* response headers.

use armageddon_common::types::CorsConfig;
use std::collections::HashMap;

/// Manages CORS policies per platform.
pub struct CorsHandler {
    /// Platform name -> CORS config.
    policies: HashMap<String, CorsConfig>,
    /// Flattened: all allowed origins across all platforms.
    all_origins: HashMap<String, String>,
}

impl CorsHandler {
    pub fn new(configs: Vec<(String, CorsConfig)>) -> Self {
        let mut all_origins = HashMap::new();
        for (platform, config) in &configs {
            for origin in &config.allowed_origins {
                all_origins.insert(origin.clone(), platform.clone());
            }
        }
        Self {
            policies: configs.into_iter().collect(),
            all_origins,
        }
    }

    /// Find which platform an origin belongs to.
    pub fn find_platform_for_origin(&self, origin: &str) -> Option<&str> {
        // Check exact match first
        if let Some(platform) = self.all_origins.get(origin) {
            return Some(platform.as_str());
        }
        // Check wildcard
        if self.all_origins.contains_key("*") {
            return self.all_origins.get("*").map(|s| s.as_str());
        }
        None
    }

    /// Check if an origin is allowed for a given platform.
    pub fn is_origin_allowed(&self, platform: &str, origin: &str) -> bool {
        self.policies.get(platform).map_or(false, |config| {
            config
                .allowed_origins
                .iter()
                .any(|allowed| allowed == "*" || allowed == origin)
        })
    }

    /// Check if an origin is allowed across any platform.
    pub fn is_origin_allowed_any(&self, origin: &str) -> bool {
        self.find_platform_for_origin(origin).is_some()
    }

    /// Build CORS response headers for a given origin.
    /// Automatically determines the platform from the origin.
    pub fn build_headers_for_origin(&self, origin: &str) -> Option<Vec<(String, String)>> {
        let platform = self.find_platform_for_origin(origin)?;
        self.build_headers(platform, origin)
    }

    /// Build CORS response headers for a given platform and origin.
    pub fn build_headers(&self, platform: &str, origin: &str) -> Option<Vec<(String, String)>> {
        let config = self.policies.get(platform)?;

        if !self.is_origin_allowed(platform, origin) {
            return None;
        }

        let mut headers = vec![
            (
                "Access-Control-Allow-Origin".to_string(),
                origin.to_string(),
            ),
            (
                "Access-Control-Allow-Methods".to_string(),
                config.allowed_methods.join(", "),
            ),
            (
                "Access-Control-Allow-Headers".to_string(),
                config.allowed_headers.join(", "),
            ),
            (
                "Access-Control-Max-Age".to_string(),
                config.max_age_secs.to_string(),
            ),
        ];

        if !config.exposed_headers.is_empty() {
            headers.push((
                "Access-Control-Expose-Headers".to_string(),
                config.exposed_headers.join(", "),
            ));
        }

        if config.allow_credentials {
            headers.push((
                "Access-Control-Allow-Credentials".to_string(),
                "true".to_string(),
            ));
        }

        // Vary header to indicate origin-based response
        headers.push(("Vary".to_string(), "Origin".to_string()));

        Some(headers)
    }

    /// Check if a request is a CORS preflight (OPTIONS with Origin + Access-Control-Request-Method).
    pub fn is_preflight(method: &str, headers: &HashMap<String, String>) -> bool {
        method.eq_ignore_ascii_case("OPTIONS")
            && headers.contains_key("origin")
            && headers.contains_key("access-control-request-method")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_cors() -> CorsHandler {
        CorsHandler::new(vec![
            (
                "web".to_string(),
                CorsConfig {
                    allowed_origins: vec![
                        "https://app.faso.dev".to_string(),
                        "https://admin.faso.dev".to_string(),
                    ],
                    allowed_methods: vec![
                        "GET".to_string(),
                        "POST".to_string(),
                        "PUT".to_string(),
                        "DELETE".to_string(),
                    ],
                    allowed_headers: vec![
                        "Content-Type".to_string(),
                        "Authorization".to_string(),
                    ],
                    exposed_headers: vec!["X-Request-Id".to_string()],
                    max_age_secs: 3600,
                    allow_credentials: true,
                },
            ),
            (
                "mobile".to_string(),
                CorsConfig {
                    allowed_origins: vec!["*".to_string()],
                    allowed_methods: vec!["GET".to_string(), "POST".to_string()],
                    allowed_headers: vec!["Content-Type".to_string()],
                    exposed_headers: vec![],
                    max_age_secs: 86400,
                    allow_credentials: false,
                },
            ),
        ])
    }

    #[test]
    fn test_origin_allowed() {
        let cors = make_test_cors();
        assert!(cors.is_origin_allowed("web", "https://app.faso.dev"));
        assert!(cors.is_origin_allowed("web", "https://admin.faso.dev"));
        assert!(!cors.is_origin_allowed("web", "https://evil.com"));
    }

    #[test]
    fn test_wildcard_origin() {
        let cors = make_test_cors();
        assert!(cors.is_origin_allowed("mobile", "https://anything.com"));
        assert!(cors.is_origin_allowed("mobile", "http://localhost:3000"));
    }

    #[test]
    fn test_build_headers() {
        let cors = make_test_cors();
        let headers = cors
            .build_headers("web", "https://app.faso.dev")
            .expect("should build headers");

        let header_map: HashMap<&str, &str> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        assert_eq!(
            header_map.get("Access-Control-Allow-Origin"),
            Some(&"https://app.faso.dev")
        );
        assert_eq!(
            header_map.get("Access-Control-Allow-Credentials"),
            Some(&"true")
        );
        assert!(header_map.contains_key("Access-Control-Max-Age"));
    }

    #[test]
    fn test_build_headers_denied() {
        let cors = make_test_cors();
        assert!(cors.build_headers("web", "https://evil.com").is_none());
    }

    #[test]
    fn test_is_preflight() {
        let mut headers = HashMap::new();
        headers.insert("origin".to_string(), "https://app.faso.dev".to_string());
        headers.insert(
            "access-control-request-method".to_string(),
            "POST".to_string(),
        );
        assert!(CorsHandler::is_preflight("OPTIONS", &headers));
        assert!(!CorsHandler::is_preflight("GET", &headers));
    }
}
