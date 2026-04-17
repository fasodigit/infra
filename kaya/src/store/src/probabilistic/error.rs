// SPDX-License-Identifier: AGPL-3.0-or-later
//! Error types for probabilistic data structures.

use thiserror::Error;

/// Errors produced by KAYA probabilistic structures (Cuckoo, HLL, CMS, TopK).
#[derive(Debug, Error)]
pub enum ProbabilisticError {
    /// The structure is full and cannot accept more items.
    #[error("probabilistic structure is full (max relocations exceeded)")]
    Full,

    /// An invalid precision parameter was provided (HyperLogLog).
    #[error("invalid precision: must be in [4, 18], got {0}")]
    InvalidPrecision(u8),

    /// Invalid dimensions for the structure (CMS width/depth, TopK, etc.).
    #[error("invalid dimensions: {0}")]
    InvalidDimensions(String),

    /// Deserialization failed.
    #[error("deserialize error: {0}")]
    Deserialize(String),
}
