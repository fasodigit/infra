// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.crypto.Argon2idCryptographicHashService;
import bf.gov.faso.auth.service.crypto.CryptographicHashService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.security.SecureRandom;
import java.time.Duration;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * OTP issuing & verification (8-digit codes by default).
 * <p>
 * Storage: KAYA hash {@code auth:otp:{otpId}} TTL 300s, fields:
 * {@code userId, codeHash (HMAC-SHA256), method, attempts, createdAt}.
 * Rate-limit: counter {@code auth:otp:rl:{userId}} TTL 300s — max 3 issues / 5 min.
 * Lock: {@code auth:otp:lock:{userId}} TTL 900s after 5 consecutive verify fails.
 */
@Service
public class OtpService {

    private static final Logger log = LoggerFactory.getLogger(OtpService.class);

    private static final String OTP_KEY = "auth:otp:";
    private static final String RL_KEY  = "auth:otp:rl:";
    private static final String LOCK_KEY = "auth:otp:lock:";
    private static final SecureRandom RNG = new SecureRandom();

    private final StringRedisTemplate redis;
    private final UserRepository userRepository;
    private final AdminEventProducer eventProducer;
    private final AdminAuditService auditService;
    private final AdminSettingsService settingsService;
    private final CryptographicHashService cryptoHashService;
    private final Argon2idCryptographicHashService argon2Service;

    @Value("${admin.otp.length:8}")
    private int defaultLength;

    @Value("${admin.otp.ttl-seconds:300}")
    private int defaultTtlSeconds;

    @Value("${admin.otp.rate-limit:3}")
    private int defaultRateLimit;

    @Value("${admin.otp.lock-after-fails:5}")
    private int defaultLockAfter;

    @Value("${admin.otp.lock-duration-seconds:900}")
    private int defaultLockDuration;

    public OtpService(StringRedisTemplate redis,
                      UserRepository userRepository,
                      AdminEventProducer eventProducer,
                      AdminAuditService auditService,
                      AdminSettingsService settingsService,
                      CryptographicHashService cryptoHashService,
                      Argon2idCryptographicHashService argon2Service) {
        this.redis = redis;
        this.userRepository = userRepository;
        this.eventProducer = eventProducer;
        this.auditService = auditService;
        this.settingsService = settingsService;
        this.cryptoHashService = cryptoHashService;
        this.argon2Service = argon2Service;
    }

    /**
     * Issue a new OTP for a user/method.
     *
     * @return the OTP id (used as lookup key on verify); the plain code is
     *         only published over the OTP_ISSUE topic for notifier-ms.
     */
    public String issue(UUID userId, String method) {
        String userKey = userId.toString();

        // Locked? — refuse fast.
        Boolean locked = redis.hasKey(LOCK_KEY + userKey);
        if (Boolean.TRUE.equals(locked)) {
            log.warn("OTP issue refused — user={} is locked", userId);
            throw new IllegalStateException("user is locked from OTP issuance");
        }

        // Rate-limit per user.
        int rateLimit = settingsService.getInt("otp.rate_limit_per_5min", defaultRateLimit);
        Long count = redis.opsForValue().increment(RL_KEY + userKey);
        if (count != null && count == 1L) {
            redis.expire(RL_KEY + userKey, Duration.ofSeconds(defaultTtlSeconds));
        }
        if (count != null && count > rateLimit) {
            log.warn("OTP rate-limit exceeded for user={} (count={})", userId, count);
            throw new IllegalStateException("otp rate-limit exceeded");
        }

        // Generate code + opaque otpId.
        int length = settingsService.getInt("otp.length", defaultLength);
        int ttl = settingsService.getInt("otp.ttl_seconds", defaultTtlSeconds);
        String code = generateNumericCode(length);
        String otpId = UUID.randomUUID().toString();

        // Phase 4.b.3 — store the Argon2id(HMAC-SHA256(pepper, code)) digest
        // instead of the plaintext. KAYA TTL still bounds exposure, but a
        // dump of the hot dataset no longer trivially leaks live OTPs.
        String codeHash = cryptoHashService.hashOtp(code);

        Map<String, String> hash = new HashMap<>();
        hash.put("userId", userKey);
        hash.put("codeHash", codeHash);
        hash.put("hashAlgo", "argon2id");
        hash.put("pepperVersion", String.valueOf(argon2Service.currentPepperVersion()));
        hash.put("method", method == null ? "EMAIL" : method);
        hash.put("attempts", "0");
        hash.put("createdAt", java.time.Instant.now().toString());

        String otpKey = OTP_KEY + otpId;
        redis.opsForHash().putAll(otpKey, hash);
        redis.expire(otpKey, Duration.ofSeconds(ttl));

        // Resolve email for the notifier-ms consumer.
        String email = userRepository.findById(userId)
                .map(u -> u.getEmail()).orElse(null);

        eventProducer.publishOtpIssued(userId, otpId, method, email);
        auditService.log("otp.issued", userId, "user:" + userId, null,
                Map.of("otpId", otpId, "method", method), null);

        log.info("OTP issued user={} method={} otpId={} ttl={}s", userId, method, otpId, ttl);
        return otpId;
    }

