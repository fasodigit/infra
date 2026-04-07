//! KAYA Compression Library
//!
//! Provides LZ4 and Zstd compression/decompression for values stored in KAYA.
//! Values below a configurable threshold are stored uncompressed.

use bytes::Bytes;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum CompressError {
    #[error("lz4 compression failed: {0}")]
    Lz4(String),

    #[error("zstd compression failed: {0}")]
    Zstd(#[from] std::io::Error),

    #[error("unknown algorithm tag: {0}")]
    UnknownAlgorithm(u8),

    #[error("data too short to contain compression header")]
    TooShort,
}

// ---------------------------------------------------------------------------
// Algorithm enum
// ---------------------------------------------------------------------------

/// Compression algorithm selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    None,
    Lz4,
    Zstd,
}

impl Algorithm {
    /// One-byte tag written before compressed payload.
    pub const fn tag(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Lz4 => 1,
            Self::Zstd => 2,
        }
    }

    pub fn from_tag(tag: u8) -> Result<Self, CompressError> {
        match tag {
            0 => Ok(Self::None),
            1 => Ok(Self::Lz4),
            2 => Ok(Self::Zstd),
            other => Err(CompressError::UnknownAlgorithm(other)),
        }
    }
}

impl Default for Algorithm {
    fn default() -> Self {
        Self::Lz4
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Runtime configuration for the compressor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressConfig {
    /// Which algorithm to use.
    pub algorithm: Algorithm,
    /// Minimum value size (bytes) to trigger compression.
    pub min_size: usize,
    /// Zstd compression level (1..=22).
    pub zstd_level: i32,
}

impl Default for CompressConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Lz4,
            min_size: 256,
            zstd_level: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

/// Stateless compressor / decompressor.
#[derive(Debug, Clone)]
pub struct Compressor {
    config: CompressConfig,
}

impl Compressor {
    pub fn new(config: CompressConfig) -> Self {
        Self { config }
    }

    /// Compress `data`. Returns `(algorithm_tag, payload)`.
    /// If the data is below `min_size`, returns it unchanged with tag 0.
    pub fn compress(&self, data: &[u8]) -> Result<Bytes, CompressError> {
        if data.len() < self.config.min_size {
            // Prefix with "None" tag
            let mut out = Vec::with_capacity(1 + data.len());
            out.push(Algorithm::None.tag());
            out.extend_from_slice(data);
            return Ok(Bytes::from(out));
        }

        match self.config.algorithm {
            Algorithm::None => {
                let mut out = Vec::with_capacity(1 + data.len());
                out.push(Algorithm::None.tag());
                out.extend_from_slice(data);
                Ok(Bytes::from(out))
            }
            Algorithm::Lz4 => {
                let compressed = lz4_flex::compress_prepend_size(data);
                let mut out = Vec::with_capacity(1 + compressed.len());
                out.push(Algorithm::Lz4.tag());
                out.extend_from_slice(&compressed);
                Ok(Bytes::from(out))
            }
            Algorithm::Zstd => {
                let compressed =
                    zstd::encode_all(data, self.config.zstd_level)?;
                let mut out = Vec::with_capacity(1 + compressed.len());
                out.push(Algorithm::Zstd.tag());
                out.extend_from_slice(&compressed);
                Ok(Bytes::from(out))
            }
        }
    }

    /// Decompress a payload previously produced by [`compress`].
    pub fn decompress(&self, data: &[u8]) -> Result<Bytes, CompressError> {
        if data.is_empty() {
            return Err(CompressError::TooShort);
        }

        let algo = Algorithm::from_tag(data[0])?;
        let payload = &data[1..];

        match algo {
            Algorithm::None => Ok(Bytes::copy_from_slice(payload)),
            Algorithm::Lz4 => {
                let decompressed = lz4_flex::decompress_size_prepended(payload)
                    .map_err(|e| CompressError::Lz4(e.to_string()))?;
                Ok(Bytes::from(decompressed))
            }
            Algorithm::Zstd => {
                let decompressed = zstd::decode_all(payload)?;
                Ok(Bytes::from(decompressed))
            }
        }
    }

    pub fn config(&self) -> &CompressConfig {
        &self.config
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new(CompressConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_lz4() {
        let c = Compressor::new(CompressConfig {
            algorithm: Algorithm::Lz4,
            min_size: 0,
            ..Default::default()
        });
        let data = b"hello world, this is KAYA compression test data!";
        let compressed = c.compress(data).unwrap();
        let decompressed = c.decompress(&compressed).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }

    #[test]
    fn below_min_size_is_noop() {
        let c = Compressor::default(); // min_size = 256
        let data = b"short";
        let compressed = c.compress(data).unwrap();
        assert_eq!(compressed[0], Algorithm::None.tag());
        let decompressed = c.decompress(&compressed).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }
}
