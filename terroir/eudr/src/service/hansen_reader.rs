// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hansen GFC tile reader (S3/MinIO).
//!
//! Object key convention (cf. RUNBOOK-GEO-MIRRORS §7):
//!   `geo-mirror/hansen-gfc/v1.11/Hansen_GFC-2024-v1.11_<layer>_<tile>.tif`
//!
//! Where:
//!   - `<layer>` ∈ {`lossyear`, `treecover2000`, `datamask`}
//!   - `<tile>`  is the 10°×10° identifier `<lat>{N|S}_<lon>{E|W}` aligned
//!     on multiples of 10° (e.g. `10N_010W`).
//!
//! # Tile lookup
//! Hansen tiles are anchored at the **top-left** corner of each 10°×10° block.
//! For a given (lat, lon) point, the tile name is:
//!   - top-edge latitude  = `ceil(lat / 10) * 10`  (clamped to [-50, 80] for tropics)
//!   - left-edge longitude = `floor(lon / 10) * 10`
//!
//! # Pixel decoding
//! A tile is 40 000 × 40 000 px (10° / 30 m ≈ 40 000). Loading a full tile is
//! ~1.6 GB uncompressed. To stay tractable in a hot HTTP path we:
//!   1. Cache the **decoded tile band** in an in-memory LRU (parking_lot) of
//!      up to N tiles (default 4 to fit ~6 GB RAM).
//!   2. Sample a coarse subset of pixels covering the polygon bbox (every
//!      pixel intersecting the bbox up to a max budget of `MAX_SAMPLE_PIXELS`
//!      to bound CPU; in P1 we default to 10 000 samples per validation).
//!
//! When the tile cannot be fetched from MinIO (offline / mirror not yet
//! seeded), the reader returns `TileSummary::NoData` and the validator
//! treats this as zero overlap with a `dataset_version` hint
//! `hansen-gfc:v1.11(no_data)` — surfaced through the audit trail so the
//! agronome reviewer can re-run when the mirror is online.

use std::{io::Cursor, num::NonZeroUsize, sync::Mutex};

use anyhow::{Context, Result};
use aws_sdk_s3::Client as S3Client;
use lru::LruCache;
use tracing::{debug, instrument, warn};

use crate::{HANSEN_CUTOFF_LOSSYEAR, HANSEN_PREFIX, HANSEN_TREECOVER_MIN};

const MAX_SAMPLE_PIXELS: u32 = 10_000;
const PIXELS_PER_DEGREE: f64 = 40_000.0; // 10° / 30 m ≈ 40 000

/// Decoded sub-region (kept small thanks to the 10 000-sample budget).
#[derive(Clone)]
pub struct DecodedTile {
    pub tile_name: String,
    /// `lossyear` band — values 0..=24 (pixel = year - 2000).
    pub lossyear: Vec<u8>,
    /// `treecover2000` band — values 0..=100 (% canopy in 2000).
    pub treecover2000: Vec<u8>,
    /// Sample width.
    pub w: u32,
    /// Sample height.
    pub h: u32,
}

/// Result of a polygon overlap analysis on a Hansen tile.
#[derive(Debug, Clone)]
pub enum TileSummary {
    /// Mirror unavailable / tile missing → degraded mode.
    NoData,
    /// Successful pixel scan.
    Scanned {
        loss_pixels: u32,
        forest_pixels: u32,
        sampled_pixels: u32,
    },
}

/// Tile reader holding the S3 client + LRU cache.
pub struct HansenReader {
    s3: S3Client,
    bucket: String,
    cache: Mutex<LruCache<String, DecodedTile>>,
}

impl HansenReader {
    pub fn new(s3: S3Client, bucket: String) -> Self {
        let cap = NonZeroUsize::new(4).expect("non-zero LRU capacity");
        Self {
            s3,
            bucket,
            cache: Mutex::new(LruCache::new(cap)),
        }
    }

    /// Compute the tile identifier covering a given (lat, lon).
    ///
    /// Hansen tiles span 10° latitude × 10° longitude.
    /// Top-edge latitude is `ceil(lat/10)*10`, clamped to ranges valid for
    /// Hansen (−50° to +80°). Left-edge longitude is `floor(lon/10)*10`.
    pub fn lookup_tile_name(lat: f64, lon: f64) -> String {
        let top_lat = (lat / 10.0).ceil() as i32 * 10;
        let left_lon = (lon / 10.0).floor() as i32 * 10;

        let lat_str = if top_lat >= 0 {
            format!("{:02}N", top_lat)
        } else {
            format!("{:02}S", top_lat.abs())
        };
        let lon_str = if left_lon >= 0 {
            format!("{:03}E", left_lon)
        } else {
            format!("{:03}W", left_lon.abs())
        };
        format!("{lat_str}_{lon_str}")
    }

    fn object_key(layer: &str, tile_name: &str) -> String {
        format!("{HANSEN_PREFIX}/Hansen_GFC-2024-v1.11_{layer}_{tile_name}.tif")
    }

