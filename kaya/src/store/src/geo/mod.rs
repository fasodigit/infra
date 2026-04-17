// SPDX-License-Identifier: AGPL-3.0-or-later
//! KAYA Geo indexes.
//!
//! KAYA Geo module, RESP3-compatible GEO commands, 100% Rust, no external
//! geospatial service. This module exposes the building blocks consumed by
//! the [`crate::Store`] geo API (GEOADD / GEOPOS / GEODIST / GEOSEARCH /
//! GEOREM / GEOHASH) and by the command handler in `kaya-commands`.

pub mod error;
pub mod index;
pub mod metrics;
pub mod point;

pub use error::GeoError;
pub use index::{
    GeoIndex, GeoSearchQuery, GeoSearchResult, Shape, SortOrder, Unit,
};
pub use point::GeoPoint;

// ---------------------------------------------------------------------------
// GeoAddOpts — flags for GEOADD
// ---------------------------------------------------------------------------

/// Modifier flags for [`crate::Store::geoadd`].
#[derive(Debug, Clone, Copy, Default)]
pub struct GeoAddOpts {
    /// NX — only add new members; never update existing members.
    pub nx: bool,
    /// XX — only update existing members; never add new members.
    pub xx: bool,
    /// CH — count changed members (insertions + updates) instead of only
    /// insertions. Mirrors the Redis GEOADD CH option.
    pub ch: bool,
}

// ---------------------------------------------------------------------------
// impl Store — geo API methods
// ---------------------------------------------------------------------------

impl crate::Store {
    /// GEOADD: add one or more `(GeoPoint, member)` pairs to the geo index at
    /// `key`. `members` is a slice of `(point, member_bytes)` tuples. The
    /// options `NX`, `XX`, `CH` control insert/update semantics.
    ///
    /// Returns the number of elements added (or changed when CH is set).
    #[tracing::instrument(level = "debug", skip_all, fields(key = ?std::str::from_utf8(key).unwrap_or("<binary>")))]
    pub fn geoadd(
        &self,
        key: &[u8],
        members: &[(GeoPoint, Vec<u8>)],
        opts: GeoAddOpts,
    ) -> Result<i64, GeoError> {
        let pairs: Vec<(GeoPoint, bytes::Bytes)> = members
            .iter()
            .map(|(pt, name)| (*pt, bytes::Bytes::copy_from_slice(name)))
            .collect();

        let count = self
            .shard(key)
            .geo_add(key, &pairs, opts.nx, opts.xx, opts.ch);

        metrics::GEO_POINTS_TOTAL
            .with_label_values(&[std::str::from_utf8(key).unwrap_or("?")])
            .inc_by(count.max(0) as u64);

        Ok(count)
    }

