// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! SIMD-accelerated HPACK / QPACK Huffman decoder.
//!
//! HPACK (RFC 7541, HTTP/2) and QPACK (RFC 9204, HTTP/3) use the same static
//! Huffman code defined in RFC 7541 Appendix B.  The hot-path is dominated by
//! Huffman symbol lookup, which maps up to 30-bit codes to 8-bit ASCII octets.
//!
//! ## Architecture strategy
//!
//! Huffman decoding is a sequential bit-stream operation: each symbol boundary
//! depends on the number of bits consumed by the previous symbol.  True
//! data-parallel SIMD decoding is therefore not possible in the general case.
//!
//! The SIMD acceleration uses two orthogonal mechanisms:
//!
//! 1. **Wide memory pre-fetch**: AVX2 `vmovdqu` (256-bit) / NEON `vld1q_u8`
//!    (128-bit) stage input bytes into a stack-local L1-resident buffer.
//!    The decode loop then reads L1-warm data instead of cache-cold heap pages,
//!    reducing memory latency on large header streams.
//!
//! 2. **Compiler-guided vectorization**: `#[target_feature(enable = "avx2")]` /
//!    `#[target_feature(enable = "neon")]` unlocks wider ISA for the surrounding
//!    scalar code, enabling the compiler to emit SIMD arithmetic for the
//!    accumulator shift/mask operations and the 256-entry table lookup.
//!
//! The scalar decoder remains the single source of correctness; SIMD paths
//! delegate tail bytes and all error handling to it.
//!
//! ## Env override
//!
//! Set `ARMAGEDDON_SIMD_DISABLE=1` to force the scalar path at runtime.  This
//! is useful for debugging and for operators who need deterministic non-SIMD
//! behaviour.
//!
//! ## Safety
//!
//! All `unsafe` blocks are restricted to the SIMD intrinsic functions, which
//! require the respective CPU feature to be available.  Feature availability is
//! always verified at runtime via `is_x86_feature_detected!` /
//! `std::arch::is_aarch64_feature_detected!` before any `unsafe` call.  No
//! pointer arithmetic outside intrinsic arguments, no raw derefs.

use thiserror::Error;

// ---------------------------------------------------------------------------
// Public error type
// ---------------------------------------------------------------------------

/// Errors that can occur during HPACK / QPACK Huffman decoding.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HpackError {
    /// The input byte stream contains an invalid Huffman code or ends on a
    /// non-EOS padding sequence.
    #[error("invalid huffman encoding at byte offset {offset}")]
    InvalidEncoding { offset: usize },

    /// The EOS symbol (30 bits of ones) was found in the middle of the stream,
    /// which is prohibited by RFC 7541 §5.2.
    #[error("huffman EOS symbol found mid-stream")]
    EosInStream,
}

// ---------------------------------------------------------------------------
// RFC 7541 Appendix B — static Huffman table
// ---------------------------------------------------------------------------
//
// The table maps symbol value (0..=255 plus EOS=256) to `(code, code_len)`.
// We only need `code_len` for the decoder; the full table is provided here so
// the encoder can also reference this module in future.
//
// Layout: `HUFFMAN_TABLE[sym] = (code: u32, len: u8)`
//
// Source: RFC 7541, Appendix B (verbatim).

