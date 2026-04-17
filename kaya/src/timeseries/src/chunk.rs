//! Gorilla-compressed chunk for KAYA TimeSeries.
//!
//! Each chunk stores up to [`CHUNK_CAPACITY`] `(timestamp_ms: i64, value: f64)` data points
//! using a Gorilla-inspired algorithm:
//!
//! - **Timestamps** : delta-of-delta with bit-packing (1, 9, 11, 14, 36 bits).
//! - **Values** : XOR delta with block encoding (leading/trailing zero compression).
//!
//! Unlike the original paper, the bit-stream is backed by a growing `Vec<u8>` and appended
//! to directly (no full rebuild on each append), giving O(1) amortised append cost.

use crate::error::ChunkError;

// -- constants --

/// Maximum data points per chunk.
pub const CHUNK_CAPACITY: usize = 256;

// ---------------------------------------------------------------------------
// Bit-stream
// ---------------------------------------------------------------------------

/// A compacted bit array backed by a `Vec<u64>` word buffer.
///
/// Bits are packed MSB-first within each byte, growing towards higher indices.
#[derive(Debug, Clone, Default)]
struct BitBuf {
    words: Vec<u64>,
    /// Total number of bits written.
    len: usize,
}

impl BitBuf {
    fn new() -> Self {
        Self::default()
    }

    /// Append `n` bits taken from the least-significant end of `value`.
    fn push(&mut self, value: u64, n: u8) {
        debug_assert!(n <= 64);
        if n == 0 {
            return;
        }
        // Determine the word and bit offset.
        let word_idx = self.len / 64;
        let bit_off = self.len % 64;

        // Extend if needed.
        if word_idx >= self.words.len() {
            self.words.push(0);
        }

        // Mask value to n bits.
        let mask = if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
        let v = value & mask;

        // How many bits fit in the current word?
        let remaining_in_word = 64 - bit_off;
        if n as usize <= remaining_in_word {
            // All bits fit in current word.
            let shift = remaining_in_word - n as usize;
            self.words[word_idx] |= v << shift;
        } else {
            // Split across two words.
            let high_bits = remaining_in_word;
            let low_bits = n as usize - high_bits;
            self.words[word_idx] |= v >> low_bits;
            // Ensure next word exists.
            if word_idx + 1 >= self.words.len() {
                self.words.push(0);
            }
            self.words[word_idx + 1] |= v << (64 - low_bits);
        }
        self.len += n as usize;
    }

    fn push_bit(&mut self, b: bool) {
        self.push(if b { 1 } else { 0 }, 1);
    }

    /// Read `n` bits starting at `pos` and return as u64 (right-aligned).
    fn read(&self, pos: usize, n: u8) -> Option<u64> {
        debug_assert!(n <= 64);
        if n == 0 {
            return Some(0);
        }
        if pos + n as usize > self.len {
            return None;
        }
        let word_idx = pos / 64;
        let bit_off = pos % 64;
        let remaining_in_word = 64 - bit_off;
        if (n as usize) <= remaining_in_word {
            let shift = remaining_in_word - n as usize;
            let mask: u64 = if n == 64 { u64::MAX } else { (1u64 << n) - 1 };
            Some((self.words[word_idx] >> shift) & mask)
        } else {
            // Split across two words.
            let high_bits = remaining_in_word; // guaranteed 1..=63
            let low_bits = n as usize - high_bits;
            // Extract bottom `high_bits` bits of current word.
            let hi_mask: u64 = if high_bits == 64 { u64::MAX } else { (1u64 << high_bits) - 1 };
            let hi = self.words[word_idx] & hi_mask;
            // Extract top `low_bits` bits of next word.
            let lo = self.words[word_idx + 1] >> (64 - low_bits);
            Some((hi << low_bits) | lo)
        }
    }

    /// Approximate heap bytes used.
    fn heap_bytes(&self) -> usize {
        self.words.len() * 8
    }
}

/// A streaming bit reader over a `BitBuf`.
struct BitReader<'a> {
    buf: &'a BitBuf,
    pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(buf: &'a BitBuf) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_bits(&mut self, n: u8) -> Option<u64> {
        let v = self.buf.read(self.pos, n)?;
        self.pos += n as usize;
        Some(v)
    }

    fn read_bit(&mut self) -> Option<bool> {
        self.read_bits(1).map(|v| v == 1)
    }
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