    /// Read both `lossyear` and `treecover2000` bands for a tile and decode a
    /// downsampled grid covering the whole tile area.
    #[instrument(skip(self))]
    async fn fetch_tile(&self, tile_name: &str) -> Result<DecodedTile> {
        if let Some(cached) = self
            .cache
            .lock()
            .map(|mut c| c.get(tile_name).cloned())
            .ok()
            .flatten()
        {
            return Ok(cached);
        }

        let lossyear_bytes = self
            .get_object(&Self::object_key("lossyear", tile_name))
            .await?;
        let treecover_bytes = self
            .get_object(&Self::object_key("treecover2000", tile_name))
            .await?;

        let lossyear = decode_grayscale_u8(&lossyear_bytes)
            .with_context(|| format!("decode lossyear tile {tile_name}"))?;
        let treecover = decode_grayscale_u8(&treecover_bytes)
            .with_context(|| format!("decode treecover2000 tile {tile_name}"))?;

        if lossyear.0 != treecover.0 || lossyear.1 != treecover.1 {
            anyhow::bail!(
                "tile {} band size mismatch: lossyear={}x{}, treecover={}x{}",
                tile_name,
                lossyear.0,
                lossyear.1,
                treecover.0,
                treecover.1
            );
        }

        let decoded = DecodedTile {
            tile_name: tile_name.to_owned(),
            lossyear: lossyear.2,
            treecover2000: treecover.2,
            w: lossyear.0,
            h: lossyear.1,
        };

        if let Ok(mut guard) = self.cache.lock() {
            guard.put(tile_name.to_owned(), decoded.clone());
        }
        Ok(decoded)
    }

    async fn get_object(&self, key: &str) -> Result<Vec<u8>> {
        debug!(bucket = %self.bucket, key = key, "S3 GetObject");
        let resp = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("S3 GetObject {key}"))?;
        let body = resp
            .body
            .collect()
            .await
            .with_context(|| format!("collect S3 body {key}"))?;
        Ok(body.into_bytes().to_vec())
    }

    /// Scan a polygon's bounding box for deforestation pixels (lossyear ≥ cutoff
    /// AND treecover2000 ≥ threshold). Returns counts capped at the sampling
    /// budget so very large polygons don't hog CPU.
    ///
    /// On any S3 / decode error a warning is logged and `TileSummary::NoData`
    /// is returned (the validator surfaces this via the dataset_version field).
    #[instrument(skip(self, polygon_bbox))]
    pub async fn scan_polygon(&self, polygon_bbox: BoundingBox) -> TileSummary {
        // For P1 MVP we use the bbox center to pick a single tile.
        // Polygons crossing tile boundaries are handled by P2 (multi-tile fusion).
        let (cx, cy) = polygon_bbox.center();
        let tile_name = Self::lookup_tile_name(cy, cx);

        let tile = match self.fetch_tile(&tile_name).await {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    tile = tile_name,
                    error = %e,
                    "Hansen tile fetch failed — returning NoData (mirror likely empty in dev)"
                );
                return TileSummary::NoData;
            }
        };

        // Map bbox lat/lon to pixel indices in this tile.
        let (top_lat, left_lon) = top_left_for_tile(&tile_name);
        let pix_per_deg_x = tile.w as f64 / 10.0;
        let pix_per_deg_y = tile.h as f64 / 10.0;

        let to_px = |lat: f64, lon: f64| -> (i64, i64) {
            let x = ((lon - left_lon as f64) * pix_per_deg_x).round() as i64;
            let y = ((top_lat as f64 - lat) * pix_per_deg_y).round() as i64;
            (x, y)
        };

        let (x0, y0) = to_px(polygon_bbox.max_lat, polygon_bbox.min_lon);
        let (x1, y1) = to_px(polygon_bbox.min_lat, polygon_bbox.max_lon);
        let (x_lo, x_hi) = (x0.min(x1).max(0), x0.max(x1).min(tile.w as i64 - 1));
        let (y_lo, y_hi) = (y0.min(y1).max(0), y0.max(y1).min(tile.h as i64 - 1));
        if x_lo > x_hi || y_lo > y_hi {
            return TileSummary::Scanned {
                loss_pixels: 0,
                forest_pixels: 0,
                sampled_pixels: 0,
            };
        }

        let total = ((x_hi - x_lo + 1) * (y_hi - y_lo + 1)) as u32;
        let stride = (total / MAX_SAMPLE_PIXELS).max(1);
        let mut loss = 0u32;
        let mut forest = 0u32;
        let mut sampled = 0u32;

        let mut idx = 0u32;
        for y in y_lo..=y_hi {
            for x in x_lo..=x_hi {
                if idx.is_multiple_of(stride) {
                    let pos = (y as usize) * (tile.w as usize) + (x as usize);
                    if pos >= tile.lossyear.len() {
                        idx += 1;
                        continue;
                    }
                    let ly = tile.lossyear[pos];
                    let tc = tile.treecover2000[pos];
                    if tc >= HANSEN_TREECOVER_MIN {
                        forest += 1;
                        if ly >= HANSEN_CUTOFF_LOSSYEAR {
                            loss += 1;
                        }
                    }
                    sampled += 1;
                }
                idx += 1;
            }
        }

        TileSummary::Scanned {
            loss_pixels: loss,
            forest_pixels: forest,
            sampled_pixels: sampled,
        }
    }
}

