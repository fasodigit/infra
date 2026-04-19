// SPDX-License-Identifier: AGPL-3.0-or-later
//! Response compression: Brotli, Zstd, Gzip.
//!
//! This module is the single integration point for outbound response compression
//! in the FORGE proxy pipeline. It:
//! - Parses the `Accept-Encoding` header and selects the best supported encoding
//!   according to quality values (q-factors), preferring `br > zstd > gzip`.
//! - Skips compression when the body is smaller than the configured threshold
//!   (default 1 KB) or when the `Content-Type` indicates a binary/already-compressed
//!   media type.
//! - Returns the compressed body as a [`bytes::Bytes`] together with the chosen
//!   encoding token so callers can set `Content-Encoding`, recompute
//!   `Content-Length`, and add `Vary: Accept-Encoding`.

use bytes::Bytes;
use std::io::Write as _;
use thiserror::Error;

// -- constants --

/// Minimum body size (bytes) below which compression is skipped.
pub const DEFAULT_MIN_COMPRESS_BYTES: usize = 1024;

// -- error type --

/// Errors that can occur during body compression.
#[derive(Debug, Error)]
pub enum CompressError {
    #[error("brotli compression failed: {0}")]
    Brotli(String),

    #[error("zstd compression failed: {0}")]
    Zstd(#[from] std::io::Error),

    #[error("gzip compression failed")]
    Gzip,
}

// -- encoding enum --

/// Chosen content encoding with its quality / level parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Brotli with quality level (0–11).
    Brotli(u32),
    /// Zstd with compression level (1–22, or negative for speed presets).
    Zstd(i32),
    /// Gzip with compression level (0–9).
    Gzip(u32),
    /// No compression (pass-through).
    Identity,
}

impl Encoding {
    /// Return the `Content-Encoding` token for this encoding.
    pub fn as_header_value(&self) -> &'static str {
        match self {
            Self::Brotli(_) => "br",
            Self::Zstd(_) => "zstd",
            Self::Gzip(_) => "gzip",
            Self::Identity => "identity",
        }
    }

    /// Returns `true` if the encoding is [`Encoding::Identity`].
    pub fn is_identity(&self) -> bool {
        matches!(self, Self::Identity)
    }
}

// -- Accept-Encoding parser --