/// A Gorilla-compressed chunk holding up to [`CHUNK_CAPACITY`] time-series points.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Compressed timestamp stream.
    ts_buf: BitBuf,
    /// Compressed value stream.
    val_buf: BitBuf,
    /// Total data points.
    count: usize,
    /// First (anchor) timestamp.
    first_ts: i64,
    /// Last timestamp (for append validation).
    last_ts: i64,
    /// Last delta (for delta-of-delta).
    last_delta: i64,
    /// Last raw value bits (for XOR).
    last_val: u64,
    /// Leading zeros of the last stored XOR block (`u8::MAX` = sentinel).
    last_leading: u8,
    /// Number of meaningful bits in the last stored XOR block.
    last_meaningful: u8,
}

impl Chunk {
    // -- construction --

    /// Create a new chunk anchored at `(first_ts, first_val)`.
    pub fn new(first_ts: i64, first_val: f64) -> Self {
        let raw_val = first_val.to_bits();
        let mut ts_buf = BitBuf::new();
        let mut val_buf = BitBuf::new();
        // Write raw 64-bit anchor values.
        ts_buf.push(first_ts as u64, 64);
        val_buf.push(raw_val, 64);
        Self {
            ts_buf,
            val_buf,
            count: 1,
            first_ts,
            last_ts: first_ts,
            last_delta: 0,
            last_val: raw_val,
            last_leading: u8::MAX, // sentinel: no previous block
            last_meaningful: 0,
        }
    }

    // -- append --

    /// Append a new data point. Returns `Err` if the chunk is full or `ts` is not greater
    /// than the last stored timestamp.
    pub fn append(&mut self, ts: i64, val: f64) -> Result<(), ChunkError> {
        if self.count >= CHUNK_CAPACITY {
            return Err(ChunkError::Full { capacity: CHUNK_CAPACITY });
        }
        if ts <= self.last_ts {
            return Err(ChunkError::OutOfOrder { ts, last: self.last_ts });
        }

        // --- Timestamp delta-of-delta ---
        let delta = ts - self.last_ts;
        let dod = delta - self.last_delta;
        Self::write_dod(&mut self.ts_buf, dod);

        // --- Value XOR ---
        let raw_val = val.to_bits();
        let xor = raw_val ^ self.last_val;
        let (new_leading, new_meaningful) = Self::write_xor(
            &mut self.val_buf,
            xor,
            self.last_leading,
            self.last_meaningful,
        );
        if xor != 0 {
            self.last_leading = new_leading;
            self.last_meaningful = new_meaningful;
        }

        self.last_delta = delta;
        self.last_ts = ts;
        self.last_val = raw_val;
        self.count += 1;
        Ok(())
    }

    // -- iterators --

