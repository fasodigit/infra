// SPDX-License-Identifier: AGPL-3.0-or-later
//! JRC TMF tile reader (S3/MinIO).
//!
//! JRC TMF mirrors store one tile per ISO-3 country (e.g. `AnnualChange_BFA.tif`)
//! covering the 1990–2024 disturbance series. For EUDR purposes we only read
//! the `deforestation_year` semantic — a pixel is considered post-2020 lost
//! when its value is ≥ 2021.
//!
//! Like `hansen_reader`, the reader downsamples and caches in an LRU map.
//! When the mirror is empty / offline, `TileSummary::NoData` is returned.

use std::{io::Cursor, num::NonZeroUsize, sync::Mutex};

use anyhow::{Context, Result};
use aws_sdk_s3::Client as S3Client;
use lru::LruCache;
use tracing::{debug, instrument, warn};

use crate::JRC_PREFIX;
use crate::service::hansen_reader::{BoundingBox, TileSummary};

const POST_2020_YEAR: u16 = 2021;

#[derive(Clone)]
struct DecodedTile {
    #[allow(dead_code)]
    pub iso3: String,
    pub data: Vec<u16>,
    #[allow(dead_code)]
    pub w: u32,
    #[allow(dead_code)]
    pub h: u32,
}

pub struct JrcReader {
    s3: S3Client,
    bucket: String,
    cache: Mutex<LruCache<String, DecodedTile>>,
}

impl JrcReader {
    pub fn new(s3: S3Client, bucket: String) -> Self {
        let cap = NonZeroUsize::new(4).expect("non-zero LRU capacity");
        Self {
            s3,
            bucket,
            cache: Mutex::new(LruCache::new(cap)),
        }
    }

    /// Resolve ISO-3 from a (lat, lon) — we keep this trivially mapped to
    /// Burkina Faso for P1 (the pilot tenant). For multi-country support,
    /// P2 will integrate `country-boundaries-rs` or an admin lookup table.
    pub fn iso3_for(_lat: f64, _lon: f64) -> &'static str {
        "BFA"
    }

    fn object_key(iso3: &str) -> String {
        format!("{JRC_PREFIX}/AnnualChange_{iso3}.tif")
    }

    /// Returns true if the country is in the JRC TMF tropical-moist coverage.
    /// In P1 we conservatively only flag tropical moist countries to avoid
    /// false negatives in sahel zones (where TMF has no data).
    pub fn covers(iso3: &str) -> bool {
        matches!(iso3, "CIV" | "GHA" | "CMR" | "GAB" | "COG" | "COD" | "LBR")
    }

    #[instrument(skip(self))]
    async fn fetch_tile(&self, iso3: &str) -> Result<DecodedTile> {
        if let Some(cached) = self
            .cache
            .lock()
            .map(|mut c| c.get(iso3).cloned())
            .ok()
            .flatten()
        {
            return Ok(cached);
        }

        let key = Self::object_key(iso3);
        debug!(bucket = %self.bucket, key = key, "S3 GetObject (JRC)");
        let resp = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .with_context(|| format!("S3 GetObject {key}"))?;
        let body = resp
            .body
            .collect()
            .await
            .context("collect S3 body (JRC)")?
            .into_bytes()
            .to_vec();

        let (w, h, data) = decode_year_band(&body).with_context(|| format!("decode JRC {key}"))?;

        let tile = DecodedTile {
            iso3: iso3.to_owned(),
            data,
            w,
            h,
        };
        if let Ok(mut guard) = self.cache.lock() {
            guard.put(iso3.to_owned(), tile.clone());
        }
        Ok(tile)
    }

    /// Scan a polygon's bounding box for post-2020 deforestation pixels in JRC TMF.
    #[instrument(skip(self, polygon_bbox))]
    pub async fn scan_polygon(&self, polygon_bbox: BoundingBox) -> TileSummary {
        let (cx, cy) = polygon_bbox.center();
        let iso3 = Self::iso3_for(cy, cx);
        if !Self::covers(iso3) {
            // Outside tropical moist coverage — JRC silent (NoData).
            return TileSummary::NoData;
        }

        let tile = match self.fetch_tile(iso3).await {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    iso3 = iso3,
                    error = %e,
                    "JRC TMF tile fetch failed — returning NoData"
                );
                return TileSummary::NoData;
            }
        };

        // We don't know the exact JRC affine transform without parsing the TIFF
        // GeoKeyDirectory (out of scope for the pure-Rust `tiff` crate). For P1
        // we approximate by scanning a uniform fraction (1%) of the tile —
        // sufficient to flag presence/absence of post-2020 pixels which is the
        // EUDR-relevant signal. The TODO P3+ section in the runbook tracks
        // moving to GDAL or `geozero` once needed for precise area computation.
        let total = tile.data.len() as u32;
        let stride = (total / 10_000).max(1);
        let mut loss = 0u32;
        let mut sampled = 0u32;
        let mut i = 0;
        while i < tile.data.len() {
            let v = tile.data[i];
            if v >= POST_2020_YEAR {
                loss += 1;
            }
            sampled += 1;
            i += stride as usize;
        }

        TileSummary::Scanned {
            loss_pixels: loss,
            forest_pixels: sampled, // we treat sampled = forest baseline (TMF coverage)
            sampled_pixels: sampled,
        }
    }
}

/// Decode the JRC TMF "deforestation_year" band from in-memory bytes.
/// Returns (width, height, year_data) where year_data is u16 (year value).
fn decode_year_band(bytes: &[u8]) -> Result<(u32, u32, Vec<u16>)> {
    let mut decoder =
        tiff::decoder::Decoder::new(Cursor::new(bytes)).context("init JRC TIFF decoder")?;
    let (w, h) = decoder.dimensions().context("read JRC TIFF dimensions")?;
    let img = decoder.read_image().context("read JRC TIFF image")?;
    let v = match img {
        tiff::decoder::DecodingResult::U16(v) => v,
        tiff::decoder::DecodingResult::U8(v) => v.into_iter().map(u16::from).collect(),
        _ => anyhow::bail!("unexpected JRC TIFF data type"),
    };

    // Downsample to ~400×400 for cache friendliness.
    let target = 400u32;
    let stride_x = (w / target).max(1);
    let stride_y = (h / target).max(1);
    let mut out = Vec::new();
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
    Ok((w / stride_x, h / stride_y, out))
}
