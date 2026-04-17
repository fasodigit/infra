// SPDX-License-Identifier: AGPL-3.0-or-later
//! Geographic point: latitude, longitude, geohash, haversine distance.
//!
//! KAYA Geo module, RESP3-compatible GEO commands, 100% Rust, no external
//! geospatial service. The geohash encoder is a native base32 implementation
//! (Geohash / Niemeyer 2008) requiring no third-party crate.

use serde::{Deserialize, Serialize};

use crate::geo::error::GeoError;

/// Maximum absolute latitude supported. This is the Web Mercator cutoff used
/// by Redis/Valkey GEO so the geohash interleaving stays symmetric.
pub const MAX_LATITUDE: f64 = 85.05112878;

/// Maximum absolute longitude.
pub const MAX_LONGITUDE: f64 = 180.0;

/// Earth radius (mean sphere) in metres used for haversine distance.
/// Same constant as Redis/Valkey GEO for cross-compatibility.
pub const EARTH_RADIUS_M: f64 = 6_372_797.560_856;

/// Base32 alphabet used by the Geohash encoding (no `a`, `i`, `l`, `o`).
const BASE32: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// Reverse table: byte -> 5-bit value, or `0xFF` if not a geohash char.
const BASE32_DECODE: [u8; 256] = {
    let mut table = [0xFFu8; 256];
    let mut i = 0;
    while i < 32 {
        table[BASE32[i] as usize] = i as u8;
        i += 1;
    }
    table
};