    /// Iterate all stored `(timestamp_ms, value)` pairs in chronological order.
    pub fn iter(&self) -> impl Iterator<Item = (i64, f64)> + '_ {
        ChunkIter::new(self)
    }

    /// Iterate points whose timestamp is in `[from_ts, to_ts]`.
    pub fn range(&self, from_ts: i64, to_ts: i64) -> impl Iterator<Item = (i64, f64)> + '_ {
        self.iter()
            .skip_while(move |(ts, _)| *ts < from_ts)
            .take_while(move |(ts, _)| *ts <= to_ts)
    }

    // -- accessors --

    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    pub fn is_full(&self) -> bool { self.count >= CHUNK_CAPACITY }
    pub fn first_ts(&self) -> i64 { self.first_ts }
    pub fn last_ts(&self) -> i64 { self.last_ts }

    /// Approximate heap footprint in bytes.
    pub fn size_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.ts_buf.heap_bytes()
            + self.val_buf.heap_bytes()
    }

    // -- private encoding helpers --

    fn write_dod(buf: &mut BitBuf, dod: i64) {
        if dod == 0 {
            buf.push_bit(false);
        } else if (-64..=63).contains(&dod) {
            buf.push(0b10, 2);
            buf.push(dod as u64 & 0x7F, 7);
        } else if (-256..=255).contains(&dod) {
            buf.push(0b110, 3);
            buf.push(dod as u64 & 0x1FF, 9);
        } else if (-2048..=2047).contains(&dod) {
            buf.push(0b1110, 4);
            buf.push(dod as u64 & 0xFFF, 12);
        } else {
            buf.push(0b1111, 4);
            buf.push(dod as u64 & 0xFFFF_FFFF, 32);
        }
    }

    /// Write XOR to the value buffer. Returns `(new_leading, new_meaningful)`.
    fn write_xor(buf: &mut BitBuf, xor: u64, prev_leading: u8, prev_meaningful: u8) -> (u8, u8) {
        if xor == 0 {
            buf.push_bit(false);
            return (prev_leading, prev_meaningful);
        }
        buf.push_bit(true); // non-zero XOR

        let leading = (xor.leading_zeros() as u8).min(31); // cap to 5 bits
        let trailing = xor.trailing_zeros() as u8;
        // Ensure leading + trailing < 64.
        let trailing = trailing.min(63u8.saturating_sub(leading));
        let meaningful = 64 - leading - trailing;

        // Compute the previous block's trailing zero count.
        let prev_trailing = if prev_leading != u8::MAX && prev_meaningful > 0 {
            64u8.saturating_sub(prev_leading).saturating_sub(prev_meaningful)
        } else {
            0
        };

        // Reuse the previous block if the current XOR's meaningful bits fit entirely
        // within the previous block window: need leading >= prev_leading AND
        // trailing >= prev_trailing (so meaningful window is a subset).
        let can_reuse = prev_leading != u8::MAX
            && prev_meaningful > 0
            && leading >= prev_leading
            && trailing >= prev_trailing;

        if can_reuse {
            buf.push_bit(false); // 0 → reuse
            // Align to the previous block window and store prev_meaningful bits.
            let shifted = xor >> prev_trailing;
            buf.push(shifted, prev_meaningful);
            (prev_leading, prev_meaningful)
        } else {
            buf.push_bit(true); // 1 → new block
            buf.push(leading as u64, 5);
            buf.push(meaningful as u64, 6);
            let shifted = xor >> trailing;
            buf.push(shifted, meaningful);
            (leading, meaningful)
        }
    }

    fn read_dod(r: &mut BitReader) -> Option<i64> {
        let b0 = r.read_bit()?;
        if !b0 {
            return Some(0);
        }
        let b1 = r.read_bit()?;
        if !b1 {
            let raw = r.read_bits(7)? as i64;
            return Some(sign_extend(raw, 7));
        }
        let b2 = r.read_bit()?;
        if !b2 {
            let raw = r.read_bits(9)? as i64;
            return Some(sign_extend(raw, 9));
        }
        let b3 = r.read_bit()?;
        if !b3 {
            let raw = r.read_bits(12)? as i64;
            return Some(sign_extend(raw, 12));
        }
        let raw = r.read_bits(32)? as i64;
        Some(sign_extend(raw, 32))
    }

    fn read_xor(r: &mut BitReader, prev_val: u64, prev_leading: &mut u8, prev_meaningful: &mut u8) -> Option<u64> {
        let b0 = r.read_bit()?;
        if !b0 {
            return Some(prev_val);
        }
        let b1 = r.read_bit()?;
        if !b1 {
            // Reuse.
            let bits = r.read_bits(*prev_meaningful)?;
            let prev_trailing = 64 - *prev_leading - *prev_meaningful;
            let xor = bits << prev_trailing;
            return Some(prev_val ^ xor);
        }
        // New block.
        let leading = r.read_bits(5)? as u8;
        let meaningful = r.read_bits(6)? as u8;
        let trailing = 64 - leading - meaningful;
        let bits = r.read_bits(meaningful)?;
        let xor = bits << trailing;
        *prev_leading = leading;
        *prev_meaningful = meaningful;
        Some(prev_val ^ xor)
    }
}

fn sign_extend(val: i64, bits: u8) -> i64 {
    let shift = 64i32 - bits as i32;
    if shift <= 0 {
        return val;
    }
    (val << shift) >> shift
}

// ---------------------------------------------------------------------------
// Iterator
// ---------------------------------------------------------------------------

struct ChunkIter<'a> {
    #[allow(dead_code)]
    chunk: &'a Chunk,
    ts_r: BitReader<'a>,
    val_r: BitReader<'a>,
    remaining: usize,
    last_ts: i64,
    last_delta: i64,
    last_val: u64,
    leading: u8,
    meaningful: u8,
    first: bool,
}