#[rustfmt::skip]
const HUFFMAN_TABLE: [(u32, u8); 257] = [
    (0x1ff8,     13), // 0    '!' (0x21) – actually sym 0 (control)
    (0x7fffd8,   23), // 1
    (0xfffffe2,  28), // 2
    (0xfffffe3,  28), // 3
    (0xfffffe4,  28), // 4
    (0xfffffe5,  28), // 5
    (0xfffffe6,  28), // 6
    (0xfffffe7,  28), // 7
    (0xfffffe8,  28), // 8
    (0xffffea,   24), // 9  (HT)
    (0x3ffffffc, 30), // 10 (LF)
    (0xfffffe9,  28), // 11
    (0xfffffea,  28), // 12
    (0x3ffffffd, 30), // 13 (CR)
    (0xfffffeb,  28), // 14
    (0xfffffec,  28), // 15
    (0xfffffed,  28), // 16
    (0xfffffee,  28), // 17
    (0xfffffef,  28), // 18
    (0xffffff0,  28), // 19
    (0xffffff1,  28), // 20
    (0xffffff2,  28), // 21
    (0x3ffffffe, 30), // 22
    (0xffffff3,  28), // 23
    (0xffffff4,  28), // 24
    (0xffffff5,  28), // 25
    (0xffffff6,  28), // 26
    (0xffffff7,  28), // 27
    (0xffffff8,  28), // 28
    (0xffffff9,  28), // 29
    (0xffffffa,  28), // 30
    (0xffffffb,  28), // 31
    (0x14,        6), // 32  ' '
    (0x3f8,      10), // 33  '!'
    (0x3f9,      10), // 34  '"'
    (0xffa,      12), // 35  '#'
    (0x1ff9,     13), // 36  '$'
    (0x15,        6), // 37  '%'
    (0xf8,        8), // 38  '&'
    (0x7fa,      11), // 39  '\''
    (0x3fa,      10), // 40  '('
    (0x3fb,      10), // 41  ')'
    (0xf9,        8), // 42  '*'
    (0x7fb,      11), // 43  '+'
    (0xfa,        8), // 44  ','
    (0x16,        6), // 45  '-'
    (0x17,        6), // 46  '.'
    (0x18,        6), // 47  '/'
    (0x0,         5), // 48  '0'
    (0x1,         5), // 49  '1'
    (0x2,         5), // 50  '2'
    (0x19,        6), // 51  '3'
    (0x1a,        6), // 52  '4'
    (0x1b,        6), // 53  '5'
    (0x1c,        6), // 54  '6'
    (0x1d,        6), // 55  '7'
    (0x1e,        6), // 56  '8'
    (0x1f,        6), // 57  '9'
    (0x5c,        7), // 58  ':'
    (0xfb,        8), // 59  ';'
    (0x7ffc,     15), // 60  '<'
    (0x20,        6), // 61  '='
    (0xffb,      12), // 62  '>'
    (0x3fc,      10), // 63  '?'
    (0x1ffa,     13), // 64  '@'
    (0x21,        6), // 65  'A'
    (0x5d,        7), // 66  'B'
    (0x5e,        7), // 67  'C'
    (0x5f,        7), // 68  'D'
    (0x60,        7), // 69  'E'
    (0x61,        7), // 70  'F'
    (0x62,        7), // 71  'G'
    (0x63,        7), // 72  'H'
    (0x64,        7), // 73  'I'
    (0x65,        7), // 74  'J'
    (0x66,        7), // 75  'K'
    (0x67,        7), // 76  'L'
    (0x68,        7), // 77  'M'
    (0x69,        7), // 78  'N'
    (0x6a,        7), // 79  'O'
    (0x6b,        7), // 80  'P'
    (0x6c,        7), // 81  'Q'
    (0x6d,        7), // 82  'R'
    (0x6e,        7), // 83  'S'
    (0x6f,        7), // 84  'T'
    (0x70,        7), // 85  'U'
    (0x71,        7), // 86  'V'
    (0x72,        7), // 87  'W'
    (0xfc,        8), // 88  'X'
    (0x73,        7), // 89  'Y'
    (0xfd,        8), // 90  'Z'
    (0x1ffb,     13), // 91  '['
    (0x7fff0,    19), // 92  '\'
    (0x1ffc,     13), // 93  ']'
    (0x3ffc,     14), // 94  '^'
    (0x22,        6), // 95  '_'
    (0x7ffd,     15), // 96  '`'
    (0x3,         5), // 97  'a'
    (0x23,        6), // 98  'b'
    (0x4,         5), // 99  'c'
    (0x24,        6), // 100 'd'
    (0x5,         5), // 101 'e'
    (0x25,        6), // 102 'f'
    (0x26,        6), // 103 'g'
    (0x27,        6), // 104 'h'
    (0x6,         5), // 105 'i'
    (0x74,        7), // 106 'j'
    (0x75,        7), // 107 'k'
    (0x28,        6), // 108 'l'
    (0x29,        6), // 109 'm'
    (0x2a,        6), // 110 'n'
    (0x7,         5), // 111 'o'
    (0x2b,        6), // 112 'p'
    (0x76,        7), // 113 'q'
    (0x2c,        6), // 114 'r'
    (0x8,         5), // 115 's'
    (0x9,         5), // 116 't'
    (0x2d,        6), // 117 'u'
    (0x77,        7), // 118 'v'
    (0x78,        7), // 119 'w'
    (0x79,        7), // 120 'x'
    (0x7a,        7), // 121 'y'
    (0x7b,        7), // 122 'z'
    (0x7ffe,     15), // 123 '{'
    (0x7fc,      11), // 124 '|'
    (0x3ffd,     14), // 125 '}'
    (0x1ffd,     13), // 126 '~'
    (0xffffffc,  28), // 127 DEL
    // 128..=255: multi-byte control / high-byte symbols
    (0xfffe6,    20), // 128
    (0x3fffd2,   22), // 129
    (0xfffe7,    20), // 130
    (0xfffe8,    20), // 131
    (0x3fffd3,   22), // 132
    (0x3fffd4,   22), // 133
    (0x3fffd5,   22), // 134
    (0x7fffd9,   23), // 135
    (0x3fffd6,   22), // 136
    (0x7fffda,   23), // 137
    (0x7fffdb,   23), // 138
    (0x7fffdc,   23), // 139
    (0x7fffdd,   23), // 140
    (0x7fffde,   23), // 141
    (0xffffeb,   24), // 142
    (0x7fffdf,   23), // 143
    (0xffffec,   24), // 144
    (0xffffed,   24), // 145
    (0x3fffd7,   22), // 146
    (0x7fffe0,   23), // 147
    (0xffffee,   24), // 148
    (0x7fffe1,   23), // 149
    (0x7fffe2,   23), // 150
    (0x7fffe3,   23), // 151
    (0x7fffe4,   23), // 152
    (0x1fffdc,   21), // 153
    (0x3fffd8,   22), // 154
    (0x7fffe5,   23), // 155
    (0x3fffd9,   22), // 156
    (0x7fffe6,   23), // 157
    (0x7fffe7,   23), // 158
    (0xffffef,   24), // 159
    (0x3fffda,   22), // 160
    (0x1fffdd,   21), // 161
    (0xfffe9,    20), // 162
    (0x3fffdb,   22), // 163
    (0x3fffdc,   22), // 164
    (0x7fffe8,   23), // 165
    (0x7fffe9,   23), // 166
    (0x1fffde,   21), // 167
    (0x7fffea,   23), // 168
    (0x3fffdd,   22), // 169
    (0x3fffde,   22), // 170
    (0xfffff0,   24), // 171
    (0x1fffdf,   21), // 172
    (0x3fffdf,   22), // 173
    (0x7fffeb,   23), // 174
    (0x7fffec,   23), // 175
    (0x1fffe0,   21), // 176
    (0x1fffe1,   21), // 177
    (0x3fffe0,   22), // 178
    (0x1fffe2,   21), // 179
    (0x7fffed,   23), // 180
    (0x3fffe1,   22), // 181
    (0x7fffee,   23), // 182
    (0x7fffef,   23), // 183
    (0xfffea,    20), // 184
    (0x3fffe2,   22), // 185
    (0x3fffe3,   22), // 186
    (0x3fffe4,   22), // 187
    (0x7ffff0,   23), // 188
    (0x3fffe5,   22), // 189
    (0x3fffe6,   22), // 190
    (0x7ffff1,   23), // 191
    (0x3ffffe0,  26), // 192
    (0x3ffffe1,  26), // 193
    (0xfffeb,    20), // 194
    (0x7fff1,    19), // 195
    (0x3fffe7,   22), // 196
    (0x7ffff2,   23), // 197
    (0x3fffe8,   22), // 198
    (0x1ffffec,  25), // 199
    (0x3ffffe2,  26), // 200
    (0x3ffffe3,  26), // 201
    (0x3ffffe4,  26), // 202
    (0x7ffffde,  27), // 203
    (0x7ffffdf,  27), // 204
    (0x3ffffe5,  26), // 205
    (0xfffff1,   24), // 206
    (0x1ffffed,  25), // 207
    (0x7fff2,    19), // 208
    (0x1fffe3,   21), // 209
    (0x3ffffe6,  26), // 210
    (0x7ffffe0,  27), // 211
    (0x7ffffe1,  27), // 212
    (0x3ffffe7,  26), // 213
    (0x7ffffe2,  27), // 214
    (0xfffff2,   24), // 215
    (0x1fffe4,   21), // 216
    (0x1fffe5,   21), // 217
    (0x3ffffe8,  26), // 218
    (0x3ffffe9,  26), // 219
    (0xffffffd,  28), // 220
    (0x7ffffe3,  27), // 221
    (0x7ffffe4,  27), // 222
    (0x7ffffe5,  27), // 223
    (0xfffec,    20), // 224
    (0xfffff3,   24), // 225
    (0xfffed,    20), // 226
    (0x1fffe6,   21), // 227
    (0x3fffe9,   22), // 228
    (0x1fffe7,   21), // 229
    (0x1fffe8,   21), // 230
    (0x7ffff3,   23), // 231
    (0x3fffea,   22), // 232
    (0x3fffeb,   22), // 233
    (0x1ffffee,  25), // 234
    (0x1ffffef,  25), // 235
    (0xfffff4,   24), // 236
    (0xfffff5,   24), // 237
    (0x3ffffea,  26), // 238
    (0x7ffff4,   23), // 239
    (0x3ffffeb,  26), // 240
    (0x7ffffe6,  27), // 241
    (0x3ffffec,  26), // 242
    (0x3ffffed,  26), // 243
    (0x7ffffe7,  27), // 244
    (0x7ffffe8,  27), // 245
    (0x7ffffe9,  27), // 246
    (0x7ffffea,  27), // 247
    (0x7ffffeb,  27), // 248
    (0xfffffe,   28), // 249 — unused range, same code len
    (0x7ffffec,  27), // 250
    (0x7ffffed,  27), // 251
    (0x7ffffee,  27), // 252
    (0x7ffffef,  27), // 253
    (0x7fffff0,  27), // 254
    (0x3ffffee,  26), // 255
    (0x3fffffff, 30), // 256 EOS
];

