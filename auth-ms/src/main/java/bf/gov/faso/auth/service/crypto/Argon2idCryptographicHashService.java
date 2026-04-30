// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.crypto;

import de.mkammerer.argon2.Argon2;
import de.mkammerer.argon2.Argon2Factory;
import de.mkammerer.argon2.Argon2Factory.Argon2Types;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;

/**
 * Default implementation of {@link CryptographicHashService} backed by
 * <a href="https://github.com/phxql/argon2-jvm">argon2-jvm</a> (libargon2 JNI).
 *
 * <p>Peppers come from Vault under {@code faso/auth-ms/{password,otp,recovery}-pepper-v{N}}.
 * They are 32-byte hex strings (64 chars). When the corresponding property is
 * blank — typical in unit tests or local dev without Vault — a build-time
 * sentinel pepper is used and a WARN log is emitted on startup.
 *
 * <p>The class is stateless beyond {@link Argon2} instances (which are
 * thread-safe and very cheap — they wrap a JNI handle to libargon2).
 */
@Service
public class Argon2idCryptographicHashService implements CryptographicHashService {

    private static final Logger log = LoggerFactory.getLogger(Argon2idCryptographicHashService.class);

    /** Sentinel pepper for tests / local dev when Vault is not bootstrapped. */
    private static final String DEV_PEPPER_PLACEHOLDER =
            "0000000000000000000000000000000000000000000000000000000000000000";

    private final Argon2 argon2 = Argon2Factory.create(Argon2Types.ARGON2id);

    // ── Argon2 profiles (cf. plan §1, OWASP 2024) ───────────────────────────
    @Value("${admin.crypto.argon2.password.memory:65536}") private int passwordMemory;
    @Value("${admin.crypto.argon2.password.iterations:3}") private int passwordIterations;
    @Value("${admin.crypto.argon2.password.parallelism:4}") private int passwordParallelism;

    @Value("${admin.crypto.argon2.otp.memory:19456}") private int otpMemory;
    @Value("${admin.crypto.argon2.otp.iterations:2}") private int otpIterations;
    @Value("${admin.crypto.argon2.otp.parallelism:1}") private int otpParallelism;

    @Value("${admin.crypto.argon2.recovery.memory:16384}") private int recoveryMemory;
    @Value("${admin.crypto.argon2.recovery.iterations:2}") private int recoveryIterations;
    @Value("${admin.crypto.argon2.recovery.parallelism:2}") private int recoveryParallelism;

    // ── Peppers (Vault) ─────────────────────────────────────────────────────
    @Value("${admin.crypto.password-pepper:}") private String passwordPepperHex;
    @Value("${admin.crypto.otp-pepper:}") private String otpPepperHex;
    @Value("${admin.crypto.recovery-pepper:}") private String recoveryPepperHex;
    @Value("${admin.crypto.pepper-version:1}") private int currentPepperVersion;

    @PostConstruct
    void warnIfPlaceholderPepper() {
        if (passwordPepperHex == null || passwordPepperHex.isBlank()) {
            log.warn("admin.crypto.password-pepper unset — using DEV placeholder. NEVER acceptable in prod.");
            passwordPepperHex = DEV_PEPPER_PLACEHOLDER;
        }
        if (otpPepperHex == null || otpPepperHex.isBlank()) {
            log.warn("admin.crypto.otp-pepper unset — using DEV placeholder. NEVER acceptable in prod.");
            otpPepperHex = DEV_PEPPER_PLACEHOLDER;
        }
        if (recoveryPepperHex == null || recoveryPepperHex.isBlank()) {
            log.warn("admin.crypto.recovery-pepper unset — using DEV placeholder. NEVER acceptable in prod.");
            recoveryPepperHex = DEV_PEPPER_PLACEHOLDER;
        }
        log.info("CryptographicHashService initialised: password=m{}/t{}/p{} otp=m{}/t{}/p{} recovery=m{}/t{}/p{} pepper_v={}",
                passwordMemory, passwordIterations, passwordParallelism,
                otpMemory, otpIterations, otpParallelism,
                recoveryMemory, recoveryIterations, recoveryParallelism,
                currentPepperVersion);
    }

    // ── Password ────────────────────────────────────────────────────────────
    @Override
    public String hashPassword(char[] plaintext) {
        try {
            return argon2.hash(passwordIterations, passwordMemory, passwordParallelism, plaintext);
        } finally {
            argon2.wipeArray(plaintext);
        }
    }