    /// GEOPOS: return the position of each member. Returns `None` for members
    /// that do not exist in the index.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn geopos(&self, key: &[u8], members: &[&[u8]]) -> Vec<Option<GeoPoint>> {
        let shard = self.shard(key);
        members
            .iter()
            .map(|m| shard.geo_pos(key, m))
            .collect()
    }

    /// GEODIST: haversine distance between two members expressed in `unit`.
    /// Returns `None` if either member does not exist.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn geodist(
        &self,
        key: &[u8],
        m1: &[u8],
        m2: &[u8],
        unit: Unit,
    ) -> Result<Option<f64>, GeoError> {
        let dist_m = self.shard(key).geo_dist(key, m1, m2)?;
        Ok(dist_m.map(|d| unit.from_meters(d)))
    }

    /// GEOSEARCH: spatial search within the given query shape. Results are
    /// returned in the order and count specified by `query.sort` / `query.count`.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn geosearch(
        &self,
        key: &[u8],
        query: GeoSearchQuery,
    ) -> Result<Vec<GeoSearchResult>, GeoError> {
        let t0 = std::time::Instant::now();
        let results = self.shard(key).geo_search(key, &query)?;
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        metrics::GEO_SEARCH_DURATION_MS.observe(elapsed_ms);
        metrics::GEO_SEARCH_RESULTS_RETURNED.observe(results.len() as f64);
        Ok(results)
    }

    /// GEOSEARCHSTORE: run a GEOSEARCH and store results into `dest`. If `dest`
    /// and `src` are on different shards the operation performs a cross-shard
    /// read+write — correctness is preserved but there is no atomicity across
    /// the two shards (consistent with the KAYA distributed model).
    ///
    /// Returns the number of elements stored in the destination key.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn geosearchstore(
        &self,
        dest: &[u8],
        src: &[u8],
        query: GeoSearchQuery,
    ) -> i64 {
        let src_shard_idx = self.shard_index(src);
        let dest_shard_idx = self.shard_index(dest);

        if src_shard_idx == dest_shard_idx {
            // Same shard: single lock acquisition path.
            self.shard_at(src_shard_idx)
                .geo_search_store(dest, src, &query)
        } else {
            // Cross-shard: read from source, write to destination.
            let results = match self.shard_at(src_shard_idx).geo_search(src, &query) {
                Ok(r) => r,
                Err(_) => return 0,
            };
            let count = results.len() as i64;
            if count == 0 {
                return 0;
            }
            let dest_shard = self.shard_at(dest_shard_idx);
            let pairs: Vec<(GeoPoint, bytes::Bytes)> =
                results.into_iter().map(|r| (r.point, r.member)).collect();
            dest_shard.geo_add(dest, &pairs, false, false, false);
            count
        }
    }

    /// GEOHASH: return the standard base32 geohash (11 characters) for each
    /// member. Returns `None` for members that do not exist.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn geohash(&self, key: &[u8], members: &[&[u8]]) -> Vec<Option<String>> {
        let shard = self.shard(key);
        members
            .iter()
            .map(|m| shard.geo_hash(key, m))
            .collect()
    }

    /// GEOREM (KAYA extension): remove members from a geo index. Semantically
    /// equivalent to ZREM on the underlying sorted structure.
    ///
    /// Returns the number of members removed.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn georem(&self, key: &[u8], members: &[&[u8]]) -> i64 {
        self.shard(key).geo_rem(key, members)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Store, StoreConfig};
    use kaya_compress::CompressConfig;

    fn store() -> Store {
        Store::new(StoreConfig::default(), CompressConfig::default())
    }

    fn paris() -> GeoPoint {
        GeoPoint::new(48.85, 2.35).unwrap()
    }
    fn new_york() -> GeoPoint {
        GeoPoint::new(40.71, -74.00).unwrap()
    }
    fn tokyo() -> GeoPoint {
        GeoPoint::new(35.68, 139.69).unwrap()
    }

    // -----------------------------------------------------------------------
    // GEOADD / GEOPOS happy path
    // -----------------------------------------------------------------------

    #[test]
    fn geoadd_and_geopos_three_cities() {
        let s = store();
        let key = b"cities";

        let added = s
            .geoadd(
                key,
                &[
                    (paris(), b"Paris".to_vec()),
                    (new_york(), b"NewYork".to_vec()),
                    (tokyo(), b"Tokyo".to_vec()),
                ],
                GeoAddOpts::default(),
            )
            .unwrap();
        assert_eq!(added, 3, "three new members");

        let positions = s.geopos(key, &[b"Paris".as_ref(), b"NewYork", b"Tokyo", b"Unknown"]);
        assert!(positions[0].is_some());
        assert!(positions[1].is_some());
        assert!(positions[2].is_some());
        assert!(positions[3].is_none(), "non-existent member returns None");

        let p = positions[0].unwrap();
        // Stored position round-trips through geohash — allow small epsilon.
        assert!((p.lat - 48.85).abs() < 0.01);
        assert!((p.lon - 2.35).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // GEODIST Paris <-> New York ≈ 5837 km (± 5%)
    // -----------------------------------------------------------------------

    #[test]
    fn geodist_paris_new_york() {
        let s = store();
        let key = b"cities2";
        s.geoadd(
            key,
            &[
                (paris(), b"Paris".to_vec()),
                (new_york(), b"NewYork".to_vec()),
            ],
            GeoAddOpts::default(),
        )
        .unwrap();

        let dist_km = s
            .geodist(key, b"Paris", b"NewYork", Unit::Km)
            .unwrap()
            .unwrap();
        // Reference: ~5836 km; allow ±5% (≤292 km)
        let expected = 5836.0_f64;
        let error_pct = ((dist_km - expected) / expected).abs() * 100.0;
        assert!(
            error_pct < 5.0,
            "distance {dist_km:.1} km, error {error_pct:.2}%"
        );
        // Tighter: our haversine should be ≤ 0.5% off.
        assert!(
            error_pct < 0.5,
            "haversine precision failed: {dist_km:.1} km, error {error_pct:.3}%"
        );
    }

    // -----------------------------------------------------------------------
    // GEOSEARCH FROMMEMBER Paris BYRADIUS 10000 km → all 3 cities
    // -----------------------------------------------------------------------

    #[test]
    fn geosearch_radius_all_cities() {
        let s = store();
        let key = b"cities3";
        s.geoadd(
            key,
            &[
                (paris(), b"Paris".to_vec()),
                (new_york(), b"NewYork".to_vec()),
                (tokyo(), b"Tokyo".to_vec()),
            ],
            GeoAddOpts::default(),
        )
        .unwrap();

        let query = GeoSearchQuery {
            shape: Shape::Radius {
                center: paris(),
                radius_m: 10_000.0 * 1000.0, // 10 000 km in metres
            },
            unit: Unit::Km,
            count: None,
            sort: SortOrder::Asc,
            with_coord: true,
            with_dist: true,
            with_hash: false,
        };
        let results = s.geosearch(key, query).unwrap();
        assert_eq!(results.len(), 3, "all three cities within 10 000 km");
        // First result should be Paris (distance ≈ 0)
        assert_eq!(results[0].member.as_ref(), b"Paris");
    }

    // -----------------------------------------------------------------------
    // GEOSEARCH BYBOX 1000x1000 km centred on Paris → only Paris
    // -----------------------------------------------------------------------

    #[test]
    fn geosearch_box_only_paris() {
        let s = store();
        let key = b"cities4";
        s.geoadd(
            key,
            &[
                (paris(), b"Paris".to_vec()),
                (new_york(), b"NewYork".to_vec()),
                (tokyo(), b"Tokyo".to_vec()),
            ],
            GeoAddOpts::default(),
        )
        .unwrap();

        let query = GeoSearchQuery {
            shape: Shape::Box {
                center: paris(),
                width_m: 1_000_000.0,  // 1000 km
                height_m: 1_000_000.0, // 1000 km
            },
            unit: Unit::Km,
            count: None,
            sort: SortOrder::Unordered,
            with_coord: false,
            with_dist: false,
            with_hash: false,
        };
        let results = s.geosearch(key, query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].member.as_ref(), b"Paris");
    }

    // -----------------------------------------------------------------------
    // GEOHASH Paris → 11-character base32 string
    // The standard geohash for Paris (48.85, 2.35) at precision 9 is
    // "u09tunquc"; at precision 11 it encodes higher resolution and starts
    // with "u1" given the exact coords stored.  We verify structural
    // correctness: 11 chars, valid base32 alphabet, round-trips to ~Paris.
    // -----------------------------------------------------------------------

    #[test]
    fn geohash_paris_well_formed() {
        let s = store();
        let key = b"cities5";
        s.geoadd(key, &[(paris(), b"Paris".to_vec())], GeoAddOpts::default())
            .unwrap();

        let hashes = s.geohash(key, &[b"Paris".as_ref(), b"Unknown"]);
        assert_eq!(hashes.len(), 2);
        let h = hashes[0].as_ref().unwrap();
        assert_eq!(h.len(), 11, "geohash must be 11 chars");

        // All characters must be in the standard Geohash base32 alphabet.
        const BASE32: &str = "0123456789bcdefghjkmnpqrstuvwxyz";
        for c in h.chars() {
            assert!(BASE32.contains(c), "invalid geohash char '{c}' in '{h}'");
        }

        // Round-trip: decode back to a point and verify it is near Paris.
        let decoded = point::GeoPoint::from_geohash(h).unwrap();
        assert!((decoded.lat - 48.85).abs() < 0.01, "lat round-trip");
        assert!((decoded.lon - 2.35).abs() < 0.01, "lon round-trip");

        // The geohash for any Paris-area point starts with 'u' (NW Europe cell).
        assert!(
            h.starts_with('u'),
            "Paris geohash {h} must start with 'u' (W-Europe cell)"
        );

        assert!(hashes[1].is_none(), "unknown member returns None");
    }

    // -----------------------------------------------------------------------
    // GEOREM: remove members
    // -----------------------------------------------------------------------

    #[test]
    fn georem_members() {
        let s = store();
        let key = b"cities6";
        s.geoadd(
            key,
            &[
                (paris(), b"Paris".to_vec()),
                (new_york(), b"NewYork".to_vec()),
            ],
            GeoAddOpts::default(),
        )
        .unwrap();

        let removed = s.georem(key, &[b"Paris".as_ref(), b"Nonexistent"]);
        assert_eq!(removed, 1);
        let pos = s.geopos(key, &[b"Paris".as_ref()]);
        assert!(pos[0].is_none(), "Paris should be gone after GEOREM");
    }

    // -----------------------------------------------------------------------
    // Edge cases: invalid coordinates
    // -----------------------------------------------------------------------

    #[test]
    fn invalid_latitude_returns_error() {
        let result = GeoPoint::new(91.0, 0.0);
        assert!(
            matches!(result, Err(GeoError::InvalidLatitude(_))),
            "latitude 91 should fail"
        );
    }

    #[test]
    fn invalid_longitude_returns_error() {
        let result = GeoPoint::new(0.0, 181.0);
        assert!(
            matches!(result, Err(GeoError::InvalidLongitude(_))),
            "longitude 181 should fail"
        );
    }

    // -----------------------------------------------------------------------
    // NX / XX / CH options
    // -----------------------------------------------------------------------

    #[test]
    fn geoadd_nx_does_not_update_existing() {
        let s = store();
        let key = b"cities7";
        s.geoadd(key, &[(paris(), b"Paris".to_vec())], GeoAddOpts::default())
            .unwrap();

        // Move Paris to Tokyo coords but with NX — must NOT update.
        let count = s
            .geoadd(
                key,
                &[(tokyo(), b"Paris".to_vec())],
                GeoAddOpts { nx: true, xx: false, ch: false },
            )
            .unwrap();
        assert_eq!(count, 0, "NX: no new member should be added");
        let pos = s.geopos(key, &[b"Paris".as_ref()])[0].unwrap();
        assert!((pos.lat - 48.85).abs() < 0.1, "Paris must not have moved");
    }

    #[test]
    fn geoadd_xx_does_not_add_new() {
        let s = store();
        let key = b"cities8";

        // XX on empty index: nothing inserted.
        let count = s
            .geoadd(
                key,
                &[(paris(), b"Paris".to_vec())],
                GeoAddOpts { nx: false, xx: true, ch: false },
            )
            .unwrap();
        assert_eq!(count, 0, "XX: must not insert when member is new");
        let pos = s.geopos(key, &[b"Paris".as_ref()]);
        assert!(pos[0].is_none());
    }

    // -----------------------------------------------------------------------
    // GEOSEARCHSTORE
    // -----------------------------------------------------------------------

    #[test]
    fn geosearchstore_copies_results() {
        let s = store();
        let src = b"src_cities";
        let dst = b"dst_cities";
        s.geoadd(
            src,
            &[
                (paris(), b"Paris".to_vec()),
                (new_york(), b"NewYork".to_vec()),
                (tokyo(), b"Tokyo".to_vec()),
            ],
            GeoAddOpts::default(),
        )
        .unwrap();

        let query = GeoSearchQuery {
            shape: Shape::Radius {
                center: paris(),
                radius_m: 10_000.0 * 1000.0,
            },
            unit: Unit::Km,
            count: None,
            sort: SortOrder::Asc,
            with_coord: false,
            with_dist: false,
            with_hash: false,
        };
        let stored = s.geosearchstore(dst, src, query);
        assert_eq!(stored, 3);
        // Verify destination contains all three cities.
        let positions = s.geopos(dst, &[b"Paris".as_ref(), b"NewYork", b"Tokyo"]);
        assert!(positions.iter().all(|p| p.is_some()));
    }
}