/// Parse the `Accept-Encoding` request header and return the best encoding the
/// FORGE proxy supports, or [`Encoding::Identity`] when nothing matches.
///
/// Priority order when q-values are equal: `br > zstd > gzip > identity`.
///
/// # Example
/// ```
/// use armageddon_forge::compression::{parse_accept_encoding, Encoding};
/// let enc = parse_accept_encoding("gzip, br;q=1.0, zstd;q=0.9");
/// assert!(matches!(enc, Encoding::Brotli(_)));
/// ```
pub fn parse_accept_encoding(header: &str) -> Encoding {
    // Weights assigned to each supported algorithm when their q-values tie.
    // Higher weight = preferred.
    const BROTLI_TIE: i32 = 3;
    const ZSTD_TIE: i32 = 2;
    const GZIP_TIE: i32 = 1;

    let mut best_enc = Encoding::Identity;
    let mut best_q: f32 = -1.0;
    let mut best_tie: i32 = 0;

    for token in header.split(',') {
        let token = token.trim();
        let mut parts = token.splitn(2, ';');
        let name = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        let q: f32 = parts
            .next()
            .and_then(|p| {
                let param = p.trim();
                if param.starts_with("q=") || param.starts_with("Q=") {
                    param[2..].parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(1.0_f32);

        if q < 0.0 || q > 1.0 {
            continue;
        }

        let (enc, tie) = match name.as_str() {
            "br" => (Encoding::Brotli(4), BROTLI_TIE),
            "zstd" => (Encoding::Zstd(3), ZSTD_TIE),
            "gzip" | "x-gzip" => (Encoding::Gzip(6), GZIP_TIE),
            "identity" | "*" => (Encoding::Identity, 0),
            _ => continue,
        };

        // q=0 means "not acceptable"; skip entirely.
        if q == 0.0 {
            continue;
        }

        if q > best_q || (q == best_q && tie > best_tie) {
            best_q = q;
            best_tie = tie;
            best_enc = enc;
        }
    }

    best_enc
}

// -- Content-Type guard --

/// Returns `true` when the given `Content-Type` value should NOT be compressed
/// (already-compressed formats, binary media, video, audio, images).
///
/// Compression of these types wastes CPU and adds negligible (or negative) gain.
pub fn should_skip_content_type(content_type: &str) -> bool {
    let ct = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase();

    // Already-compressed or binary formats — skip.
    matches!(
        ct.as_str(),
        "image/jpeg"
            | "image/png"
            | "image/gif"
            | "image/webp"
            | "image/avif"
            | "image/bmp"
            | "image/tiff"
            | "audio/mpeg"
            | "audio/ogg"
            | "audio/opus"
            | "audio/aac"
            | "audio/flac"
            | "video/mp4"
            | "video/webm"
            | "video/ogg"
            | "video/mpeg"
            | "application/zip"
            | "application/x-gzip"
            | "application/gzip"
            | "application/zstd"
            | "application/x-bzip2"
            | "application/x-7z-compressed"
            | "application/x-rar-compressed"
            | "application/x-tar"
            | "application/pdf"
            | "application/octet-stream"
            | "application/wasm"
    )
}

// -- core compression function --

/// Compress `body` using the requested [`Encoding`].
///
/// Returns [`Ok(Bytes::copy_from_slice(body))`] when the encoding is
/// [`Encoding::Identity`] or the body is smaller than `min_size`.
///
/// # Errors
/// Returns a [`CompressError`] if the underlying compression library fails.
pub fn compress_body(body: &[u8], enc: Encoding, min_size: usize) -> Result<Bytes, CompressError> {
    if enc.is_identity() || body.len() < min_size {
        return Ok(Bytes::copy_from_slice(body));
    }

    match enc {
        Encoding::Brotli(quality) => compress_brotli(body, quality),
        Encoding::Zstd(level) => compress_zstd(body, level),
        Encoding::Gzip(level) => compress_gzip(body, level),
        Encoding::Identity => Ok(Bytes::copy_from_slice(body)),
    }
}

// -- private helpers --

fn compress_brotli(input: &[u8], quality: u32) -> Result<Bytes, CompressError> {
    let quality = quality.min(11);
    let lgwin = 22_u32; // window size log2; 22 = 4 MB — good default
    let mut output = Vec::with_capacity(input.len() / 2);
    {
        let mut encoder = brotli::CompressorWriter::new(&mut output, 4096, quality, lgwin);
        encoder
            .write_all(input)
            .map_err(|e| CompressError::Brotli(e.to_string()))?;
        // flush + finalize
        encoder
            .flush()
            .map_err(|e| CompressError::Brotli(e.to_string()))?;
    } // encoder drops here, writing the final Brotli stream terminator
    Ok(Bytes::from(output))
}

fn compress_zstd(input: &[u8], level: i32) -> Result<Bytes, CompressError> {
    let compressed = zstd::encode_all(input, level)?;
    Ok(Bytes::from(compressed))
}

fn compress_gzip(input: &[u8], level: u32) -> Result<Bytes, CompressError> {
    use flate2::{write::GzEncoder, Compression};
    let compression = Compression::new(level.min(9));
    let mut encoder = GzEncoder::new(Vec::with_capacity(input.len() / 2), compression);
    encoder.write_all(input).map_err(|_| CompressError::Gzip)?;
    let output = encoder.finish().map_err(|_| CompressError::Gzip)?;
    Ok(Bytes::from(output))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_accept_encoding --

    #[test]
    fn test_parse_prefers_brotli_over_gzip() {
        let enc = parse_accept_encoding("gzip, br");
        assert!(matches!(enc, Encoding::Brotli(_)), "expected Brotli, got {enc:?}");
    }

    #[test]
    fn test_parse_prefers_zstd_over_gzip() {
        let enc = parse_accept_encoding("gzip, zstd");
        assert!(matches!(enc, Encoding::Zstd(_)), "expected Zstd, got {enc:?}");
    }

    #[test]
    fn test_parse_q_value_wins() {
        // zstd has higher q than br — must win.
        let enc = parse_accept_encoding("br;q=0.5, zstd;q=0.9, gzip;q=0.8");
        assert!(matches!(enc, Encoding::Zstd(_)), "expected Zstd, got {enc:?}");
    }

    #[test]
    fn test_parse_q_zero_rejected() {
        // br explicitly refused; zstd and gzip remain.
        let enc = parse_accept_encoding("br;q=0, zstd;q=0.8, gzip;q=0.7");
        assert!(matches!(enc, Encoding::Zstd(_)), "expected Zstd, got {enc:?}");
    }

    #[test]
    fn test_parse_identity_when_empty() {
        let enc = parse_accept_encoding("");
        assert_eq!(enc, Encoding::Identity);
    }

    #[test]
    fn test_parse_unknown_tokens_ignored() {
        let enc = parse_accept_encoding("deflate;q=0.9, gzip;q=0.8");
        assert!(matches!(enc, Encoding::Gzip(_)), "expected Gzip, got {enc:?}");
    }

    // -- should_skip_content_type --

    #[test]
    fn test_skip_image_jpeg() {
        assert!(should_skip_content_type("image/jpeg"));
    }

    #[test]
    fn test_skip_video_mp4() {
        assert!(should_skip_content_type("video/mp4; codecs=avc1"));
    }

    #[test]
    fn test_no_skip_json() {
        assert!(!should_skip_content_type("application/json"));
    }

    #[test]
    fn test_no_skip_html() {
        assert!(!should_skip_content_type("text/html; charset=utf-8"));
    }

    #[test]
    fn test_skip_zip() {
        assert!(should_skip_content_type("application/zip"));
    }

    // -- compress_body threshold --

    #[test]
    fn test_no_compress_below_threshold() {
        let body = b"hello";
        let result = compress_body(body, Encoding::Brotli(4), DEFAULT_MIN_COMPRESS_BYTES).unwrap();
        assert_eq!(result.as_ref(), body, "body below threshold must be returned as-is");
    }

    #[test]
    fn test_identity_passthrough() {
        let body = b"some data that would normally be compressed";
        let result = compress_body(body, Encoding::Identity, 0).unwrap();
        assert_eq!(result.as_ref(), body);
    }

    // -- Brotli round-trip --

    #[test]
    fn test_brotli_roundtrip() {
        let original = generate_json_10kb();
        let compressed = compress_body(&original, Encoding::Brotli(4), 0).unwrap();

        // Decompress and verify fidelity.
        let mut decompressed = Vec::new();
        let mut decoder = brotli::Decompressor::new(compressed.as_ref(), 4096);
        std::io::copy(&mut decoder, &mut decompressed).expect("brotli decompression failed");
        assert_eq!(decompressed, original, "Brotli round-trip mismatch");
    }

    #[test]
    fn test_brotli_compression_ratio_10kb_json() {
        let original = generate_json_10kb();
        let compressed = compress_body(&original, Encoding::Brotli(4), 0).unwrap();
        let ratio = compressed.len() as f64 / original.len() as f64;
        assert!(
            ratio <= 0.50,
            "Brotli ratio {:.2} exceeds 50% on 10 KB JSON (compressed={} original={})",
            ratio,
            compressed.len(),
            original.len()
        );
    }

    // -- Zstd round-trip --

    #[test]
    fn test_zstd_roundtrip() {
        let original = generate_json_10kb();
        let compressed = compress_body(&original, Encoding::Zstd(3), 0).unwrap();
        let decompressed = zstd::decode_all(compressed.as_ref()).expect("zstd decompression failed");
        assert_eq!(decompressed, original, "Zstd round-trip mismatch");
    }

    #[test]
    fn test_zstd_compression_ratio_10kb_json() {
        let original = generate_json_10kb();
        let compressed = compress_body(&original, Encoding::Zstd(3), 0).unwrap();
        let ratio = compressed.len() as f64 / original.len() as f64;
        assert!(
            ratio <= 0.50,
            "Zstd ratio {:.2} exceeds 50% on 10 KB JSON (compressed={} original={})",
            ratio,
            compressed.len(),
            original.len()
        );
    }

    // -- Gzip round-trip --

    #[test]
    fn test_gzip_roundtrip() {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let original = generate_json_10kb();
        let compressed = compress_body(&original, Encoding::Gzip(6), 0).unwrap();
        let mut decoder = GzDecoder::new(compressed.as_ref());
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).expect("gzip decompression failed");
        assert_eq!(decompressed, original, "Gzip round-trip mismatch");
    }

    #[test]
    fn test_gzip_compression_ratio_10kb_json() {
        let original = generate_json_10kb();
        let compressed = compress_body(&original, Encoding::Gzip(6), 0).unwrap();
        let ratio = compressed.len() as f64 / original.len() as f64;
        assert!(
            ratio <= 0.50,
            "Gzip ratio {:.2} exceeds 50% on 10 KB JSON (compressed={} original={})",
            ratio,
            compressed.len(),
            original.len()
        );
    }

    // -- Encoding::as_header_value --

    #[test]
    fn test_header_values() {
        assert_eq!(Encoding::Brotli(4).as_header_value(), "br");
        assert_eq!(Encoding::Zstd(3).as_header_value(), "zstd");
        assert_eq!(Encoding::Gzip(6).as_header_value(), "gzip");
        assert_eq!(Encoding::Identity.as_header_value(), "identity");
    }

    // -- helper --

    /// Generate a synthetic JSON payload of approximately 10 KB with high
    /// entropy-reduction potential (repetitive structure).
    fn generate_json_10kb() -> Vec<u8> {
        let mut obj = String::from("[");
        for i in 0..200 {
            if i > 0 {
                obj.push(',');
            }
            obj.push_str(&format!(
                r#"{{"id":{i},"name":"utilisateur_{i}","email":"user{i}@faso.bf","role":"agent","active":true,"score":{score:.2}}}"#,
                i = i,
                score = (i as f64) * 0.5 + 1.0,
            ));
        }
        obj.push(']');
        // Pad to ensure we're at least 10 KB.
        while obj.len() < 10_240 {
            obj.push(' ');
        }
        obj.into_bytes()
    }
}