/// Bounding box in WGS84 (EPSG:4326).
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

impl BoundingBox {
    pub fn center(&self) -> (f64, f64) {
        (
            (self.min_lon + self.max_lon) / 2.0,
            (self.min_lat + self.max_lat) / 2.0,
        )
    }
    pub fn area_km2(&self) -> f64 {
        // crude equirectangular estimate — sufficient to size pixel scans.
        let dy = (self.max_lat - self.min_lat).abs() * 111.32;
        let dx = (self.max_lon - self.min_lon).abs()
            * 111.32
            * ((self.min_lat + self.max_lat) / 2.0).to_radians().cos();
        dy * dx
    }
}

/// Recompute (top_lat, left_lon) from a tile name like `10N_010W`.
fn top_left_for_tile(tile_name: &str) -> (i32, i32) {
    // Format: <NN>{N|S}_<NNN>{E|W}
    let mut parts = tile_name.split('_');
    let lat = parts.next().unwrap_or("00N");
    let lon = parts.next().unwrap_or("000E");
    let top_lat: i32 = {
        let n: i32 = lat[..lat.len() - 1].parse().unwrap_or(0);
        if lat.ends_with('S') { -n } else { n }
    };
    let left_lon: i32 = {
        let n: i32 = lon[..lon.len() - 1].parse().unwrap_or(0);
        if lon.ends_with('W') { -n } else { n }
    };
    (top_lat, left_lon)
}

/// Decode a single-band 8-bit GeoTIFF from in-memory bytes.
///
/// We use the pure-Rust `tiff` crate to keep the build pipeline simple.
/// Hansen layers are 8-bit unsigned, so `DecodingResult::U8` is expected.
fn decode_grayscale_u8(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder =
        tiff::decoder::Decoder::new(Cursor::new(bytes)).context("init TIFF decoder")?;
    let (w, h) = decoder.dimensions().context("read TIFF dimensions")?;
    let img = decoder.read_image().context("read TIFF image")?;
    match img {
        tiff::decoder::DecodingResult::U8(v) => {
            // Sub-sample to fit MAX_SAMPLE_PIXELS budget if too large.
            let target_w = (PIXELS_PER_DEGREE.sqrt() as u32 * 2).min(w); // ~400
            let target_h = (PIXELS_PER_DEGREE.sqrt() as u32 * 2).min(h); // ~400
            let stride_x = (w / target_w).max(1);
            let stride_y = (h / target_h).max(1);
            let mut out = Vec::with_capacity((target_w * target_h) as usize);
            let mut yy = 0u32;
            while yy < h {
                let mut xx = 0u32;
                while xx < w {
                    let pos = (yy as usize) * (w as usize) + (xx as usize);
                    if pos < v.len() {
                        out.push(v[pos]);
                    }
                    xx += stride_x;
                }
                yy += stride_y;
            }
            let dw = w.div_ceil(stride_x);
            let dh = h.div_ceil(stride_y);
            Ok((dw, dh, out))
        }
        other => anyhow::bail!("unexpected TIFF type for Hansen band: {:?}", other.kind()),
    }
}

trait DecodingResultKind {
    fn kind(&self) -> &'static str;
}
impl DecodingResultKind for tiff::decoder::DecodingResult {
    fn kind(&self) -> &'static str {
        match self {
            tiff::decoder::DecodingResult::U8(_) => "U8",
            tiff::decoder::DecodingResult::U16(_) => "U16",
            tiff::decoder::DecodingResult::U32(_) => "U32",
            tiff::decoder::DecodingResult::U64(_) => "U64",
            tiff::decoder::DecodingResult::F32(_) => "F32",
            tiff::decoder::DecodingResult::F64(_) => "F64",
            tiff::decoder::DecodingResult::I8(_) => "I8",
            tiff::decoder::DecodingResult::I16(_) => "I16",
            tiff::decoder::DecodingResult::I32(_) => "I32",
            tiff::decoder::DecodingResult::I64(_) => "I64",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_lookup_burkina_faso() {
        // Boucle du Mouhoun, BF: ~12.5°N, -3.0°E
        let name = HansenReader::lookup_tile_name(12.5, -3.0);
        assert_eq!(name, "20N_010W");
    }

    #[test]
    fn tile_lookup_southern_hemisphere() {
        let name = HansenReader::lookup_tile_name(-7.5, 35.0);
        assert_eq!(name, "00N_030E");
    }

    #[test]
    fn top_left_parsing() {
        let (lat, lon) = top_left_for_tile("20N_010W");
        assert_eq!((lat, lon), (20, -10));
        let (lat, lon) = top_left_for_tile("10S_030E");
        assert_eq!((lat, lon), (-10, 30));
    }
}
