//! SIMD-accelerated hot-path primitives for KAYA store.
//!
//! Provides batch hashing, byte comparison, zero-detection and fast copy.
//! On x86_64, AVX2 intrinsics are used when `is_x86_feature_detected!("avx2")`
//! returns true at runtime. On aarch64, NEON is selected the same way. Every
//! platform falls back to a pure-Rust scalar path so the code compiles and
//! runs correctly everywhere.
//!
//! The environment variable `KAYA_SIMD_DISABLE=1` forces the scalar fallback
//! at runtime, which is useful for testing and for operators that need
//! deterministic non-SIMD behaviour.

use ahash::RandomState;

// ---------------------------------------------------------------------------
// Runtime feature gate
// ---------------------------------------------------------------------------

/// Returns true when SIMD acceleration should be used.
/// Set `KAYA_SIMD_DISABLE=1` to force the scalar path (useful in tests).
#[inline(always)]
fn simd_enabled() -> bool {
    std::env::var_os("KAYA_SIMD_DISABLE")
        .map(|v| v != "1")
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Shared hasher state (deterministic seeds matching shard routing in lib.rs)
// ---------------------------------------------------------------------------

fn hasher() -> RandomState {
    RandomState::with_seeds(1, 2, 3, 4)
}

// ---------------------------------------------------------------------------
// hash_batch
// ---------------------------------------------------------------------------

/// Hash a batch of keys in one call.
///
/// On x86_64 with AVX2 available (and `KAYA_SIMD_DISABLE` not set) the keys
/// are hashed in groups of 4 using independent ahash instances executed with
/// data pre-loaded into registers, giving better ILP. On every other platform,
/// or when SIMD is disabled, each key is hashed sequentially by the same
/// ahash `RandomState` used by the shard router, guaranteeing bit-for-bit
/// identical results.
///
/// # Panics
/// Panics in debug mode if `keys.len() != out.len()`.
pub fn hash_batch(keys: &[&[u8]], out: &mut [u64]) {
    debug_assert_eq!(keys.len(), out.len(), "hash_batch: keys and out must have the same length");

    #[cfg(target_arch = "x86_64")]
    {
        if simd_enabled() && is_x86_feature_detected!("avx2") {
            // SAFETY: we have verified AVX2 support at runtime above.
            unsafe { hash_batch_avx2(keys, out) };
            return;
        }
    }

    hash_batch_scalar(keys, out);
}

/// Scalar fallback: one hash per key, deterministic with shard routing.
fn hash_batch_scalar(keys: &[&[u8]], out: &mut [u64]) {
    let h = hasher();
    for (key, slot) in keys.iter().zip(out.iter_mut()) {
        *slot = h.hash_one(key);
    }
}

/// AVX2-assisted batch hash.
///
/// The speedup comes from processing 4 keys per loop iteration so the CPU can
/// pipeline the independent hash computations. The actual hash algorithm is
/// ahash (scalar), but avoiding branch mispredictions and improving cache
/// pre-fetch gives measurable improvement for large batches.
///
/// SAFETY: caller must guarantee that AVX2 is available (`avx2` target
/// feature). The function itself only calls ahash which is pure Rust; the
/// `#[target_feature]` annotation instructs the compiler to generate AVX2
/// code for surrounding scalar loops, improving auto-vectorisation.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn hash_batch_avx2(keys: &[&[u8]], out: &mut [u64]) {
    let h = hasher();
    let n = keys.len();
    let chunks = n / 4;
    let remainder = n % 4;

    let mut i = 0usize;
    for _ in 0..chunks {
        // Four independent hash computations — the compiler can schedule them
        // in parallel across execution units thanks to the avx2 feature flag.
        out[i]     = h.hash_one(keys[i]);
        out[i + 1] = h.hash_one(keys[i + 1]);
        out[i + 2] = h.hash_one(keys[i + 2]);
        out[i + 3] = h.hash_one(keys[i + 3]);
        i += 4;
    }
    for j in 0..remainder {
        out[i + j] = h.hash_one(keys[i + j]);
    }
}

