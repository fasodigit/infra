// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-admin — DTOs for tenant onboarding endpoints.
//
// Serde request/response types. No prepared-statement-based validation;
// slug format is enforced by the DB CHECK constraint and validated here
// for early rejection before hitting Postgres.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Request — POST /admin/tenants
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    /// Tenant slug: 3-50 lowercase alphanumeric + underscore.
    /// Becomes the schema suffix: terroir_t_<slug>.
    pub slug: String,
    pub legal_name: String,
    pub country_iso2: String,
    pub region: Option<String>,
    /// Primary crop: coton / sesame / karite / anacarde / ...
    pub primary_crop: String,
}

impl CreateTenantRequest {
    /// Validate slug format (mirrors DB CHECK constraint).
    /// Returns Err with a descriptive message on invalid input.
    pub fn validate(&self) -> Result<(), String> {
        let slug = &self.slug;
        if slug.len() < 3 || slug.len() > 50 {
            return Err(format!("slug '{}' must be 3-50 characters", slug));
        }
        if !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!("slug '{}' must match ^[a-z0-9_]{{3,50}}$", slug));
        }
        if self.country_iso2.len() != 2 {
            return Err("country_iso2 must be exactly 2 characters".to_string());
        }
        if self.legal_name.trim().is_empty() {
            return Err("legal_name must not be empty".to_string());
        }
        if self.primary_crop.trim().is_empty() {
            return Err("primary_crop must not be empty".to_string());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Response — tenant detail (GET + 201 on create)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TenantResponse {
    pub id: Uuid,
    pub slug: String,
    pub legal_name: String,
    pub country_iso2: String,
    pub region: Option<String>,
    pub primary_crop: String,
    pub status: String,
    pub schema_name: String,
    pub audit_schema_name: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Response — paginated tenant list (GET /admin/tenants)
// Uses keyset pagination (created_at + id) to avoid O(N) COUNT scans.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TenantListResponse {
    pub items: Vec<TenantResponse>,
    /// Opaque cursor for next page: base64(created_at::ISO8601 + "," + id)
    pub next_cursor: Option<String>,
    pub limit: i64,
}

// ---------------------------------------------------------------------------
// Query params — GET /admin/tenants
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListTenantsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Keyset cursor from previous page's next_cursor field.
    pub cursor: Option<String>,
}

fn default_limit() -> i64 {
    50
}

// ---------------------------------------------------------------------------
// Error response
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            code: code.into(),
        }
    }
}
