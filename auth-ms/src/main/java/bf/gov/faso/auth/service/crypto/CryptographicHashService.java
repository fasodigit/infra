// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.crypto;

/**
 * Unified hashing API for FASO auth-ms (Phase 4.b.3 — RFC 9106 Argon2id).
 *
 * <p>Three usage families:
 * <ul>
 *   <li><b>Password</b> — high cost (m=65536, t=3, p=4) ; no pepper because the
 *   plaintext is provided by the user (already high entropy potentially).
 *   The implementation may still apply a pepper if configured.</li>
 *   <li><b>OTP</b> — short numeric code (8 digits ≈ 26 bits). To stay
 *   resistant when the DB is exfiltrated but Vault is intact, the plaintext
 *   is first HMAC-SHA256(pepper, code) and the digest is fed to Argon2id with
 *   moderate cost (m=19456, t=2, p=1).</li>
 *   <li><b>Recovery code</b> — XXXX-XXXX (alphabet 32 chars × 8 ≈ 40 bits).
 *   Same HMAC pattern, slightly lower Argon2id cost (m=16384, t=2, p=2)
 *   because the entropy is higher than an OTP.</li>
 * </ul>
 *
 * <p>All verification methods are constant-time (handled by libargon2). They
 * also surface the {@code needsRehash} signal so callers can opportunistically
 * upgrade legacy bcrypt / outdated parameters / rotated pepper version.
 *
 * <p><b>Thread-safety</b>: implementations must be safe for concurrent use.
 */
public interface CryptographicHashService {

    /**
     * Hash a user password with the current "password" Argon2id profile.
     * The plaintext array is wiped from memory before this method returns.
     *
     * @param plaintext char[] (mutated to all-zero on return)
     * @return the encoded Argon2 hash ($argon2id$v=19$m=...$t=...$p=...$salt$hash)
     */
    String hashPassword(char[] plaintext);

    /**
     * Verify a candidate password against the stored hash.
     *
     * @param plaintext   char[] (mutated to all-zero on return)
     * @param storedHash  encoded Argon2 hash returned by {@link #hashPassword}
     * @param params      hash parameters as recorded at creation time. Used to
     *                    decide if a re-hash is required ({@link VerifyResult#needsRehash}).
     */
    VerifyResult verifyPassword(char[] plaintext, String storedHash, HashParams params);

    /**
     * Hash an OTP using {@code Argon2id(HMAC-SHA256(pepper_v_current, code))}.
     * The plaintext code is read from a String — accepted because OTPs are
     * sourced from KAYA hashes that already exposed the value as String.
     *
     * @param code  numeric OTP plaintext (8 digits by default)
     * @return encoded Argon2 hash, ready for DB storage
     */
    String hashOtp(String code);

    boolean verifyOtp(String code, String storedHash, int pepperVersion);

    /**
     * Hash a recovery code using {@code Argon2id(HMAC-SHA256(pepper_v_current, code))}.
     */
    String hashRecoveryCode(String code);

    boolean verifyRecoveryCode(String code, String storedHash, int pepperVersion);

    /** Current Argon2 password profile, exposed for {@link HashParams#needsRehash}. */
    HashParams currentPasswordParams();

    /** Current Argon2 OTP profile. */
    HashParams currentOtpParams();

    /** Current Argon2 recovery profile. */
    HashParams currentRecoveryParams();

    /**
     * Outcome of a verification call. {@link #needsRehash} is true when the
     * verified hash uses an older algorithm or smaller cost parameters than
     * the current profile — callers should re-hash silently on next write.
     */
    record VerifyResult(boolean ok, boolean needsRehash) {
        public static final VerifyResult FAIL = new VerifyResult(false, false);
        public static VerifyResult success(boolean rehash) { return new VerifyResult(true, rehash); }
    }
}
