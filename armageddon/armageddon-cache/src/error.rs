// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

//! Error types for the response cache.

use thiserror::Error;

/// All errors that can be produced by the cache layer.
#[derive(Debug, Error)]
pub enum CacheError {
    /// KAYA storage or retrieval failure.
    #[error("KAYA error: {0}")]
    Kaya(String),

    /// JSON serialization / deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The cached payload was structurally invalid.
    #[error("invalid cached payload: {0}")]
    InvalidPayload(String),

    /// The response body exceeded the configured max_body_size.
    #[error("body too large: {size} > {limit}")]
    BodyTooLarge { size: usize, limit: usize },
}