    /**
     * Verify a candidate code against the OTP record.
     *
     * @return true on success; false on mismatch / expiry / lock.
     */
    public boolean verify(String otpId, String candidate) {
        String key = OTP_KEY + otpId;
        Map<Object, Object> data = redis.opsForHash().entries(key);
        if (data == null || data.isEmpty()) {
            log.info("OTP verify miss otpId={}", otpId);
            return false;
        }

        String userIdStr = (String) data.get("userId");
        String storedHash = (String) data.get("codeHash");
        String pepperVersionStr = (String) data.get("pepperVersion");
        UUID userId = UUID.fromString(userIdStr);
        int pepperVersion = pepperVersionStr == null ? 0 : Integer.parseInt(pepperVersionStr);

        // Argon2id-backed verify (constant-time inside libargon2).
        boolean ok = storedHash != null && candidate != null
                && cryptoHashService.verifyOtp(candidate, storedHash, pepperVersion);

        if (ok) {
            redis.delete(key);
            redis.delete(LOCK_KEY + userIdStr);
            eventProducer.publishOtpVerified(userId, otpId, true);
            auditService.log("otp.verified", userId, "user:" + userId, null,
                    Map.of("otpId", otpId), null);
            log.info("OTP verified user={} otpId={}", userId, otpId);
            return true;
        }

        // Increment attempts; lock when threshold exceeded.
        Long attempts = redis.opsForHash().increment(key, "attempts", 1L);
        int lockAfter = settingsService.getInt("otp.lock_after_fails", defaultLockAfter);
        int lockDuration = settingsService.getInt("otp.lock_duration_seconds", defaultLockDuration);

        if (attempts != null && attempts >= lockAfter) {
            redis.opsForValue().set(LOCK_KEY + userIdStr, "locked",
                    Duration.ofSeconds(lockDuration));
            redis.delete(key);
            log.warn("OTP user={} locked after {} failed attempts", userId, attempts);
        }

        eventProducer.publishOtpVerified(userId, otpId, false);
        auditService.log("otp.verify.failed", userId, "user:" + userId, null,
                Map.of("otpId", otpId, "attempts", attempts), null);
        return false;
    }

    /**
     * Look up the userId associated with an OTP id (used by Break-glass and
     * dual-control approval flows that need to pair an otp with the request).
     */
    public Optional<UUID> getUserIdForOtp(String otpId) {
        Object userIdStr = redis.opsForHash().get(OTP_KEY + otpId, "userId");
        if (userIdStr instanceof String s) {
            try { return Optional.of(UUID.fromString(s)); } catch (Exception ignored) {}
        }
        return Optional.empty();
    }

    /**
     * Maintenance task — KAYA already auto-expires; this is a stub for an
     * explicit admin "purge now" lever.
     */
    public int purgeExpired() {
        // TODO Phase 4.b iteration 2 — SCAN auth:otp:* and report orphans.
        return 0;
    }

    private String generateNumericCode(int length) {
        StringBuilder sb = new StringBuilder(length);
        for (int i = 0; i < length; i++) sb.append(RNG.nextInt(10));
        return sb.toString();
    }
}
