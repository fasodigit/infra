// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.service.KetoService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.time.Duration;
import java.time.Instant;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Break-glass elevation. A SUPER-ADMIN that has lost their phone (or needs
 * urgent privileged access) can activate a temporary capability for up to
 * {@code break-glass.ttl-seconds} (4h default). The activation:
 * <ul>
 *   <li>Verifies the OTP proof passed in</li>
 *   <li>Stores a marker in KAYA at {@code auth:break_glass:{userId}}</li>
 *   <li>Writes a temporary tuple in Keto</li>
 *   <li>Publishes to {@code admin.break_glass.activated}</li>
 * </ul>
 */
@Service
public class BreakGlassService {

    private static final Logger log = LoggerFactory.getLogger(BreakGlassService.class);
    private static final String KAYA_PREFIX = "auth:break_glass:";

    private final StringRedisTemplate redis;
    private final OtpService otpService;
    private final KetoService ketoService;
    private final AdminEventProducer eventProducer;
    private final AdminAuditService auditService;
    private final AdminSettingsService settingsService;

    @Value("${admin.break-glass.ttl-seconds:14400}")
    private long defaultTtlSeconds;

    public BreakGlassService(StringRedisTemplate redis,
                             OtpService otpService,
                             KetoService ketoService,
                             AdminEventProducer eventProducer,
                             AdminAuditService auditService,
                             AdminSettingsService settingsService) {
        this.redis = redis;
        this.otpService = otpService;
        this.ketoService = ketoService;
        this.eventProducer = eventProducer;
        this.auditService = auditService;
        this.settingsService = settingsService;
    }

    /**
     * Activate break-glass for a user.
     *
     * @param userId        the user requesting elevation
     * @param capability    e.g. "manage_users", "update_settings"
     * @param justification mandatory text rationale
     * @param otpId         the otpId obtained from {@code OtpService.issue()}
     * @param otpCode       the candidate code
     * @return the elevation token (KAYA key)
     */
    public String activate(UUID userId, String capability, String justification,
                           String otpId, String otpCode) {
        if (!settingsService.getBool("break_glass.enabled", true)) {
            throw new IllegalStateException("break-glass disabled by settings");
        }
        if (settingsService.getBool("break_glass.require_justification", true) &&
                (justification == null || justification.isBlank())) {
            throw new IllegalArgumentException("justification required");
        }

        // OTP MUST belong to this user and verify cleanly.
        Optional<UUID> otpUser = otpService.getUserIdForOtp(otpId);
        if (otpUser.isEmpty() || !otpUser.get().equals(userId)) {
            throw new IllegalArgumentException("otp/user mismatch");
        }
        if (!otpService.verify(otpId, otpCode)) {
            auditService.log("break_glass.activate.otp_failed", userId,
                    "user:" + userId, null,
                    Map.of("capability", capability, "justification", justification), null);
            throw new IllegalArgumentException("invalid otp");
        }

        long ttl = settingsService.getInt("break_glass.ttl_seconds", (int) defaultTtlSeconds);

        // KAYA marker
        String key = KAYA_PREFIX + userId;
        Map<String, String> data = new HashMap<>();
        data.put("capability", capability);
        data.put("justification", justification);
        data.put("activatedAt", Instant.now().toString());
        data.put("ttl", String.valueOf(ttl));
        redis.opsForHash().putAll(key, data);
        redis.expire(key, Duration.ofSeconds(ttl));

        // Temporary Keto grant: AdminRole#super_admin@user
        ketoService.writeRelationTuple("AdminRole", capability, "super_admin", userId.toString());

        eventProducer.publishBreakGlassActivated(userId, capability, justification, ttl);
        auditService.log("break_glass.activated", userId, "user:" + userId, null,
                Map.of("capability", capability, "ttlSeconds", ttl), null);

        log.warn("BREAK-GLASS ACTIVATED user={} capability={} ttl={}s justification='{}'",
                userId, capability, ttl, justification);
        return key;
    }

    public Map<Object, Object> status(UUID userId) {
        return redis.opsForHash().entries(KAYA_PREFIX + userId);
    }

    /**
     * Manual revocation — stub. KAYA TTL handles the auto-revocation; this
     * method is the kill switch for an in-flight elevation. Iteration 2 will
     * also rip the Keto tuple.
     */
    public boolean revokeManual(UUID userId, UUID actorId) {
        // TODO Phase 4.b iteration 2 — also delete the Keto tuple synchronously.
        Boolean deleted = redis.delete(KAYA_PREFIX + userId);
        if (Boolean.TRUE.equals(deleted)) {
            auditService.log("break_glass.revoked.manual", actorId, "user:" + userId, null,
                    Map.of("targetUserId", userId.toString()), null);
            return true;
        }
        return false;
    }
}
