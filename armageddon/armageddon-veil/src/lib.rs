//! armageddon-veil: Header masking and security header injection.
//!
//! Removes sensitive server headers from responses and injects
//! security-related headers (CSP, HSTS, X-Frame-Options, etc.).

pub mod mask;

use armageddon_common::error::Result;
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
