//! armageddon-veil: Header masking and security header injection.
//!
//! Removes sensitive server headers from responses and injects
//! security-related headers (CSP, HSTS, X-Frame-Options, etc.).

pub mod mask;

use armageddon_common::context::RequestContext;
use armageddon_config::security::{HeaderInjection, VeilConfig};

/// The VEIL header engine.
pub struct Veil {
    config: VeilConfig,
}

impl Veil {
    pub fn new(config: VeilConfig) -> Self {
        Self { config }
    }

    /// Process response headers: remove sensitive ones, inject security headers.
    pub fn process_response_headers(
        &self,
        headers: &mut Vec<(String, String)>,
    ) {
        if !self.config.enabled {
            return;
        }

        // Remove sensitive headers
        headers.retain(|(name, _)| {
            !self
                .config
                .remove_headers
                .iter()
                .any(|h| h.eq_ignore_ascii_case(name))
        });

        // Inject security headers
        for injection in &self.config.inject_headers {
            // Only inject if not already present
            if !headers.iter().any(|(name, _)| name.eq_ignore_ascii_case(&injection.name)) {
                headers.push((injection.name.clone(), injection.value.clone()));
            }
        }
    }

    /// Identity headers that must be stripped from incoming requests (anti-spoofing)
    /// and then injected from the authenticated RequestContext.
    const IDENTITY_HEADERS: &'static [&'static str] =
        &["x-user-id", "x-tenant-id", "x-user-role"];

    /// Inject identity headers into the request headers going upstream.
    ///
    /// 1. Strip any pre-existing identity headers (anti-spoofing).
    /// 2. Inject X-User-Id, X-Tenant-Id, X-User-Role from the RequestContext.
    pub fn inject_identity_headers(
        headers: &mut Vec<(String, String)>,
        ctx: &RequestContext,
    ) {
        // Strip spoofed identity headers
        headers.retain(|(name, _)| {
            !Self::IDENTITY_HEADERS
                .iter()
                .any(|h| h.eq_ignore_ascii_case(name))
        });

        // Inject authenticated identity
        if let Some(ref user_id) = ctx.user_id {
            headers.push(("x-user-id".to_string(), user_id.clone()));
        }
        if let Some(ref tenant_id) = ctx.tenant_id {
            headers.push(("x-tenant-id".to_string(), tenant_id.clone()));
        }
        for role in &ctx.user_roles {
            headers.push(("x-user-role".to_string(), role.clone()));
        }
    }

    /// Get the default security headers to inject.
    pub fn default_security_headers() -> Vec<HeaderInjection> {
        vec![
            HeaderInjection {
                name: "Strict-Transport-Security".to_string(),
                value: "max-age=31536000; includeSubDomains; preload".to_string(),
            },
            HeaderInjection {
                name: "X-Content-Type-Options".to_string(),
                value: "nosniff".to_string(),
            },
            HeaderInjection {
                name: "X-Frame-Options".to_string(),
                value: "DENY".to_string(),
            },
            HeaderInjection {
                name: "X-XSS-Protection".to_string(),
                value: "0".to_string(),
            },
            HeaderInjection {
                name: "Referrer-Policy".to_string(),
                value: "strict-origin-when-cross-origin".to_string(),
            },
            HeaderInjection {
                name: "Permissions-Policy".to_string(),
                value: "camera=(), microphone=(), geolocation=()".to_string(),
            },
            HeaderInjection {
                name: "Content-Security-Policy".to_string(),
                value: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::context::RequestContext;
    use armageddon_common::types::{
        ConnectionInfo, HttpRequest, HttpVersion, Protocol,
    };
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};

    fn make_test_ctx() -> RequestContext {
        let http_req = HttpRequest {
            method: "GET".to_string(),
            uri: "/api/test".to_string(),
            path: "/api/test".to_string(),
            query: None,
            headers: HashMap::new(),
            body: None,
            version: HttpVersion::Http11,
        };
        let conn = ConnectionInfo {
            client_ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            client_port: 12345,
            server_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            server_port: 8443,
            tls: None,
            ja3_fingerprint: None,
        };
        RequestContext::new(http_req, conn, Protocol::Http)
    }

    #[test]
    fn test_inject_identity_headers_strips_spoofed_and_injects() {
        let mut ctx = make_test_ctx();
        ctx.user_id = Some("user-123".to_string());
        ctx.tenant_id = Some("tenant-456".to_string());
        ctx.user_roles = vec!["admin".to_string(), "editor".to_string()];

        let mut headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-user-id".to_string(), "spoofed-id".to_string()),
            ("X-Tenant-Id".to_string(), "spoofed-tenant".to_string()),
            ("x-user-role".to_string(), "spoofed-role".to_string()),
        ];

        Veil::inject_identity_headers(&mut headers, &ctx);

        // Spoofed headers should be gone
        assert!(!headers.iter().any(|(_, v)| v == "spoofed-id"));
        assert!(!headers.iter().any(|(_, v)| v == "spoofed-tenant"));
        assert!(!headers.iter().any(|(_, v)| v == "spoofed-role"));

        // Legitimate content-type should remain
        assert!(headers.iter().any(|(k, v)| k == "content-type" && v == "application/json"));

        // Identity headers should be injected
        assert!(headers.iter().any(|(k, v)| k == "x-user-id" && v == "user-123"));
        assert!(headers.iter().any(|(k, v)| k == "x-tenant-id" && v == "tenant-456"));
        let roles: Vec<&str> = headers
            .iter()
            .filter(|(k, _)| k == "x-user-role")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(roles, vec!["admin", "editor"]);
    }

    #[test]
    fn test_inject_identity_headers_no_auth_no_injection() {
        let ctx = make_test_ctx();

        let mut headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-user-id".to_string(), "spoofed-id".to_string()),
        ];

        Veil::inject_identity_headers(&mut headers, &ctx);

        // Spoofed x-user-id should be removed
        assert!(!headers.iter().any(|(k, _)| k == "x-user-id"));
        // No identity headers injected since ctx has no user_id
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "content-type");
    }
}
