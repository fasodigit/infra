// SPDX-License-Identifier: AGPL-3.0-or-later
//! In-memory geospatial index: add, search, distance, remove.
//!
//! KAYA Geo module, RESP3-compatible GEO commands, 100% Rust, no external
//! geospatial service. Each [`GeoIndex`] belongs to a single collection key
//! and owns both a direct member lookup map and a sorted view by packed
//! geohash score so that radius queries can stream over a tight range.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use bytes::Bytes;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::geo::error::GeoError;
use crate::geo::point::GeoPoint;

/// Distance / length unit supported by GEOSEARCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// Metres.
    M,
    /// Kilometres.
    Km,
    /// Statute miles.
    Mi,
    /// Feet.
    Ft,
}

impl Unit {
    /// Number of metres in one unit.
    pub fn meters_per_unit(self) -> f64 {
        match self {
            Unit::M => 1.0,
            Unit::Km => 1000.0,
            Unit::Mi => 1609.344,
            Unit::Ft => 0.304_8,
        }
    }

    /// Convert a metre value to this unit.
    pub fn from_meters(self, meters: f64) -> f64 {
        meters / self.meters_per_unit()
    }

    /// Convert a value expressed in this unit to metres.
    pub fn to_meters(self, value: f64) -> f64 {
        value * self.meters_per_unit()
    }

    /// Parse `"M" / "KM" / "MI" / "FT"` case-insensitively.
    pub fn parse(s: &str) -> Result<Self, GeoError> {
        match s.to_ascii_uppercase().as_str() {
            "M" => Ok(Unit::M),
            "KM" => Ok(Unit::Km),
            "MI" => Ok(Unit::Mi),
            "FT" => Ok(Unit::Ft),
            other => Err(GeoError::GeohashError(format!("unknown unit: {other}"))),
        }
    }
}

/// Shape of a GEOSEARCH query.
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    /// Circle centred on `center` with `radius_m` metres radius.
    Radius {
        center: GeoPoint,
        radius_m: f64,
    },
    /// Axis-aligned bounding box centred on `center`, with full width and
    /// full height expressed in metres (not half-extents).
    Box {
        center: GeoPoint,
        width_m: f64,
        height_m: f64,
    },
}

impl Shape {
    /// Conservative radius in metres that fully contains the shape. Used as
    /// the first-pass geohash cell coverage estimator.
    pub fn bounding_radius_m(&self) -> f64 {
        match *self {
            Shape::Radius { radius_m, .. } => radius_m,
            Shape::Box { width_m, height_m, .. } => {
                (width_m * width_m + height_m * height_m).sqrt() / 2.0
            }
        }
    }

    /// Centre of the shape.
    pub fn center(&self) -> GeoPoint {
        match *self {
            Shape::Radius { center, .. } => center,
            Shape::Box { center, .. } => center,
        }
    }
}

/// Sort order for GEOSEARCH results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// No explicit order (insertion / natural order).
    Unordered,
    /// Ascending distance from the shape centre.
    Asc,
    /// Descending distance from the shape centre.
    Desc,
}

/// Full GEOSEARCH query.
#[derive(Debug, Clone)]
pub struct GeoSearchQuery {
    pub shape: Shape,
    pub unit: Unit,
    pub count: Option<usize>,
    pub sort: SortOrder,
    pub with_coord: bool,
    pub with_dist: bool,
    pub with_hash: bool,
}

/// Single GEOSEARCH result row.
#[derive(Debug, Clone)]
pub struct GeoSearchResult {
    pub member: Bytes,
    pub distance_m: f64,
    pub point: GeoPoint,
    pub hash: u64,
}

/// In-memory geospatial index scoped to a single collection key.
pub struct GeoIndex {
    /// Direct member -> point lookup.
    members: DashMap<Bytes, GeoPoint>,
    /// Sorted index by packed 52-bit geohash score. The map value is the
    /// list of members sharing the same score (extremely rare but possible).
    sorted_by_hash: RwLock<BTreeMap<u64, Vec<Bytes>>>,
}