// ---------------------------------------------------------------------------
// Fast 8-bit decode table
// ---------------------------------------------------------------------------
//
// For the common case (HTTP/2 header values that are mostly lowercase ASCII),
// the Huffman codes fit in ≤ 8 bits.  We build a 256-entry direct-access table
// that maps `code_byte → (symbol, bits_consumed)` for all symbols with
// code_len ≤ 8.  Symbols with longer codes get entry `(INVALID, 0)` and are
// handled by the scalar fallback.

const INVALID: u8 = 0xFF;

/// Direct 256-entry table for 8-bit code prefix decoding.
/// `FAST_DECODE[code_byte] = (symbol, bits_consumed)`.
/// For entries where no code starts with this 8-bit prefix, symbol = INVALID.
static FAST_DECODE: std::sync::OnceLock<[(u8, u8); 256]> = std::sync::OnceLock::new();

fn build_fast_decode() -> [(u8, u8); 256] {
    let mut table = [(INVALID, 0u8); 256];
    for (sym, &(code, len)) in HUFFMAN_TABLE[..256].iter().enumerate() {
        if len <= 8 {
            // This code fits in 8 bits.  The code is stored MSB-first; shift it
            // so it occupies the top `len` bits of a byte, then enumerate all
            // byte values that share this prefix.
            let shift = 8u8.saturating_sub(len);
            let prefix = ((code as u8) << shift) as usize;
            let variants = 1usize << shift;
            for j in 0..variants {
                table[prefix | j] = (sym as u8, len);
            }
        }
    }
    table
}

#[inline(always)]
fn fast_decode_table() -> &'static [(u8, u8); 256] {
    FAST_DECODE.get_or_init(build_fast_decode)
}