/// A geographic point on the Earth surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    /// Build a validated point. Returns [`GeoError`] if latitude or longitude
    /// are outside the accepted envelope.
    #[tracing::instrument(level = "trace")]
    pub fn new(lat: f64, lon: f64) -> Result<Self, GeoError> {
        if !lat.is_finite() || lat.abs() > MAX_LATITUDE {
            return Err(GeoError::InvalidLatitude(lat));
        }
        if !lon.is_finite() || lon.abs() > MAX_LONGITUDE {
            return Err(GeoError::InvalidLongitude(lon));
        }
        Ok(Self { lat, lon })
    }

    /// Encode this point as a base32 geohash of the given character precision.
    /// A precision of 11 yields roughly 15 cm cells at the equator.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn geohash(&self, precision: u8) -> String {
        let precision = precision.clamp(1, 12) as usize;
        let mut lat_range = (-MAX_LATITUDE, MAX_LATITUDE);
        let mut lon_range = (-MAX_LONGITUDE, MAX_LONGITUDE);
        let mut out = String::with_capacity(precision);
        let mut bits: u8 = 0;
        let mut bit_count: u8 = 0;
        let mut even = true; // even => longitude bit, odd => latitude bit

        while out.len() < precision {
            if even {
                let mid = (lon_range.0 + lon_range.1) / 2.0;
                if self.lon >= mid {
                    bits = (bits << 1) | 1;
                    lon_range.0 = mid;
                } else {
                    bits <<= 1;
                    lon_range.1 = mid;
                }
            } else {
                let mid = (lat_range.0 + lat_range.1) / 2.0;
                if self.lat >= mid {
                    bits = (bits << 1) | 1;
                    lat_range.0 = mid;
                } else {
                    bits <<= 1;
                    lat_range.1 = mid;
                }
            }
            even = !even;
            bit_count += 1;
            if bit_count == 5 {
                out.push(BASE32[bits as usize] as char);
                bits = 0;
                bit_count = 0;
            }
        }
        out
    }

    /// Decode a base32 geohash string back to a point (cell centre).
    #[tracing::instrument(level = "trace")]
    pub fn from_geohash(hash: &str) -> Result<Self, GeoError> {
        if hash.is_empty() {
            return Err(GeoError::GeohashError("empty hash".into()));
        }
        let mut lat_range = (-MAX_LATITUDE, MAX_LATITUDE);
        let mut lon_range = (-MAX_LONGITUDE, MAX_LONGITUDE);
        let mut even = true;

        for &c in hash.as_bytes() {
            let val = BASE32_DECODE[c as usize];
            if val == 0xFF {
                return Err(GeoError::GeohashError(format!(
                    "invalid geohash character: 0x{c:02x}"
                )));
            }
            for i in (0..5).rev() {
                let bit = (val >> i) & 1;
                if even {
                    let mid = (lon_range.0 + lon_range.1) / 2.0;
                    if bit == 1 {
                        lon_range.0 = mid;
                    } else {
                        lon_range.1 = mid;
                    }
                } else {
                    let mid = (lat_range.0 + lat_range.1) / 2.0;
                    if bit == 1 {
                        lat_range.0 = mid;
                    } else {
                        lat_range.1 = mid;
                    }
                }
                even = !even;
            }
        }
        Ok(Self {
            lat: (lat_range.0 + lat_range.1) / 2.0,
            lon: (lon_range.0 + lon_range.1) / 2.0,
        })
    }

    /// Haversine distance in metres between two points on the Earth sphere.
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn distance_m(&self, other: &GeoPoint) -> f64 {
        let lat1 = self.lat.to_radians();
        let lat2 = other.lat.to_radians();
        let d_lat = (other.lat - self.lat).to_radians();
        let d_lon = (other.lon - self.lon).to_radians();

        let a = (d_lat / 2.0).sin().powi(2)
            + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        EARTH_RADIUS_M * c
    }

    /// Pack this point into a 52-bit interleaved geohash score, usable as a
    /// sorted-set score (fits in an `f64` mantissa losslessly).
    ///
    /// Bits are interleaved lon/lat, 26 bits each, resulting in a value in
    /// `[0, 2^52)`. This matches the Redis/Valkey GEO score scheme.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn to_u64_score(&self) -> u64 {
        let lat_frac = (self.lat + MAX_LATITUDE) / (2.0 * MAX_LATITUDE);
        let lon_frac = (self.lon + MAX_LONGITUDE) / (2.0 * MAX_LONGITUDE);
        let lat_bits = (lat_frac.clamp(0.0, 1.0) * ((1u64 << 26) as f64)) as u64;
        let lon_bits = (lon_frac.clamp(0.0, 1.0) * ((1u64 << 26) as f64)) as u64;
        let lat_bits = lat_bits.min((1u64 << 26) - 1);
        let lon_bits = lon_bits.min((1u64 << 26) - 1);
        interleave_52(lat_bits, lon_bits)
    }

    /// Inverse of [`to_u64_score`]: recover an approximate `GeoPoint` from
    /// the 52-bit interleaved geohash score.
    pub fn from_u64_score(score: u64) -> Self {
        let (lat_bits, lon_bits) = deinterleave_52(score);
        let lat_frac = (lat_bits as f64 + 0.5) / ((1u64 << 26) as f64);
        let lon_frac = (lon_bits as f64 + 0.5) / ((1u64 << 26) as f64);
        let lat = lat_frac * 2.0 * MAX_LATITUDE - MAX_LATITUDE;
        let lon = lon_frac * 2.0 * MAX_LONGITUDE - MAX_LONGITUDE;
        Self { lat, lon }
    }
}

/// Interleave two 26-bit values into a 52-bit result, latitude bit first.
fn interleave_52(lat: u64, lon: u64) -> u64 {
    let mut out = 0u64;
    for i in 0..26 {
        let lon_bit = (lon >> i) & 1;
        let lat_bit = (lat >> i) & 1;
        out |= lon_bit << (2 * i);
        out |= lat_bit << (2 * i + 1);
    }
    out
}

/// De-interleave a 52-bit interleaved value into `(lat, lon)` 26-bit ints.
fn deinterleave_52(score: u64) -> (u64, u64) {
    let mut lat = 0u64;
    let mut lon = 0u64;
    for i in 0..26 {
        lon |= ((score >> (2 * i)) & 1) << i;
        lat |= ((score >> (2 * i + 1)) & 1) << i;
    }
    (lat, lon)
}
