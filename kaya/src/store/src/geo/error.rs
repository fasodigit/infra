// SPDX-License-Identifier: AGPL-3.0-or-later
//! Error types for the KAYA Geo module.
//!
//! KAYA Geo module, RESP3-compatible GEO commands, 100% Rust, no external
//! geospatial service.

use thiserror::Error;

/// Errors produced by the KAYA Geo index (GEOADD / GEOSEARCH / GEODIST / ...).
#[derive(Debug, Error)]
pub enum GeoError {
    /// Latitude outside the supported web-mercator compatible range
    /// `|lat| <= 85.05112878`.
    #[error("invalid latitude: {0} (expected |lat| <= 85.05112878)")]
    InvalidLatitude(f64),

    /// Longitude outside `[-180.0, 180.0]`.
    #[error("invalid longitude: {0} (expected |lon| <= 180.0)")]
    InvalidLongitude(f64),

    /// Search radius was negative or not finite.
    #[error("invalid radius: {0} (expected finite and positive)")]
    InvalidRadius(f64),

    /// The queried member does not exist in the geo index.
    #[error("member not found in geo index")]
    MemberNotFound,

    /// Geohash encode/decode error.
    #[error("geohash error: {0}")]
    GeohashError(String),
}