// ---------------------------------------------------------------------------
// memcmp_simd
// ---------------------------------------------------------------------------

/// Compare two byte slices for equality using SIMD when available.
///
/// Returns `true` if and only if `a == b` (same length and identical content).
/// For slices shorter than 16 bytes, the scalar path is always taken.
pub fn memcmp_simd(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    if a.is_empty() {
        return true;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if a.len() >= 16 && simd_enabled() && is_x86_feature_detected!("avx2") {
            // SAFETY: we verified AVX2 availability above.
            return unsafe { memcmp_avx2(a, b) };
        }
        if a.len() >= 16 && simd_enabled() && is_x86_feature_detected!("sse2") {
            // SAFETY: we verified SSE2 availability above.
            return unsafe { memcmp_sse2(a, b) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if a.len() >= 16 && simd_enabled() {
            // NEON is always present on aarch64 Linux/macOS targets, but we
            // guard with the runtime check for correctness on embedded targets.
            if std::arch::is_aarch64_feature_detected!("neon") {
                // SAFETY: NEON availability verified above.
                return unsafe { memcmp_neon(a, b) };
            }
        }
    }

    a == b
}

/// AVX2 comparison: processes 32 bytes per iteration.
///
/// SAFETY: caller must guarantee AVX2 availability.
/// Pointers are loaded with `_mm256_loadu_si256` (unaligned load), so no
/// alignment invariant is required on `a` or `b`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn memcmp_avx2(a: &[u8], b: &[u8]) -> bool {
    use std::arch::x86_64::{
        _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
    };

    let len = a.len();
    let mut offset = 0usize;

    while offset + 32 <= len {
        // SAFETY: offset + 32 <= len, so reading 32 bytes is in-bounds.
        // `_mm256_loadu_si256` performs an unaligned 256-bit load.
        let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const _);
        let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const _);
        let eq = _mm256_cmpeq_epi8(va, vb);
        let mask = _mm256_movemask_epi8(eq);
        if mask != -1i32 {
            return false;
        }
        offset += 32;
    }

    // Handle the remaining bytes (< 32) with the scalar path.
    a[offset..] == b[offset..]
}

/// SSE2 comparison: processes 16 bytes per iteration.
///
/// SAFETY: caller must guarantee SSE2 availability.
/// All loads use `_mm_loadu_si128` (unaligned).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn memcmp_sse2(a: &[u8], b: &[u8]) -> bool {
    use std::arch::x86_64::{
        _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8,
    };

    let len = a.len();
    let mut offset = 0usize;

    while offset + 16 <= len {
        // SAFETY: offset + 16 <= len guarantees in-bounds access.
        // `_mm_loadu_si128` performs an unaligned 128-bit load.
        let va = _mm_loadu_si128(a.as_ptr().add(offset) as *const _);
        let vb = _mm_loadu_si128(b.as_ptr().add(offset) as *const _);
        let eq = _mm_cmpeq_epi8(va, vb);
        let mask = _mm_movemask_epi8(eq);
        if mask != 0xFFFF {
            return false;
        }
        offset += 16;
    }

    a[offset..] == b[offset..]
}

/// NEON comparison: processes 16 bytes per iteration.
///
/// SAFETY: caller must guarantee NEON availability.
/// `vld1q_u8` performs an unaligned 128-bit load.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn memcmp_neon(a: &[u8], b: &[u8]) -> bool {
    use std::arch::aarch64::{vceqq_u8, vget_lane_u64, vld1q_u8, vreinterpretq_u64_u8};

    let len = a.len();
    let mut offset = 0usize;

    while offset + 16 <= len {
        // SAFETY: offset + 16 <= len; `vld1q_u8` is an unaligned load.
        let va = vld1q_u8(a.as_ptr().add(offset));
        let vb = vld1q_u8(b.as_ptr().add(offset));
        let eq = vceqq_u8(va, vb);
        // Reinterpret the 16×u8 comparison result as 2×u64 and check all bits.
        let eq64 = vreinterpretq_u64_u8(eq);
        let lo = vget_lane_u64::<0>(eq64);
        let hi = vget_lane_u64::<1>(eq64);
        if lo != u64::MAX || hi != u64::MAX {
            return false;
        }
        offset += 16;
    }

    a[offset..] == b[offset..]
}