impl Default for GeoIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl GeoIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self {
            members: DashMap::new(),
            sorted_by_hash: RwLock::new(BTreeMap::new()),
        }
    }

    /// Number of indexed members.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Returns `true` when the index is empty.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Add a member at the given point. Returns `true` if the member was new.
    /// If the member already existed, its previous point is replaced.
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn add(&self, member: Bytes, point: GeoPoint) -> bool {
        let new_score = point.to_u64_score();
        let mut sorted = self.sorted_by_hash.write();

        let is_new = match self.members.get(&member) {
            Some(existing) => {
                let old_score = existing.to_u64_score();
                if old_score != new_score {
                    if let Some(bucket) = sorted.get_mut(&old_score) {
                        bucket.retain(|m| m != &member);
                        if bucket.is_empty() {
                            sorted.remove(&old_score);
                        }
                    }
                }
                false
            }
            None => true,
        };

        // Insert in the sorted index (idempotent under score equality).
        let bucket = sorted.entry(new_score).or_default();
        if !bucket.iter().any(|m| m == &member) {
            bucket.push(member.clone());
        }
        drop(sorted);

        self.members.insert(member, point);
        is_new
    }

    /// Retrieve the point for a member.
    pub fn pos(&self, member: &[u8]) -> Option<GeoPoint> {
        self.members.get(member).map(|r| *r.value())
    }

    /// Distance in metres between two members, if both exist.
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn dist(&self, m1: &[u8], m2: &[u8]) -> Option<f64> {
        let p1 = self.pos(m1)?;
        let p2 = self.pos(m2)?;
        Some(p1.distance_m(&p2))
    }

    /// Remove a member. Returns `true` if it existed.
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn remove(&self, member: &[u8]) -> bool {
        let removed = self.members.remove(member);
        if let Some((key, point)) = removed {
            let score = point.to_u64_score();
            let mut sorted = self.sorted_by_hash.write();
            if let Some(bucket) = sorted.get_mut(&score) {
                bucket.retain(|m| m != &key);
                if bucket.is_empty() {
                    sorted.remove(&score);
                }
            }
            true
        } else {
            false
        }
    }

    /// Execute a search query and return matching members.
    #[tracing::instrument(level = "debug", skip_all, fields(shape = ?query.shape, count = ?query.count))]
    pub fn search(&self, query: &GeoSearchQuery) -> Vec<GeoSearchResult> {
        let center = query.shape.center();
        let bounding_m = query.shape.bounding_radius_m();

        // Step 1: estimate geohash score range covering the bounding circle.
        // We take the centre score and expand a safety margin in score space.
        // For robustness in a first implementation we scan the whole sorted
        // map whenever the shape is large; for small shapes we bracket.
        let sorted = self.sorted_by_hash.read();
        let iter_members: Vec<(u64, Bytes)> = if bounding_m > 50_000.0 {
            sorted
                .iter()
                .flat_map(|(score, bucket)| {
                    bucket.iter().map(move |m| (*score, m.clone()))
                })
                .collect()
        } else {
            let (lo, hi) = estimate_score_range(&center, bounding_m);
            sorted
                .range(lo..=hi)
                .flat_map(|(score, bucket)| {
                    bucket.iter().map(move |m| (*score, m.clone()))
                })
                .collect()
        };
        drop(sorted);

        // Step 2: exact filter by haversine / box membership.
        let mut results: Vec<GeoSearchResult> = iter_members
            .into_iter()
            .filter_map(|(hash, member)| {
                let point = *self.members.get(&member)?.value();
                let distance_m = point.distance_m(&center);
                let keep = match query.shape {
                    Shape::Radius { radius_m, .. } => distance_m <= radius_m,
                    Shape::Box { width_m, height_m, center } => {
                        within_box(point, center, width_m, height_m)
                    }
                };
                if keep {
                    Some(GeoSearchResult {
                        member,
                        distance_m,
                        point,
                        hash,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Step 3: sort then limit.
        match query.sort {
            SortOrder::Asc => results.sort_by(|a, b| {
                a.distance_m
                    .partial_cmp(&b.distance_m)
                    .unwrap_or(Ordering::Equal)
            }),
            SortOrder::Desc => results.sort_by(|a, b| {
                b.distance_m
                    .partial_cmp(&a.distance_m)
                    .unwrap_or(Ordering::Equal)
            }),
            SortOrder::Unordered => {}
        }

        if let Some(n) = query.count {
            results.truncate(n);
        }
        results
    }

    /// Iterate over every `(member, point)` pair currently indexed.
    pub fn snapshot(&self) -> Vec<(Bytes, GeoPoint)> {
        self.members
            .iter()
            .map(|r| (r.key().clone(), *r.value()))
            .collect()
    }
}

/// Box-membership test: we rely on the local approximation that 1 degree of
/// latitude ~= 111_320 m and 1 degree of longitude ~= 111_320 * cos(lat) m.
fn within_box(point: GeoPoint, center: GeoPoint, width_m: f64, height_m: f64) -> bool {
    let half_w = width_m / 2.0;
    let half_h = height_m / 2.0;

    // North/south distance: use straight haversine along the meridian.
    let meridian = GeoPoint {
        lat: point.lat,
        lon: center.lon,
    };
    let ns_m = meridian.distance_m(&center);
    if ns_m > half_h {
        return false;
    }

    // East/west distance at the point's latitude.
    let parallel = GeoPoint {
        lat: center.lat,
        lon: point.lon,
    };
    let ew_m = parallel.distance_m(&center);
    ew_m <= half_w
}

/// Very rough score range bracketing. We convert `radius_m` into a score
/// delta by approximating bits of precision vs. metres of span. For a 52-bit
/// interleaved geohash the whole world is `2^52`; each bit halves span.
/// We use a conservative heuristic and then let the exact haversine filter
/// eliminate false positives.
fn estimate_score_range(center: &GeoPoint, radius_m: f64) -> (u64, u64) {
    // Earth longitudinal span at equator ~ 40_075_016 m; divide by 2^26
    // => ~0.6 m per longitude bit unit, but the interleave doubles this.
    // Compute the number of bits needed to cover `radius_m` along longitude.
    let world_m = 40_075_016.686_f64;
    let bits_needed = (world_m / radius_m.max(1.0)).log2().ceil() as i32;
    let bits_needed = bits_needed.clamp(0, 26);
    let mask_bits = 26 - bits_needed as u32;
    // Expand by one extra bit on each side to be safe.
    let score = center.to_u64_score();
    let span = 1u64 << (2 * mask_bits.saturating_add(2).min(52));
    let lo = score.saturating_sub(span);
    let hi = score.saturating_add(span);
    (lo, hi)
}
