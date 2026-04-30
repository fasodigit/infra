// SPDX-License-Identifier: AGPL-3.0-or-later
//! Application-level error types for terroir-core.
//!
//! `AppError` maps to HTTP status codes via `IntoResponse`.
//! gRPC handlers convert `AppError` to `tonic::Status` manually.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

// ---------------------------------------------------------------------------
// AppError
// ---------------------------------------------------------------------------

/// Unified error type for all terroir-core handlers.
#[derive(Debug, Error)]
pub enum AppError {
    /// The requested resource does not exist (or was soft-deleted).
    #[error("not found: {0}")]
    NotFound(String),

    /// Input validation failed.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// The caller is not authenticated or the JWT is invalid.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// The caller does not have permission for this resource.
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// LWW version conflict: client sent a stale version.
    /// The current version on the server is returned in the body.
    #[error("stale LWW version: client={client}, server={server}")]
    StaleLww { client: i64, server: i64 },

    /// A conflicting resource already exists (e.g., duplicate idempotency key).
    #[error("conflict: {0}")]
    Conflict(String),

    /// Internal infrastructure error (DB, Vault, KAYA, Redpanda).
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            AppError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                json!({ "error": "not_found", "message": msg }),
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                json!({ "error": "bad_request", "message": msg }),
            ),
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "unauthorized", "message": msg }),
            ),
            AppError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                json!({ "error": "forbidden", "message": msg }),
            ),
            AppError::StaleLww { client, server } => (
                StatusCode::CONFLICT,
                json!({
                    "error": "stale_lww",
                    "message": "client version is stale",
                    "client_version": client,
                    "current": server
                }),
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                json!({ "error": "conflict", "message": msg }),
            ),
            AppError::Internal(e) => {
                tracing::error!(error = %e, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "error": "internal_error", "message": "an internal error occurred" }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

// Convenience conversion from sqlx errors.
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("row not found".into()),
            sqlx::Error::Database(ref db_err) => {
                // PostgreSQL SQLSTATE codes:
                //   23503 = foreign_key_violation → map to 404 (referenced row absent)
                //   23505 = unique_violation     → map to 409 (Conflict)
                //   23514 = check_violation      → map to 400 (BadRequest)
                match db_err.code().as_deref() {
                    Some("23503") => AppError::NotFound(format!(
                        "foreign key violation: {}",
                        db_err.message()
                    )),
                    Some("23505") => AppError::Conflict(format!(
                        "unique violation: {}",
                        db_err.message()
                    )),
                    Some("23514") => AppError::BadRequest(format!(
                        "check violation: {}",
                        db_err.message()
                    )),
                    _ => AppError::Internal(anyhow::anyhow!("database error: {}", e)),
                }
            }
            other => AppError::Internal(anyhow::anyhow!("database error: {}", other)),
        }
    }
}