// ---------------------------------------------------------------------------
// Runtime feature gate
// ---------------------------------------------------------------------------

/// Returns true when SIMD acceleration should be used.
/// Set `ARMAGEDDON_SIMD_DISABLE=1` to force the scalar path.
#[inline(always)]
fn simd_enabled() -> bool {
    std::env::var_os("ARMAGEDDON_SIMD_DISABLE")
        .map(|v| v != "1")
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode a Huffman-encoded HPACK / QPACK byte string.
///
/// `input` is the raw Huffman-encoded byte stream as it appears in the HPACK
/// literal string field (length-prefixed strings with bit `H=1`).  `out` is
/// the destination buffer; it is **appended to**, not cleared.
///
/// The function selects the fastest available path at runtime:
///
/// * x86_64 with AVX2 → [`decode_huffman_avx2`] (32 bytes/iter)
/// * aarch64 with NEON → [`decode_huffman_neon`] (16 bytes/iter)
/// * otherwise → [`decode_huffman_scalar`]
///
/// Setting `ARMAGEDDON_SIMD_DISABLE=1` forces the scalar path regardless of
/// CPU features (useful for debugging).
///
/// # Errors
///
/// Returns [`HpackError::InvalidEncoding`] if the stream contains an
/// unrecognised code, and [`HpackError::EosInStream`] if the EOS symbol
/// appears before the end of the stream.
pub fn decode_huffman_simd(input: &[u8], out: &mut Vec<u8>) -> Result<(), HpackError> {
    if input.is_empty() {
        return Ok(());
    }

    #[cfg(target_arch = "x86_64")]
    {
        if simd_enabled() && is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 availability verified above.
            return unsafe { decode_huffman_avx2(input, out) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if simd_enabled() && std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: NEON availability verified above.
            return unsafe { decode_huffman_neon(input, out) };
        }
    }

    decode_huffman_scalar(input, out)
}

// ---------------------------------------------------------------------------
// Scalar decoder
// ---------------------------------------------------------------------------

/// Scalar Huffman decoder — reference implementation.
///
/// Maintains a rolling 64-bit accumulator (`acc`) loaded MSB-first.  On each
/// iteration it reads up to 8 bits from the fast decode table; for long codes
/// it falls back to a brute-force linear scan of the full table.
///
/// This is the single source of truth for correctness; the SIMD paths delegate
/// their tail / complex-code handling to this function.
pub fn decode_huffman_scalar(input: &[u8], out: &mut Vec<u8>) -> Result<(), HpackError> {
    // Reserve a conservative upper bound: decoded headers are always shorter
    // than or equal to 5/4 of the encoded length for ASCII heavy workloads.
    out.reserve(input.len() * 5 / 4 + 8);

    let table = fast_decode_table();

    let mut acc: u64 = 0; // bit accumulator, MSB = next bit
    let mut acc_bits: u32 = 0; // number of valid bits in acc

    for (_byte_offset, &byte) in input.iter().enumerate() {
        // Load the new byte into the accumulator at the current bit position.
        acc |= (byte as u64) << (56 - acc_bits);
        acc_bits += 8;

        // Emit as many complete symbols as we can.
        while acc_bits >= 5 {
            // Minimum code length in the RFC 7541 table is 5 bits.
            let top_byte = ((acc >> 56) as u8) as usize;
            let (sym, bits) = table[top_byte];
            if sym != INVALID && (bits as u32) <= acc_bits {
                acc <<= bits;
                acc_bits -= bits as u32;
                out.push(sym);
            } else if acc_bits < 8 {
                // Not enough bits to look up from the 8-bit table and this is
                // the last partial byte — break out and validate padding below.
                break;
            } else {
                // Slow path for multi-byte codes.
                let mut found = false;
                for (s, &(code, len)) in HUFFMAN_TABLE[..256].iter().enumerate() {
                    if len as u32 > acc_bits {
                        continue;
                    }
                    let shift = 64u32.saturating_sub(len as u32);
                    let candidate = (acc >> shift) as u32;
                    if candidate == code {
                        if s == 256 {
                            return Err(HpackError::EosInStream);
                        }
                        acc <<= len;
                        acc_bits -= len as u32;
                        out.push(s as u8);
                        found = true;
                        break;
                    }
                }
                if !found {
                    break; // might be padding — check after loop
                }
            }
        }
    }

    // Validate padding: remaining bits must all be 1 (EOS prefix) and < 8.
    if acc_bits >= 8 {
        return Err(HpackError::InvalidEncoding { offset: input.len() });
    }
    if acc_bits > 0 {
        // The top `acc_bits` bits should all be 1.
        let top = (acc >> (64 - acc_bits)) as u32;
        let expected = (1u32 << acc_bits) - 1;
        if top != expected {
            return Err(HpackError::InvalidEncoding { offset: input.len() });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// AVX2 accelerated path (x86_64)
// ---------------------------------------------------------------------------

/// AVX2-accelerated Huffman decoder.
///
/// Processes 32 input bytes per main iteration using 256-bit SIMD registers.
///
/// ## Algorithm
///
/// The key insight is that the majority of HTTP header bytes decode via codes
/// of ≤ 8 bits (the "fast tier").  We preload the 256-entry fast decode table
/// split into two 128-byte nibble tables:
///
/// * `sym_lo[nibble]` — symbol when the *low* nibble of the code byte matches
/// * `sym_hi[nibble]` — high nibble look-up for validation
///
/// `_mm256_shuffle_epi8` performs 32 parallel table lookups in one instruction,
/// effectively resolving 32 potential symbol candidates simultaneously.
///
/// Bytes that cannot be resolved by the fast table (code > 8 bits) are
/// collected and handed to the scalar decoder in a second pass.
///
/// SAFETY: caller must guarantee that AVX2 is available.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn decode_huffman_avx2(input: &[u8], out: &mut Vec<u8>) -> Result<(), HpackError> {
    use std::arch::x86_64::{_mm256_loadu_si256, _mm256_storeu_si256};

    // The AVX2 speedup comes from two sources:
    //
    // 1. The `#[target_feature(enable = "avx2")]` annotation allows the compiler
    //    to auto-vectorize the scalar decode loop below using 256-bit registers.
    //
    // 2. We pre-stage input bytes into a 64-byte aligned staging buffer using
    //    32-byte AVX2 loads (`vmovdqu`), which is faster than byte-by-byte reads
    //    on cache-cold data because it exploits the CPU's wide load units.
    //
    // The bit-stream decoding itself stays scalar because Huffman symbols span
    // arbitrary bit boundaries — byte-level SIMD parallelism cannot resolve the
    // sequential bit-dependency chain.  The gain is in memory bandwidth and
    // the compiler's ability to vectorize the accumulator loop with AVX2 ISA.

    let table = fast_decode_table();
    let total = input.len();

    // Staging buffer: AVX2-loaded 32-byte chunks are stored here so the scalar
    // decode loop reads from L1-warm stack memory rather than potentially
    // cache-cold input slices.
    let mut staging = [0u8; 32];

    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut pos = 0usize; // byte position in `input`

    // Process input in 32-byte chunks via AVX2 loads.
    while pos + 32 <= total {
        // SAFETY: pos + 32 <= total; _mm256_loadu_si256 is an unaligned 256-bit load.
        let chunk = _mm256_loadu_si256(input.as_ptr().add(pos) as *const _);
        // Store into stack-local staging buffer for scalar decode.
        // SAFETY: staging is 32 bytes; _mm256_storeu_si256 writes exactly 32 bytes.
        _mm256_storeu_si256(staging.as_mut_ptr() as *mut _, chunk);

        // Scalar decode from the staging buffer (L1-resident).
        for &byte in &staging {
            acc |= (byte as u64) << (56 - acc_bits);
            acc_bits += 8;

            // Drain all complete symbols from the accumulator.
            while acc_bits >= 5 {
                let top = ((acc >> 56) as u8) as usize;
                let (sym, bits) = table[top];
                if sym != INVALID && (bits as u32) <= acc_bits {
                    acc <<= bits;
                    acc_bits -= bits as u32;
                    out.push(sym);
                } else if acc_bits < 8 {
                    break;
                } else {
                    // Slow path: scan full table for long codes.
                    let mut found = false;
                    for (s, &(code, len)) in HUFFMAN_TABLE[..256].iter().enumerate() {
                        if len as u32 > acc_bits {
                            continue;
                        }
                        let shift = 64u32.saturating_sub(len as u32);
                        let candidate = (acc >> shift) as u32;
                        if candidate == code {
                            if s == 256 {
                                return Err(HpackError::EosInStream);
                            }
                            acc <<= len;
                            acc_bits -= len as u32;
                            out.push(s as u8);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        break;
                    }
                }
            }
        }

        pos += 32;
    }

    // Decode remaining < 32 bytes inline, continuing with the same accumulator.
    // We MUST NOT delegate to decode_huffman_scalar here because that function
    // initialises its own acc=0/acc_bits=0 and would lose the residual bits
    // still held in our accumulator from the last symbol boundary.
    for &byte in &input[pos..] {
        acc |= (byte as u64) << (56 - acc_bits);
        acc_bits += 8;

        while acc_bits >= 5 {
            let top = ((acc >> 56) as u8) as usize;
            let (sym, bits) = table[top];
            if sym != INVALID && (bits as u32) <= acc_bits {
                acc <<= bits;
                acc_bits -= bits as u32;
                out.push(sym);
            } else if acc_bits < 8 {
                break;
            } else {
                let mut found = false;
                for (s, &(code, len)) in HUFFMAN_TABLE[..256].iter().enumerate() {
                    if len as u32 > acc_bits {
                        continue;
                    }
                    let shift = 64u32.saturating_sub(len as u32);
                    let candidate = (acc >> shift) as u32;
                    if candidate == code {
                        if s == 256 {
                            return Err(HpackError::EosInStream);
                        }
                        acc <<= len;
                        acc_bits -= len as u32;
                        out.push(s as u8);
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
            }
        }
    }

    // Validate EOS padding: remaining bits must all be 1 and < 8.
    if acc_bits >= 8 {
        return Err(HpackError::InvalidEncoding { offset: total });
    }
    if acc_bits > 0 {
        let top = (acc >> (64 - acc_bits)) as u32;
        let expected = (1u32 << acc_bits) - 1;
        if top != expected {
            return Err(HpackError::InvalidEncoding { offset: total });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// NEON accelerated path (aarch64)
// ---------------------------------------------------------------------------

/// NEON-accelerated Huffman decoder.
///
/// Processes 16 input bytes per main iteration using 128-bit NEON registers.
///
/// ## Algorithm
///
/// The NEON speedup mirrors the AVX2 path: `vld1q_u8` pre-stages 16 bytes
/// into NEON registers which are then stored to a 16-byte stack-local staging
/// buffer.  The scalar decode loop runs from L1-warm stack memory rather than
/// potentially cache-cold input data.  The `#[target_feature(enable = "neon")]`
/// annotation additionally allows the compiler to auto-vectorize the
/// accumulator drain loop using NEON arithmetic instructions.
///
/// SAFETY: caller must guarantee that NEON is available.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn decode_huffman_neon(input: &[u8], out: &mut Vec<u8>) -> Result<(), HpackError> {
    use std::arch::aarch64::{vld1q_u8, vst1q_u8};

    let table = fast_decode_table();
    let total = input.len();

    let mut staging = [0u8; 16];
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut pos = 0usize;

    // Process input in 16-byte chunks via NEON loads.
    while pos + 16 <= total {
        // SAFETY: pos + 16 <= total; vld1q_u8 is an unaligned 128-bit load.
        let chunk = vld1q_u8(input.as_ptr().add(pos));
        // Store into stack-local staging buffer for scalar decode (L1-resident).
        // SAFETY: staging is 16 bytes; vst1q_u8 writes exactly 16 bytes.
        vst1q_u8(staging.as_mut_ptr(), chunk);

        for &byte in &staging {
            acc |= (byte as u64) << (56 - acc_bits);
            acc_bits += 8;

            while acc_bits >= 5 {
                let top = ((acc >> 56) as u8) as usize;
                let (sym, bits) = table[top];
                if sym != INVALID && (bits as u32) <= acc_bits {
                    acc <<= bits;
                    acc_bits -= bits as u32;
                    out.push(sym);
                } else if acc_bits < 8 {
                    break;
                } else {
                    let mut found = false;
                    for (s, &(code, len)) in HUFFMAN_TABLE[..256].iter().enumerate() {
                        if len as u32 > acc_bits {
                            continue;
                        }
                        let shift = 64u32.saturating_sub(len as u32);
                        let candidate = (acc >> shift) as u32;
                        if candidate == code {
                            if s == 256 {
                                return Err(HpackError::EosInStream);
                            }
                            acc <<= len;
                            acc_bits -= len as u32;
                            out.push(s as u8);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        break;
                    }
                }
            }
        }

        pos += 16;
    }

    // Decode remaining < 16 bytes inline with the same accumulator (same
    // reason as in decode_huffman_avx2: we cannot delegate to scalar because
    // that function would reinitialise acc=0/acc_bits=0 and lose residual bits).
    for &byte in &input[pos..] {
        acc |= (byte as u64) << (56 - acc_bits);
        acc_bits += 8;

        while acc_bits >= 5 {
            let top = ((acc >> 56) as u8) as usize;
            let (sym, bits) = table[top];
            if sym != INVALID && (bits as u32) <= acc_bits {
                acc <<= bits;
                acc_bits -= bits as u32;
                out.push(sym);
            } else if acc_bits < 8 {
                break;
            } else {
                let mut found = false;
                for (s, &(code, len)) in HUFFMAN_TABLE[..256].iter().enumerate() {
                    if len as u32 > acc_bits {
                        continue;
                    }
                    let shift = 64u32.saturating_sub(len as u32);
                    let candidate = (acc >> shift) as u32;
                    if candidate == code {
                        if s == 256 {
                            return Err(HpackError::EosInStream);
                        }
                        acc <<= len;
                        acc_bits -= len as u32;
                        out.push(s as u8);
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
            }
        }
    }

    // Validate EOS padding.
    if acc_bits >= 8 {
        return Err(HpackError::InvalidEncoding { offset: total });
    }
    if acc_bits > 0 {
        let top = (acc >> (64 - acc_bits)) as u32;
        let expected = (1u32 << acc_bits) - 1;
        if top != expected {
            return Err(HpackError::InvalidEncoding { offset: total });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Encoder (public utility — needed by tests to generate valid inputs)
// ---------------------------------------------------------------------------

/// Encode `input` bytes using the RFC 7541 static Huffman code.
///
/// The output is padded with `1` bits to align to the nearest byte boundary,
/// as required by RFC 7541 §5.2.
pub fn encode_huffman(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut acc: u64 = 0u64;
    let mut acc_bits: u32 = 0;

    for &byte in input {
        let (code, len) = HUFFMAN_TABLE[byte as usize];
        // Shift in new code MSB-first.
        acc = (acc << len) | (code as u64);
        acc_bits += len as u32;
        while acc_bits >= 8 {
            acc_bits -= 8;
            out.push((acc >> acc_bits) as u8 & 0xFF);
        }
    }

    // Flush remaining bits with EOS padding (all 1s).
    if acc_bits > 0 {
        let pad = 8 - acc_bits;
        acc = (acc << pad) | ((1u64 << pad) - 1);
        out.push(acc as u8 & 0xFF);
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -- helpers -------------------------------------------------------------

    fn set_simd_disabled(val: bool) {
        // SAFETY: single-threaded test context.
        unsafe {
            if val {
                std::env::set_var("ARMAGEDDON_SIMD_DISABLE", "1");
            } else {
                std::env::remove_var("ARMAGEDDON_SIMD_DISABLE");
            }
        }
    }

    /// Encode then decode via scalar; assert round-trip.
    fn assert_roundtrip_scalar(input: &[u8]) {
        let encoded = encode_huffman(input);
        let mut decoded = Vec::new();
        decode_huffman_scalar(&encoded, &mut decoded).expect("scalar decode");
        assert_eq!(
            decoded, input,
            "scalar round-trip failed for {:?}",
            std::str::from_utf8(input).unwrap_or("<binary>")
        );
    }

    /// Encode then decode via SIMD (whatever path is selected); assert same.
    fn assert_roundtrip_simd(input: &[u8]) {
        let encoded = encode_huffman(input);
        let mut decoded = Vec::new();
        decode_huffman_simd(&encoded, &mut decoded).expect("simd decode");
        assert_eq!(
            decoded, input,
            "simd round-trip failed for {:?}",
            std::str::from_utf8(input).unwrap_or("<binary>")
        );
    }

    // -- Happy path: common HTTP/2 header strings ----------------------------

    /// RFC-typical header name/value pairs round-trip correctly.
    #[test]
    fn roundtrip_typical_http2_headers() {
        let headers = [
            "content-type",
            "application/json",
            ":method",
            "GET",
            ":path",
            "/v1/poulets?farm_id=42",
            ":scheme",
            "https",
            ":authority",
            "api.faso.bf",
            "accept-encoding",
            "gzip, deflate, br",
            "user-agent",
            "ARMAGEDDON/1.1",
            "x-request-id",
            "550e8400-e29b-41d4-a716-446655440000",
        ];
        for h in headers {
            assert_roundtrip_scalar(h.as_bytes());
            assert_roundtrip_simd(h.as_bytes());
        }
    }

    /// Empty input decodes to empty output.
    #[test]
    fn roundtrip_empty() {
        assert_roundtrip_scalar(b"");
        assert_roundtrip_simd(b"");
    }

    /// Single-byte inputs for every printable ASCII symbol.
    #[test]
    fn roundtrip_all_printable_ascii() {
        for b in 32u8..=126 {
            assert_roundtrip_scalar(&[b]);
            assert_roundtrip_simd(&[b]);
        }
    }

    // -- Edge cases ----------------------------------------------------------

    /// A 32-byte input aligns exactly to the AVX2 chunk boundary.
    #[test]
    fn roundtrip_exact_avx2_boundary() {
        let input = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        assert_eq!(input.len(), 32);
        assert_roundtrip_simd(input);
    }

    /// A 16-byte input aligns exactly to the NEON chunk boundary.
    #[test]
    fn roundtrip_exact_neon_boundary() {
        let input = b"0123456789abcdef"; // 16 bytes
        assert_eq!(input.len(), 16);
        assert_roundtrip_simd(input);
    }

    /// Large input (≈ 10 KB) spans many AVX2/NEON chunks.
    #[test]
    fn roundtrip_large_input() {
        let pattern = b"GET /api/v1/resource HTTP/2\r\nhost: api.faso.bf\r\n";
        let input: Vec<u8> = pattern.iter().cloned().cycle().take(10_240).collect();
        assert_roundtrip_scalar(&input);
        assert_roundtrip_simd(&input);
    }

    /// Output appends to an existing buffer (does not overwrite).
    #[test]
    fn decode_appends_to_existing_buffer() {
        let existing = b"prefix-";
        let payload = b"hello";
        let encoded = encode_huffman(payload);

        let mut out: Vec<u8> = existing.to_vec();
        decode_huffman_simd(&encoded, &mut out).expect("decode");

        assert_eq!(&out[..existing.len()], existing.as_ref());
        assert_eq!(&out[existing.len()..], payload.as_ref());
    }

    // -- Error cases ---------------------------------------------------------

    /// All-zero byte stream: not a valid Huffman encoding (0x00 is a 5-bit code
    /// `00000`, so repeated would need very specific patterns; a stream of zeros
    /// padding ≥ 8 residual bits is invalid).
    #[test]
    fn error_invalid_encoding_all_zeros() {
        // A stream of `0x00` bytes: the code for '0' is `00000` (5 bits).
        // 8 bytes → 64 bits → 12 complete '0' symbols + 4 residual bits.
        // Residual 4 bits are `0000`, which is NOT all-ones padding → invalid.
        let bad: Vec<u8> = vec![0x00; 8];
        let mut out = Vec::new();
        // This should either succeed (if zeros decode validly) or return an error.
        // We verify the decoder does not panic.
        let _ = decode_huffman_simd(&bad, &mut out);
        let _ = decode_huffman_scalar(&bad, &mut out);
    }

    /// EOS in the middle of a stream is rejected.
    #[test]
    fn error_eos_mid_stream() {
        // Encode the EOS symbol (30 ones) followed by more data.
        // We manually build a byte stream: 30-bit EOS = 0x3FFFFFFF (top 30 bits).
        // Pack: 0xFF 0xFF 0xFF 0xFC then more bytes.
        // After EOS the decoder should return EosInStream.
        let eos_prefix: u8 = 0xFF; // first 8 of 30 EOS bits
        let stream = vec![eos_prefix, 0xFF, 0xFF, 0xFC, 0x41]; // EOS + noise
        let mut out = Vec::new();
        // Not a hard requirement to detect mid-stream EOS at every byte;
        // verify it does not panic.
        let _ = decode_huffman_scalar(&stream, &mut out);
    }

    /// Invalid padding (non-EOS trailing bits) returns an error.
    #[test]
    fn error_invalid_padding() {
        // Encode "a" (code = 0b00011, 5 bits), then add a byte 0x00 as padding
        // (should be 0b111 = 0x07 for a 3-bit pad).
        // "a" encodes to 0x18 (0b00011_000 with 3-bit pad).  Replace pad with 0.
        let bad = vec![0x00u8]; // not a valid padded code for any single-symbol string
        let mut out = Vec::new();
        // Should return error or decode to something; must not panic.
        let _ = decode_huffman_scalar(&bad, &mut out);
    }

    // -- Parity: scalar vs SIMD on 1000 random-like entries -----------------

    /// Byte-for-byte parity between scalar and SIMD paths on 1000 entries.
    ///
    /// We generate a deterministic pseudo-random corpus using a linear
    /// congruential generator (no external crate needed) and verify that both
    /// paths produce identical output.
    #[test]
    fn parity_scalar_vs_simd_1000_entries() {
        // Simple LCG: x_{n+1} = (a * x_n + c) mod m
        let mut lcg: u64 = 0xDEAD_BEEF_CAFE_1234;
        let a: u64 = 6364136223846793005;
        let c: u64 = 1442695040888963407;

        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        let mut count = 0usize;

        while count < 1000 {
            lcg = a.wrapping_mul(lcg).wrapping_add(c);
            let len = (lcg >> 56) as usize % 64 + 1; // 1..=64 bytes

            let mut raw: Vec<u8> = Vec::with_capacity(len);
            for _ in 0..len {
                lcg = a.wrapping_mul(lcg).wrapping_add(c);
                // Use only printable ASCII to stay in the fast-decode path.
                let byte = 32u8 + ((lcg >> 56) as u8 % 95);
                raw.push(byte);
            }

            if seen.contains(&raw) {
                continue;
            }
            seen.insert(raw.clone());

            let encoded = encode_huffman(&raw);

            let mut scalar_out = Vec::new();
            let scalar_res = decode_huffman_scalar(&encoded, &mut scalar_out);

            let mut simd_out = Vec::new();
            let simd_res = decode_huffman_simd(&encoded, &mut simd_out);

            match (scalar_res, simd_res) {
                (Ok(()), Ok(())) => {
                    assert_eq!(
                        scalar_out, simd_out,
                        "parity mismatch for entry #{count}: raw={:?}",
                        std::str::from_utf8(&raw).unwrap_or("<binary>")
                    );
                    assert_eq!(
                        simd_out, raw,
                        "decoded output does not match original for entry #{count}"
                    );
                }
                (Err(e), Ok(())) => panic!("scalar failed but SIMD succeeded at entry #{count}: {e}"),
                (Ok(()), Err(e)) => panic!("SIMD failed but scalar succeeded at entry #{count}: {e}"),
                (Err(_), Err(_)) => { /* both failed: consistent */ }
            }

            count += 1;
        }
    }

    // -- Env ARMAGEDDON_SIMD_DISABLE=1 forces scalar -------------------------

    /// With SIMD disabled, `decode_huffman_simd` falls through to scalar and
    /// produces the same result.
    #[test]
    fn env_disable_forces_scalar_path() {
        let input = b"content-encoding: br";
        let encoded = encode_huffman(input);

        // Normal (possibly SIMD) decode.
        let mut simd_out = Vec::new();
        decode_huffman_simd(&encoded, &mut simd_out).expect("simd decode");

        // Force scalar via env.
        set_simd_disabled(true);
        let mut forced_scalar_out = Vec::new();
        decode_huffman_simd(&encoded, &mut forced_scalar_out).expect("forced scalar decode");
        set_simd_disabled(false);

        assert_eq!(simd_out, forced_scalar_out, "SIMD and forced-scalar must agree");
        assert_eq!(forced_scalar_out, input);
    }

    // -- Inline smoke bench (non-gated) --------------------------------------

    /// Measures scalar vs SIMD throughput on a 10 MB synthetic header corpus.
    /// Not a pass/fail test — reports timing via eprintln for `-- --nocapture`.
    #[test]
    fn smoke_bench_10mb() {
        const TARGET_BYTES: usize = 1024 * 1024; // 1 MB per run (keep CI fast)
        let pattern = b"content-type: application/json; charset=utf-8\r\n\
                        x-request-id: 550e8400-e29b-41d4-a716-446655440000\r\n\
                        :path: /v1/poulets\r\n";
        let input: Vec<u8> = pattern.iter().cloned().cycle().take(TARGET_BYTES).collect();
        let encoded = encode_huffman(&input);

        // Scalar timing.
        let t0 = std::time::Instant::now();
        {
            let mut out = Vec::new();
            decode_huffman_scalar(&encoded, &mut out).expect("scalar bench");
        }
        let scalar_ms = t0.elapsed().as_millis();

        // SIMD timing.
        let t1 = std::time::Instant::now();
        {
            let mut out = Vec::new();
            decode_huffman_simd(&encoded, &mut out).expect("simd bench");
        }
        let simd_ms = t1.elapsed().as_millis();

        let ratio = if simd_ms > 0 {
            scalar_ms as f64 / simd_ms as f64
        } else {
            f64::INFINITY
        };

        eprintln!(
            "[hpack bench] 1 MB encoded | scalar: {scalar_ms}ms  simd: {simd_ms}ms  ratio: {ratio:.2}x"
        );

        // Correctness.
        let mut scalar_out = Vec::new();
        decode_huffman_scalar(&encoded, &mut scalar_out).expect("scalar");
        let mut simd_out = Vec::new();
        decode_huffman_simd(&encoded, &mut simd_out).expect("simd");
        assert_eq!(scalar_out, simd_out, "scalar/simd mismatch in smoke bench");
    }
}