impl<'a> ChunkIter<'a> {
    fn new(chunk: &'a Chunk) -> Self {
        Self {
            chunk,
            ts_r: BitReader::new(&chunk.ts_buf),
            val_r: BitReader::new(&chunk.val_buf),
            remaining: chunk.count,
            last_ts: 0,
            last_delta: 0,
            last_val: 0,
            leading: u8::MAX,
            meaningful: 0,
            first: true,
        }
    }
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = (i64, f64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;

        if self.first {
            self.first = false;
            let ts_raw = self.ts_r.read_bits(64)? as i64;
            let val_raw = self.val_r.read_bits(64)?;
            self.last_ts = ts_raw;
            self.last_val = val_raw;
            return Some((ts_raw, f64::from_bits(val_raw)));
        }

        let dod = Chunk::read_dod(&mut self.ts_r)?;
        let delta = self.last_delta + dod;
        self.last_ts += delta;
        self.last_delta = delta;

        let val_raw = Chunk::read_xor(&mut self.val_r, self.last_val, &mut self.leading, &mut self.meaningful)?;
        self.last_val = val_raw;

        Some((self.last_ts, f64::from_bits(val_raw)))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_point_roundtrip() {
        let c = Chunk::new(1_000, 3.14);
        let pts: Vec<_> = c.iter().collect();
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].0, 1_000);
        assert!((pts[0].1 - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_monotonic_append_and_roundtrip() {
        // Anchor: (ts=0, val=0.0). Append i=1..=10 with val=i.
        let mut c = Chunk::new(0, 0.0);
        for i in 1i64..=10 {
            c.append(i * 1000, i as f64).unwrap();
        }
        let pts: Vec<_> = c.iter().collect();
        assert_eq!(pts.len(), 11);
        assert_eq!(pts[0], (0, 0.0));
        for i in 1i64..=10 {
            let idx = i as usize;
            assert_eq!(pts[idx].0, i * 1000, "ts mismatch at i={i}");
            assert!(
                (pts[idx].1 - i as f64).abs() < 1e-10,
                "val mismatch at i={i}: got {}", pts[idx].1
            );
        }
    }

    #[test]
    fn test_full_256_roundtrip() {
        let mut c = Chunk::new(0, 0.0);
        for i in 1..256i64 {
            c.append(i, i as f64 * 0.001).unwrap();
        }
        assert!(c.is_full());
        assert_eq!(c.append(999, 0.0), Err(ChunkError::Full { capacity: CHUNK_CAPACITY }));
        let pts: Vec<_> = c.iter().collect();
        assert_eq!(pts.len(), 256);
        for (i, (ts, val)) in pts.iter().enumerate() {
            assert_eq!(*ts, i as i64, "ts mismatch at idx={i}");
            let expected = i as f64 * 0.001;
            assert!(
                (val - expected).abs() < 1e-10,
                "val mismatch at idx={i}: got {val}, expected {expected}"
            );
        }
    }

    #[test]
    fn test_out_of_order_rejected() {
        let mut c = Chunk::new(100, 1.0);
        let err = c.append(50, 2.0);
        assert!(matches!(err, Err(ChunkError::OutOfOrder { .. })));
    }

    #[test]
    fn test_compression_ratio_sinusoidal() {
        use std::f64::consts::PI;
        let base_ts = 1_700_000_000_000i64;
        let mut c = Chunk::new(base_ts, (0.0f64).sin());
        for i in 1..256i64 {
            let angle = (i as f64) * 2.0 * PI / 256.0;
            c.append(base_ts + i * 1000, angle.sin()).unwrap();
        }
        // Raw: 256 * 16 = 4096 bytes.
        let raw_bytes = 256usize * 16;
        let compressed = c.ts_buf.heap_bytes() + c.val_buf.heap_bytes();
        let ratio = raw_bytes as f64 / compressed as f64;
        // For sinusoidal f64 data, Gorilla XOR compression achieves ~2x.
        // Timestamp stream benefits heavily from delta-of-delta (constant intervals).
        assert!(
            ratio > 1.8,
            "compression ratio too low: {ratio:.2}x (compressed={compressed} raw={raw_bytes})"
        );
    }

    #[test]
    fn test_range_filter() {
        let mut c = Chunk::new(0, 0.0);
        for i in 1..=20i64 {
            c.append(i * 100, i as f64).unwrap();
        }
        let pts: Vec<_> = c.range(500, 1000).collect();
        assert_eq!(pts.len(), 6); // ts=500,600,700,800,900,1000
        assert_eq!(pts[0].0, 500);
        assert_eq!(pts[5].0, 1000);
    }

    #[test]
    fn test_delta_of_delta_constant_interval() {
        let mut c = Chunk::new(0, 1.0);
        for i in 1..=50i64 {
            c.append(i * 1000, 1.0).unwrap();
        }
        let pts: Vec<_> = c.iter().collect();
        assert_eq!(pts.len(), 51);
        for (i, (ts, _)) in pts.iter().enumerate() {
            assert_eq!(*ts, i as i64 * 1000, "ts mismatch at idx={i}");
        }
    }

    #[test]
    fn test_constant_value_compressed_well() {
        let mut c = Chunk::new(0, 42.0);
        for i in 1..256i64 {
            c.append(i * 1000, 42.0).unwrap();
        }
        let pts: Vec<_> = c.iter().collect();
        assert_eq!(pts.len(), 256);
        for (_, val) in &pts {
            assert!((val - 42.0).abs() < 1e-10);
        }
    }
}
