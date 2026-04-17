// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Admin API error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// All errors that can occur in the Admin API.
#[derive(Debug, Error)]
pub enum AdminError {
    #[error("config file read failed: {0}")]
    ReadFile(#[from] std::io::Error),

    #[error("config parse failed: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("config validation failed: {0}")]
    Validation(String),

    #[error("authentication required")]
    Unauthorized,

    #[error("cluster not found: {0}")]
    ClusterNotFound(String),

    #[error("drain error: {0}")]
    Drain(String),

    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AdminError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AdminError::ClusterNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AdminError::ReadFile(_) | AdminError::Parse(_) | AdminError::Validation(_) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            AdminError::Drain(_) | AdminError::Serialize(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
        };
        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}
