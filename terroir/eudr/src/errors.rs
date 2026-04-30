// SPDX-License-Identifier: AGPL-3.0-or-later
//! Application-level error types for terroir-eudr.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("upstream service error: {0}")]
    Upstream(String),

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
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                json!({ "error": "conflict", "message": msg }),
            ),
            AppError::Upstream(msg) => (
                StatusCode::BAD_GATEWAY,
                json!({ "error": "upstream", "message": msg }),
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

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("row not found".into()),
            other => AppError::Internal(anyhow::anyhow!("database error: {}", other)),
        }
    }
}