    @Override
    public VerifyResult verifyPassword(char[] plaintext, String storedHash, HashParams params) {
        try {
            // Legacy bcrypt or anything non-argon2 is force-rehashed. We can't
            // verify it here — the caller must invoke a dedicated bcrypt path.
            if (params != null && !"argon2id".equals(params.algo())) {
                return VerifyResult.FAIL;
            }
            boolean ok = argon2.verify(storedHash, plaintext);
            if (!ok) return VerifyResult.FAIL;
            HashParams current = currentPasswordParams();
            HashParams stored = params == null
                    ? HashParams.argon2id(passwordMemory, passwordIterations, passwordParallelism, currentPepperVersion)
                    : params;
            return VerifyResult.success(stored.needsRehash(current));
        } finally {
            argon2.wipeArray(plaintext);
        }
    }

    // ── OTP (HMAC + Argon2id) ───────────────────────────────────────────────
    @Override
    public String hashOtp(String code) {
        char[] hmac = hmacToHexChars(code, otpPepperHex);
        try {
            return argon2.hash(otpIterations, otpMemory, otpParallelism, hmac);
        } finally {
            argon2.wipeArray(hmac);
        }
    }

    @Override
    public boolean verifyOtp(String code, String storedHash, int pepperVersion) {
        // pepperVersion only matters when we keep multiple peppers in memory.
        // Today only the current pepper is loaded; v0 (legacy) hashes won't
        // verify and force the caller to issue a new code.
        if (pepperVersion != currentPepperVersion) {
            log.warn("OTP verify skipped — stale pepper_version={} (current={})",
                    pepperVersion, currentPepperVersion);
            return false;
        }
        char[] hmac = hmacToHexChars(code, otpPepperHex);
        try {
            return argon2.verify(storedHash, hmac);
        } finally {
            argon2.wipeArray(hmac);
        }
    }

    // ── Recovery codes (HMAC + Argon2id) ────────────────────────────────────
    @Override
    public String hashRecoveryCode(String code) {
        char[] hmac = hmacToHexChars(code, recoveryPepperHex);
        try {
            return argon2.hash(recoveryIterations, recoveryMemory, recoveryParallelism, hmac);
        } finally {
            argon2.wipeArray(hmac);
        }
    }

    @Override
    public boolean verifyRecoveryCode(String code, String storedHash, int pepperVersion) {
        if (pepperVersion != currentPepperVersion) {
            log.warn("Recovery verify skipped — stale pepper_version={} (current={})",
                    pepperVersion, currentPepperVersion);
            return false;
        }
        char[] hmac = hmacToHexChars(code, recoveryPepperHex);
        try {
            return argon2.verify(storedHash, hmac);
        } finally {
            argon2.wipeArray(hmac);
        }
    }

    // ── Profile getters ─────────────────────────────────────────────────────
    @Override
    public HashParams currentPasswordParams() {
        return HashParams.argon2id(passwordMemory, passwordIterations, passwordParallelism, currentPepperVersion);
    }

    @Override
    public HashParams currentOtpParams() {
        return HashParams.argon2id(otpMemory, otpIterations, otpParallelism, currentPepperVersion);
    }

    @Override
    public HashParams currentRecoveryParams() {
        return HashParams.argon2id(recoveryMemory, recoveryIterations, recoveryParallelism, currentPepperVersion);
    }

    /** Public accessor used by lazy-rehash callers. */
    public int currentPepperVersion() { return currentPepperVersion; }

    // ── Internals ───────────────────────────────────────────────────────────
    /**
     * Compute {@code HMAC-SHA256(pepper, plaintext)} and return the hex digest
     * as a {@code char[]} so it can be fed to argon2-jvm and wiped afterward.
     * The pepper hex string is decoded once per call (small) — kept as String
     * because Vault delivers it that way.
     */
    private static char[] hmacToHexChars(String plaintext, String pepperHex) {
        if (plaintext == null) plaintext = "";
        try {
            byte[] keyBytes = HexFormat.of().parseHex(pepperHex);
            Mac mac = Mac.getInstance("HmacSHA256");
            mac.init(new SecretKeySpec(keyBytes, "HmacSHA256"));
            byte[] digest = mac.doFinal(plaintext.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(digest).toCharArray();
        } catch (Exception e) {
            throw new IllegalStateException("HMAC-SHA256 failed", e);
        }
    }
}
