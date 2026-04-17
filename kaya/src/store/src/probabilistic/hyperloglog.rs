// SPDX-License-Identifier: AGPL-3.0-or-later
//! HyperLogLog++ cardinality estimator.
//!
//! Based on Flajolet et al. 2007 (HLL) and Heule et al. 2013 (HLL++),
//! with the linear-counting bias correction for low cardinalities.
//!
//! Default precision = 14 gives 2^14 = 16384 registers and relative error
//! of 1.04 / sqrt(16384) ~= 0.81%.
//!
//! # RESP3-compatible probabilistic commands
//!
//! - `PFADD <key> <element> [element ...]` — add elements.
//! - `PFCOUNT <key> [key ...]` — estimated cardinality (union across keys).
//! - `PFMERGE <dest> <src> [src ...]` — merge source HLLs into `dest`.

use bytes::{BufMut, Bytes, BytesMut};
use tracing::instrument;
use xxhash_rust::xxh3::xxh3_64_with_seed;

use super::error::ProbabilisticError;

const HLL_SEED: u64 = 0xC0FF_EE00_C0FF_EE00;
const MIN_PRECISION: u8 = 4;
const MAX_PRECISION: u8 = 18;
const DEFAULT_PRECISION: u8 = 14;
const MAGIC: &[u8; 4] = b"HLL1";

/// A HyperLogLog++ cardinality estimator.
#[derive(Debug, Clone)]
pub struct HyperLogLog {
    precision: u8,
    m: usize,
    registers: Vec<u8>,
}

impl HyperLogLog {
    /// Create a new HLL with the given precision (4..=18). Uses
    /// default precision (14) on invalid input.
    #[instrument(level = "debug", skip_all)]
    pub fn new(precision: u8) -> Self {
        let p = if (MIN_PRECISION..=MAX_PRECISION).contains(&precision) {
            precision
        } else {
            DEFAULT_PRECISION
        };
        let m = 1usize << p;
        Self {
            precision: p,
            m,
            registers: vec![0u8; m],
        }
    }

    /// Create with explicit precision validation. Returns an error if `precision`
    /// is outside [4, 18].
    pub fn try_new(precision: u8) -> Result<Self, ProbabilisticError> {
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&precision) {
            return Err(ProbabilisticError::InvalidPrecision(precision));
        }
        Ok(Self::new(precision))
    }

    /// PFADD: add a single item to the estimator.
    #[instrument(level = "trace", skip_all)]
    pub fn add(&mut self, item: &[u8]) {
        let h = xxh3_64_with_seed(item, HLL_SEED);
        let p = self.precision as u32;
        // Top `p` bits: register index.
        let idx = (h >> (64 - p)) as usize;
        // Remaining bits: count leading zeros + 1.
        let w = (h << p) | (1u64 << (p - 1));
        let rho = w.leading_zeros() as u8 + 1;
        if rho > self.registers[idx] {
            self.registers[idx] = rho;
        }
    }

    /// PFCOUNT: estimated cardinality.
    #[instrument(level = "trace", skip_all)]
    pub fn count(&self) -> u64 {
        let m = self.m as f64;
        let alpha = alpha(self.m);

        // Raw HLL estimate: alpha * m^2 / sum(2^-M[i])
        let mut sum = 0.0_f64;
        let mut zeros = 0usize;
        for &r in &self.registers {
            if r == 0 {
                zeros += 1;
            }
            sum += 2f64.powi(-(r as i32));
        }
        let e = alpha * m * m / sum;

        // Linear counting correction for low cardinalities (HLL++ bias fix).
        if e <= 2.5 * m && zeros > 0 {
            let lc = m * (m / zeros as f64).ln();
            return lc.round() as u64;
        }

        // Large-range correction (for 32-bit hash); with 64-bit hash this is
        // mostly unnecessary but we keep the classic guard.
        let two_32 = (1u64 << 32) as f64;
        if e > two_32 / 30.0 {
            // Our hash is 64-bit, skip the 32-bit saturation correction.
            return e.round() as u64;
        }

        e.round() as u64
    }

    /// Merge (union) another HLL into this one. Both must have the same
    /// precision; otherwise this is a no-op.
    #[instrument(level = "debug", skip_all)]
    pub fn merge(&mut self, other: &Self) {
        if self.precision != other.precision {
            return;
        }
        for (dst, &src) in self.registers.iter_mut().zip(other.registers.iter()) {
            if src > *dst {
                *dst = src;
            }
        }
    }

    /// Precision used by this HLL.
    pub fn precision(&self) -> u8 {
        self.precision
    }

    /// Serialize to wire format: magic(4) + precision(1) + registers.
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(4 + 1 + self.registers.len());
        buf.put_slice(MAGIC);
        buf.put_u8(self.precision);
        buf.put_slice(&self.registers);
        buf.freeze()
    }

    /// Deserialize from wire format.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, ProbabilisticError> {
        if bytes.len() < 5 {
            return Err(ProbabilisticError::Deserialize(
                "input too short for HLL header".into(),
            ));
        }
        if &bytes[0..4] != MAGIC {
            return Err(ProbabilisticError::Deserialize(format!(
                "bad HLL magic: expected {:?}, got {:?}",
                MAGIC,
                &bytes[0..4]
            )));
        }
        let precision = bytes[4];
        if !(MIN_PRECISION..=MAX_PRECISION).contains(&precision) {
            return Err(ProbabilisticError::InvalidPrecision(precision));
        }
        let m = 1usize << precision;
        let expected = 5 + m;
        if bytes.len() != expected {
            return Err(ProbabilisticError::Deserialize(format!(
                "wrong HLL length: expected {} got {}",
                expected,
                bytes.len()
            )));
        }
        let registers = bytes[5..].to_vec();
        Ok(Self {
            precision,
            m,
            registers,
        })
    }
}

impl Default for HyperLogLog {
    fn default() -> Self {
        Self::new(DEFAULT_PRECISION)
    }
}

/// HyperLogLog alpha constant for `m` registers.
fn alpha(m: usize) -> f64 {
    match m {
        16 => 0.673,
        32 => 0.697,
        64 => 0.709,
        _ => 0.7213 / (1.0 + 1.079 / m as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_count() {
        let mut hll = HyperLogLog::new(14);
        for i in 0..1000u32 {
            hll.add(&i.to_le_bytes());
        }
        let c = hll.count();
        let err = (c as f64 - 1000.0).abs() / 1000.0;
        assert!(err < 0.05, "error {err} too high, count={c}");
    }

    #[test]
    fn serialize_roundtrip() {
        let mut hll = HyperLogLog::new(12);
        for i in 0..500u32 {
            hll.add(&i.to_le_bytes());
        }
        let bytes = hll.serialize();
        let hll2 = HyperLogLog::deserialize(&bytes).unwrap();
        assert_eq!(hll.count(), hll2.count());
        assert_eq!(hll.precision(), hll2.precision());
    }
}