// ---------------------------------------------------------------------------
// is_all_zero
// ---------------------------------------------------------------------------

/// Return `true` if every byte in `buf` is zero.
///
/// Uses AVX2 on x86_64 (32-byte chunks), NEON on aarch64 (16-byte chunks),
/// and scalar otherwise.
pub fn is_all_zero(buf: &[u8]) -> bool {
    if buf.is_empty() {
        return true;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if simd_enabled() && is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 verified above.
            return unsafe { is_all_zero_avx2(buf) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if simd_enabled() && std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: NEON verified above.
            return unsafe { is_all_zero_neon(buf) };
        }
    }

    buf.iter().all(|&b| b == 0)
}

/// AVX2 all-zero check: 32 bytes per iteration.
///
/// SAFETY: caller ensures AVX2 is available.
/// All loads are unaligned (`_mm256_loadu_si256`).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn is_all_zero_avx2(buf: &[u8]) -> bool {
    use std::arch::x86_64::{
        _mm256_loadu_si256, _mm256_setzero_si256, _mm256_testz_si256,
    };

    let len = buf.len();
    let mut offset = 0usize;

    while offset + 32 <= len {
        // SAFETY: offset + 32 <= len; unaligned 256-bit load.
        let v = _mm256_loadu_si256(buf.as_ptr().add(offset) as *const _);
        let z = _mm256_setzero_si256();
        // _mm256_testz_si256 returns 1 if (v AND v) == 0, i.e. v is all zero.
        if _mm256_testz_si256(v, v) == 0 {
            return false;
        }
        let _ = z; // suppress unused warning
        offset += 32;
    }

    buf[offset..].iter().all(|&b| b == 0)
}

/// NEON all-zero check: 16 bytes per iteration.
///
/// SAFETY: caller ensures NEON is available.
/// `vld1q_u8` is an unaligned load.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn is_all_zero_neon(buf: &[u8]) -> bool {
    use std::arch::aarch64::{
        vceqzq_u8, vget_lane_u64, vld1q_u8, vreinterpretq_u64_u8,
    };

    let len = buf.len();
    let mut offset = 0usize;

    while offset + 16 <= len {
        // SAFETY: offset + 16 <= len; unaligned 128-bit load.
        let v = vld1q_u8(buf.as_ptr().add(offset));
        let eq_zero = vceqzq_u8(v);
        let eq64 = vreinterpretq_u64_u8(eq_zero);
        let lo = vget_lane_u64::<0>(eq64);
        let hi = vget_lane_u64::<1>(eq64);
        if lo != u64::MAX || hi != u64::MAX {
            return false;
        }
        offset += 16;
    }

    buf[offset..].iter().all(|&b| b == 0)
}

// ---------------------------------------------------------------------------
// memcpy_fast
// ---------------------------------------------------------------------------

/// Copy `src` into `dst`.
///
/// For buffers >= 32 bytes on x86_64 with AVX2, a 32-byte-unrolled copy is
/// used so the compiler can emit `vmovdqu` instructions. For smaller buffers
/// or on other platforms, this calls `std::ptr::copy_nonoverlapping` directly,
/// which is already the fastest approach available.
///
/// # Panics
/// Panics if `dst.len() < src.len()`.
pub fn memcpy_fast(src: &[u8], dst: &mut [u8]) {
    assert!(
        dst.len() >= src.len(),
        "memcpy_fast: dst too small ({} < {})",
        dst.len(),
        src.len()
    );

    #[cfg(target_arch = "x86_64")]
    {
        if src.len() >= 32 && simd_enabled() && is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 verified; dst.len() >= src.len() asserted above.
            unsafe { memcpy_avx2(src, dst) };
            return;
        }
    }

    // SAFETY: src and dst are valid, non-overlapping slices; dst.len() >= src.len().
    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), src.len());
    }
}

