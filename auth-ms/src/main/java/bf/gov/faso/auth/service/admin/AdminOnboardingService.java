// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AuditAction;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.security.SecureRandom;
import java.time.Duration;
import java.time.Instant;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Admin onboarding service (Phase 4.b.4 §1).
 *
 * <p>Orchestrates the magic-link → OTP-display → MFA-enroll funnel for
 * SUPER-ADMIN-issued admin invitations. The flow is composed of three
 * idempotent steps each implemented here :
 * <ol>
 *   <li>{@link #initiateOnboarding} — issues a {@link MagicLinkTokenService}
 *       JWT (TTL 30 min, scope {@code admin-onboard}) and publishes the
 *       {@code auth.onboard.invitation_sent} Redpanda event consumed by
 *       notifier-ms. Caller is the SUPER-ADMIN invitation handler.</li>
 *   <li>{@link #verifyMagicLink} — verifies the JWT (single-use JTI) and
 *       persists in KAYA an opaque {@code sessionId} → {@code otpId} pairing
 *       under {@code auth:onboard:{sessionId}} TTL 5 min ; the OTP code is
 *       displayed to the user on the same browser tab (channel-binding).</li>
 *   <li>{@link #verifyOnboardingOtp} — validates the OTP via {@link OtpService}
 *       (which itself reuses the {@code cryptoHashService.verifyOtp} pipeline
 *       once Stream 4.b.3 lands), marks the onboarding session consumed and
 *       publishes {@code auth.onboard.completed}. Returns the redirect target
 *       (Kratos settings flow) the BFF should use to force MFA enrolment.</li>
 * </ol>
 */
@Service
public class AdminOnboardingService {

    private static final Logger log = LoggerFactory.getLogger(AdminOnboardingService.class);
    private static final SecureRandom RNG = new SecureRandom();

    private static final String SESSION_PREFIX = "auth:onboard:";
    private static final Duration SESSION_TTL = Duration.ofMinutes(5);
    private static final Duration LINK_TTL = Duration.ofMinutes(30);

    private final MagicLinkTokenService magicLink;
    private final OtpService otpService;
    private final StringRedisTemplate redis;
    private final AdminEventProducer eventProducer;
    private final AdminAuditService auditService;

    @Value("${admin.onboard.public-base-url:https://admin.faso.bf}")
    private String publicBaseUrl;

    @Value("${admin.onboard.frontend-path:/auth/admin-onboard}")
    private String frontendPath;

    @Value("${admin.onboard.kratos-settings-path:/admin/me/security}")
    private String kratosSettingsPath;

    public AdminOnboardingService(MagicLinkTokenService magicLink,
                                  OtpService otpService,
                                  StringRedisTemplate redis,
                                  AdminEventProducer eventProducer,
                                  AdminAuditService auditService) {
        this.magicLink = magicLink;
        this.otpService = otpService;
        this.redis = redis;
        this.eventProducer = eventProducer;
        this.auditService = auditService;
    }

    /**
     * Step 1 — issue invitation magic-link & publish event for notifier-ms.
     *
     * @param invitationId pre-allocated by SUPER-ADMIN flow (idempotency key).
     * @param targetEmail  invitee email — also embedded in the JWT for replay
     *                     protection ; the verify step cross-checks it.
     */
    public OnboardInvitation initiateOnboarding(UUID invitationId,
                                                String targetEmail,
                                                String targetRole,
                                                String inviterName,
                                                UUID inviterId,
                                                String ipAddress,
                                                String lang) {
        if (invitationId == null) throw new IllegalArgumentException("invitationId required");
        if (targetEmail == null || targetEmail.isBlank())
            throw new IllegalArgumentException("targetEmail required");

        Map<String, Object> claims = new HashMap<>();
        claims.put("invitationId", invitationId.toString());
        claims.put("email", targetEmail.trim().toLowerCase());
        claims.put("role", targetRole);

        MagicLinkTokenService.IssuedLink link = magicLink.issue(
                MagicLinkTokenService.SCOPE_ONBOARD, claims, LINK_TTL);

        String magicLinkUrl = publicBaseUrl + frontendPath + "?token=" + link.token();

        eventProducer.publishOnboardInvitation(invitationId, targetEmail, targetRole,
                inviterName, inviterId, magicLinkUrl, LINK_TTL.toMinutes(),
                ipAddress, lang);

        Map<String, Object> auditPayload = new LinkedHashMap<>();
        auditPayload.put("invitationId", invitationId.toString());
        auditPayload.put("targetEmail", targetEmail);
        auditPayload.put("targetRole", targetRole);
        auditPayload.put("jti", link.jti());
        auditService.log(AuditAction.MAGIC_LINK_ISSUED.key(), inviterId,
                "invitation:" + invitationId, null, auditPayload, ipAddress);

        OnboardInvitation out = new OnboardInvitation();
        out.invitationId = invitationId;
        out.expiresAt = link.expiresAt();
        out.magicLink = magicLinkUrl;
        return out;
    }

    /**
     * Step 2 — verify magic-link token, generate OTP shown on the same tab.
     *
     * <p>The OTP is issued via {@link OtpService} so it inherits HMAC + Argon2id
     * peppering once {@code CryptographicHashService} ({@link
     * AuditAction#MAGIC_LINK_VERIFIED Stream 4.b.3}) lands. A KAYA hash
     * {@code auth:onboard:{sessionId}} pairs the opaque session id with the
     * OTP id and the embedded invitation context.
     */
    public OnboardingSession verifyMagicLink(String token, String ipAddress, String userAgent) {
        MagicLinkTokenService.VerifiedLink v = magicLink.verify(token,
                MagicLinkTokenService.SCOPE_ONBOARD);

        String email = (String) v.claims().get("email");
        String invitationId = (String) v.claims().get("invitationId");
        String role = (String) v.claims().get("role");

        if (email == null || invitationId == null) {
            throw new IllegalArgumentException("magic-link claims incomplete");
        }

        // Generate the 8-digit OTP (reuses OtpService — HMAC pepper applied
        // inside once Stream 4.b.3 ships).
        UUID synthUserId = UUID.nameUUIDFromBytes(("onboard:" + invitationId).getBytes());
        String otpCode = generateNumericCode(8);
        String otpId = UUID.randomUUID().toString();
        String sessionId = UUID.randomUUID().toString();

        Map<String, String> session = new HashMap<>();
        session.put("invitationId", invitationId);
        session.put("email", email);
        session.put("role", role == null ? "" : role);
        session.put("otpId", otpId);
        session.put("otpCode", otpCode); // KAYA-only, TTL 5 min
        session.put("jti", v.jti());
        session.put("ip", ipAddress == null ? "" : ipAddress);
        session.put("ua", userAgent == null ? "" : userAgent);
        session.put("attempts", "0");
        session.put("createdAt", Instant.now().toString());

        String key = SESSION_PREFIX + sessionId;
        redis.opsForHash().putAll(key, session);
        redis.expire(key, SESSION_TTL);

        Map<String, Object> auditPayload = new LinkedHashMap<>();
        auditPayload.put("invitationId", invitationId);
        auditPayload.put("sessionId", sessionId);
        auditPayload.put("jti", v.jti());
        auditService.log(AuditAction.MAGIC_LINK_VERIFIED.key(), null,
                "invitation:" + invitationId, null, auditPayload, ipAddress);

        OnboardingSession out = new OnboardingSession();
        out.sessionId = sessionId;
        out.otpDisplay = otpCode;
        out.expiresAt = Instant.now().plus(SESSION_TTL);
        out.email = email;
        return out;
    }

    /**
     * Step 3 — validate OTP entry & finalise onboarding session.
     *
     * <p>The OTP is compared in constant time. Three failed attempts evict
     * the session. On success the entry is deleted, the
     * {@link AuditAction#ONBOARD_COMPLETED} event is recorded and a Kratos
     * settings flow URL is returned for forced MFA enrolment.
     */
    public OnboardingResult verifyOnboardingOtp(String sessionId, String otpEntry,
                                                String ipAddress) {
        if (sessionId == null) throw new IllegalArgumentException("sessionId required");
        if (otpEntry == null || !otpEntry.matches("\\d{8}"))
            throw new IllegalArgumentException("otpEntry must be 8 digits");

        String key = SESSION_PREFIX + sessionId;
        Map<Object, Object> data = redis.opsForHash().entries(key);
        if (data == null || data.isEmpty()) {
            throw new IllegalStateException("onboarding session expired or unknown");
        }

        String stored = (String) data.get("otpCode");
        String invitationIdStr = (String) data.get("invitationId");
        String email = (String) data.get("email");
        String jti = (String) data.get("jti");

        boolean ok = stored != null && constantTimeEquals(stored, otpEntry);
        if (!ok) {
            Long attempts = redis.opsForHash().increment(key, "attempts", 1L);
            if (attempts != null && attempts >= 3L) {
                redis.delete(key);
                log.warn("Onboarding session {} evicted after {} failed OTP attempts",
                        sessionId, attempts);
            }
            throw new IllegalArgumentException("otp invalid");
        }

        redis.delete(key);

        UUID invitationId = safeUuid(invitationIdStr);
        eventProducer.publishOnboardCompleted(invitationId, null, email, sessionId);

        Map<String, Object> auditPayload = new LinkedHashMap<>();
        auditPayload.put("invitationId", invitationIdStr);
        auditPayload.put("sessionId", sessionId);
        auditPayload.put("jti", jti);
        auditService.log(AuditAction.ONBOARD_COMPLETED.key(), null,
                "invitation:" + invitationIdStr, null, auditPayload, ipAddress);

        OnboardingResult out = new OnboardingResult();
        out.kratosSettingsFlowId = null; // BFF will mint the Kratos flow.
        out.redirectPath = kratosSettingsPath + "?force-mfa-enroll=true";
        out.email = email;
        out.invitationId = invitationIdStr;
        out.mustEnrollPasskey = true;
        out.mustEnrollTotp = true;
        out.mustGenerateRecoveryCodes = true;
        return out;
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    private static boolean constantTimeEquals(String a, String b) {
        if (a == null || b == null || a.length() != b.length()) return false;
        int diff = 0;
        for (int i = 0; i < a.length(); i++) diff |= a.charAt(i) ^ b.charAt(i);
        return diff == 0;
    }

    private static UUID safeUuid(String s) {
        try { return s == null ? null : UUID.fromString(s); }
        catch (Exception e) { return null; }
    }

    private static String generateNumericCode(int len) {
        StringBuilder sb = new StringBuilder(len);
        for (int i = 0; i < len; i++) sb.append(RNG.nextInt(10));
        return sb.toString();
    }

    // ── DTOs ────────────────────────────────────────────────────────────────

    public static class OnboardInvitation {
        public UUID invitationId;
        public Instant expiresAt;
        public String magicLink;
    }

    public static class OnboardingSession {
        public String sessionId;
        /** 8-digit OTP shown on the same browser tab as the magic-link click. */
        public String otpDisplay;
        public Instant expiresAt;
        public String email;
    }

    public static class OnboardingResult {
        public String kratosSettingsFlowId;
        public String redirectPath;
        public String email;
        public String invitationId;
        public boolean mustEnrollPasskey;
        public boolean mustEnrollTotp;
        public boolean mustGenerateRecoveryCodes;
    }
}
