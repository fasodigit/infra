// SPDX-License-Identifier: AGPL-3.0-or-later
//! Error types for terroir-mobile-bff.
//!
//! `BffError` maps to HTTP status codes via `IntoResponse`.
//! gRPC errors from terroir-core (Tonic `Status`) are bubbled up via
//! `From<tonic::Status>` and translated to the closest BFF status.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BffError {
    /// The requested resource does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Input validation failed (e.g. batch > SYNC_BATCH_MAX_ITEMS).
    #[error("bad request: {0}")]
    BadRequest(String),

    /// Caller is not authenticated.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Caller exceeded the per-userId rate-limit.
    #[error("rate limit exceeded for user {user_id}")]
    RateLimited { user_id: String },

    /// Conflict (idempotent batch already processed).
    #[error("conflict: {0}")]
    Conflict(String),

    /// Upstream gRPC failure when calling terroir-core :8730.
    #[error("upstream gRPC error: {0}")]
    Upstream(String),

    /// Internal infrastructure failure (DB, KAYA, …).
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for BffError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            BffError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                json!({ "error": "not_found", "message": msg }),
            ),
            BffError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                json!({ "error": "bad_request", "message": msg }),
            ),
            BffError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "unauthorized", "message": msg }),
            ),
            BffError::RateLimited { user_id } => (
                StatusCode::TOO_MANY_REQUESTS,
                json!({ "error": "rate_limited", "user_id": user_id }),
            ),
            BffError::Conflict(msg) => (
                StatusCode::CONFLICT,
                json!({ "error": "conflict", "message": msg }),
            ),
            BffError::Upstream(msg) => {
                tracing::warn!(error = %msg, "upstream gRPC error");
                (
                    StatusCode::BAD_GATEWAY,
                    json!({ "error": "upstream", "message": msg }),
                )
            }
            BffError::Internal(e) => {
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

impl From<sqlx::Error> for BffError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => BffError::NotFound("row not found".into()),
            other => BffError::Internal(anyhow::anyhow!("database error: {}", other)),
        }
    }
}

impl From<tonic::Status> for BffError {
    fn from(s: tonic::Status) -> Self {
        match s.code() {
            tonic::Code::NotFound => BffError::NotFound(s.message().to_owned()),
            tonic::Code::InvalidArgument => BffError::BadRequest(s.message().to_owned()),
            tonic::Code::Unauthenticated => BffError::Unauthorized(s.message().to_owned()),
            tonic::Code::AlreadyExists => BffError::Conflict(s.message().to_owned()),
            _ => BffError::Upstream(format!("{}: {}", s.code(), s.message())),
        }
    }
}

impl From<tonic::transport::Error> for BffError {
    fn from(e: tonic::transport::Error) -> Self {
        BffError::Upstream(format!("transport: {e}"))
    }
}