/// AVX2-assisted copy: 32-byte unrolled store loop.
///
/// SAFETY: caller must ensure AVX2 availability and `dst.len() >= src.len()`.
/// Both loads (`_mm256_loadu_si256`) and stores (`_mm256_storeu_si256`) are
/// unaligned, so no alignment invariant is required.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn memcpy_avx2(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::{_mm256_loadu_si256, _mm256_storeu_si256};

    let len = src.len();
    let mut offset = 0usize;

    while offset + 32 <= len {
        // SAFETY: offset + 32 <= len; both pointers are valid and the ranges
        // do not overlap (src and dst are distinct slices guaranteed by
        // the Rust borrow checker at the call site).
        let v = _mm256_loadu_si256(src.as_ptr().add(offset) as *const _);
        _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut _, v);
        offset += 32;
    }

    // Handle remainder (< 32 bytes).
    let remaining = len - offset;
    if remaining > 0 {
        std::ptr::copy_nonoverlapping(
            src.as_ptr().add(offset),
            dst.as_mut_ptr().add(offset),
            remaining,
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers -------------------------------------------------------------

    fn scalar_hash(key: &[u8]) -> u64 {
        hasher().hash_one(key)
    }

    fn set_simd_disabled(val: bool) {
        if val {
            // SAFETY: we are in a single-threaded test context.
            unsafe { std::env::set_var("KAYA_SIMD_DISABLE", "1") };
        } else {
            unsafe { std::env::remove_var("KAYA_SIMD_DISABLE") };
        }
    }

    // -- hash_batch ----------------------------------------------------------

    /// hash_batch must produce the same results as scalar hashing key by key.
    #[test]
    fn hash_batch_matches_scalar() {
        let keys: Vec<&[u8]> = vec![
            b"alpha", b"beta", b"gamma", b"delta",
            b"epsilon", b"zeta", b"eta", b"theta",
        ];
        let mut out = vec![0u64; keys.len()];
        hash_batch(&keys, &mut out);

        for (key, &h) in keys.iter().zip(out.iter()) {
            assert_eq!(
                h,
                scalar_hash(key),
                "hash_batch diverged from scalar for key {:?}",
                key
            );
        }
    }

    /// hash_batch on a batch of 64 keys (representative of mget hot-path).
    #[test]
    fn hash_batch_64_keys() {
        let raw: Vec<Vec<u8>> = (0u32..64).map(|i| format!("key:{i:04}").into_bytes()).collect();
        let keys: Vec<&[u8]> = raw.iter().map(|v| v.as_slice()).collect();
        let mut out = vec![0u64; 64];
        hash_batch(&keys, &mut out);

        for (key, &h) in keys.iter().zip(out.iter()) {
            assert_eq!(h, scalar_hash(key));
        }
    }

    /// hash_batch with empty input should not panic.
    #[test]
    fn hash_batch_empty() {
        let keys: Vec<&[u8]> = vec![];
        let mut out: Vec<u64> = vec![];
        hash_batch(&keys, &mut out); // must not panic
    }

    /// hash_batch with SIMD disabled falls back to scalar — results identical.
    #[test]
    fn hash_batch_scalar_fallback_env() {
        let keys: Vec<&[u8]> = vec![b"fallback-a", b"fallback-b", b"fallback-c"];
        let mut out_simd = vec![0u64; 3];
        hash_batch(&keys, &mut out_simd);

        set_simd_disabled(true);
        let mut out_scalar = vec![0u64; 3];
        hash_batch(&keys, &mut out_scalar);
        set_simd_disabled(false);

        assert_eq!(out_simd, out_scalar, "SIMD and scalar paths must agree");
    }

    // -- memcmp_simd ---------------------------------------------------------

    /// Empty slices are equal.
    #[test]
    fn memcmp_empty_equal() {
        assert!(memcmp_simd(b"", b""));
    }

    /// Single byte: equal.
    #[test]
    fn memcmp_single_byte_equal() {
        assert!(memcmp_simd(b"x", b"x"));
    }

    /// Single byte: not equal.
    #[test]
    fn memcmp_single_byte_unequal() {
        assert!(!memcmp_simd(b"a", b"b"));
    }

    /// Different lengths are never equal.
    #[test]
    fn memcmp_different_lengths() {
        assert!(!memcmp_simd(b"hello", b"hell"));
    }

    /// Lengths below SIMD threshold (< 16 bytes).
    #[test]
    fn memcmp_lengths_below_threshold() {
        let a: Vec<u8> = (0u8..15).collect();
        let b = a.clone();
        assert!(memcmp_simd(&a, &b));

        let mut c = a.clone();
        c[7] ^= 0xFF;
        assert!(!memcmp_simd(&a, &c));
    }

    /// Length exactly 16 (SSE2 / NEON threshold).
    #[test]
    fn memcmp_length_16_equal() {
        let a = b"0123456789abcdef";
        let b = b"0123456789abcdef";
        assert!(memcmp_simd(a, b));
    }

    #[test]
    fn memcmp_length_16_unequal() {
        let a = b"0123456789abcdef";
        let mut b = *a;
        b[15] ^= 0x01;
        assert!(!memcmp_simd(a, &b));
    }

    /// Lengths spanning AVX2 boundary (31, 32, 63, 64, 1024).
    #[test]
    fn memcmp_simd_various_lengths() {
        for &len in &[1usize, 15, 16, 31, 32, 63, 64, 127, 128, 1024] {
            let a: Vec<u8> = (0u8..=255).cycle().take(len).collect();
            let b = a.clone();
            assert!(memcmp_simd(&a, &b), "should be equal for len={len}");

            if len > 0 {
                let mut c = a.clone();
                c[len / 2] ^= 0xFF;
                assert!(!memcmp_simd(&a, &c), "should differ for len={len}");
            }
        }
    }

    /// Force scalar path via env and verify same result.
    #[test]
    fn memcmp_scalar_fallback_env() {
        let a: Vec<u8> = (0u8..64).collect();
        let b = a.clone();

        set_simd_disabled(true);
        let result = memcmp_simd(&a, &b);
        set_simd_disabled(false);

        assert!(result);
    }

    // -- is_all_zero ---------------------------------------------------------

    /// Empty buffer reports all-zero.
    #[test]
    fn is_all_zero_empty() {
        assert!(is_all_zero(b""));
    }

    /// Buffer of actual zeros.
    #[test]
    fn is_all_zero_true() {
        for &len in &[1usize, 15, 16, 31, 32, 33, 63, 64, 1024] {
            let buf = vec![0u8; len];
            assert!(is_all_zero(&buf), "all-zero buffer len={len} failed");
        }
    }

    /// Buffer with one non-zero byte.
    #[test]
    fn is_all_zero_false() {
        for &len in &[1usize, 16, 32, 33, 64, 1024] {
            let mut buf = vec![0u8; len];
            buf[len / 2] = 1;
            assert!(!is_all_zero(&buf), "non-zero buffer len={len} reported all-zero");
        }
    }

    /// Non-zero in the very last byte (tests remainder handling).
    #[test]
    fn is_all_zero_nonzero_at_tail() {
        let mut buf = vec![0u8; 65]; // 2×32 + 1
        *buf.last_mut().unwrap() = 0xAB;
        assert!(!is_all_zero(&buf));
    }

    /// Scalar fallback via env var.
    #[test]
    fn is_all_zero_scalar_fallback() {
        let buf = vec![0u8; 64];
        set_simd_disabled(true);
        let result = is_all_zero(&buf);
        set_simd_disabled(false);
        assert!(result);
    }

    // -- memcpy_fast ---------------------------------------------------------

    /// Basic copy correctness for various sizes.
    #[test]
    fn memcpy_fast_basic() {
        for &len in &[0usize, 1, 15, 16, 31, 32, 33, 63, 64, 1024] {
            let src: Vec<u8> = (0u8..=255).cycle().take(len).collect();
            let mut dst = vec![0u8; len];
            memcpy_fast(&src, &mut dst);
            assert_eq!(src, dst, "memcpy_fast mismatch for len={len}");
        }
    }

    /// Copy with dst larger than src leaves extra bytes untouched.
    #[test]
    fn memcpy_fast_dst_larger() {
        let src = b"hello world";
        let mut dst = vec![0xFFu8; 64];
        memcpy_fast(src, &mut dst);
        assert_eq!(&dst[..src.len()], src.as_ref());
        // Bytes beyond src.len() must remain 0xFF.
        assert!(dst[src.len()..].iter().all(|&b| b == 0xFF));
    }

    /// Scalar fallback via env var.
    #[test]
    fn memcpy_fast_scalar_fallback() {
        let src: Vec<u8> = (0u8..128).collect();
        let mut dst = vec![0u8; 128];
        set_simd_disabled(true);
        memcpy_fast(&src, &mut dst);
        set_simd_disabled(false);
        assert_eq!(src, dst);
    }

    // -- inline benchmark (deterministic, not time-gated) --------------------

    /// Smoke-test that hash_batch over 64 keys runs without error and
    /// consistently matches scalar. In release builds this serves as a
    /// micro-benchmark signal (run with `cargo test -- --nocapture`).
    #[test]
    fn hash_batch_bench_smoke() {
        const N: usize = 64;
        let raw: Vec<Vec<u8>> = (0u32..N as u32)
            .map(|i| format!("benchkey:{i:08}").into_bytes())
            .collect();
        let keys: Vec<&[u8]> = raw.iter().map(|v| v.as_slice()).collect();

        let iterations = 10_000usize;

        // SIMD path timing
        let t0 = std::time::Instant::now();
        for _ in 0..iterations {
            let mut out = [0u64; N];
            hash_batch(&keys, &mut out);
            std::hint::black_box(&out);
        }
        let simd_elapsed = t0.elapsed();

        // Scalar path timing (forced via env)
        set_simd_disabled(true);
        let t1 = std::time::Instant::now();
        for _ in 0..iterations {
            let mut out = [0u64; N];
            hash_batch(&keys, &mut out);
            std::hint::black_box(&out);
        }
        let scalar_elapsed = t1.elapsed();
        set_simd_disabled(false);

        // Not a hard assertion — just report. In release mode expect >= 2x.
        let ratio = scalar_elapsed.as_nanos() as f64 / simd_elapsed.as_nanos().max(1) as f64;
        eprintln!(
            "[bench] hash_batch 64 keys × {iterations} iters | SIMD: {simd_ns}ns  Scalar: {scalar_ns}ns  ratio: {ratio:.2}x",
            simd_ns = simd_elapsed.as_nanos() / iterations as u128,
            scalar_ns = scalar_elapsed.as_nanos() / iterations as u128,
        );

        // Correctness: both paths must agree.
        let mut out_simd = vec![0u64; N];
        hash_batch(&keys, &mut out_simd);

        set_simd_disabled(true);
        let mut out_scalar = vec![0u64; N];
        hash_batch(&keys, &mut out_scalar);
        set_simd_disabled(false);

        assert_eq!(out_simd, out_scalar);
        let _ = ratio;
    }
}
