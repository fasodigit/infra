// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AccountRecoveryRequest;
import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.AccountRecoveryRequestRepository;
import bf.gov.faso.auth.repository.DeviceRegistrationRepository;
import bf.gov.faso.auth.repository.RecoveryCodeRepository;
import bf.gov.faso.auth.repository.TotpEnrollmentRepository;
import bf.gov.faso.auth.repository.UserRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.time.Duration;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Account recovery flows (delta amendment 2026-04-30 §5).
 *
 * <p>Two paths:
 * <ul>
 *   <li><b>SELF</b> — user lost MFA. {@link #initiateSelfRecovery} issues a
 *   single-use signed token (lifetime 30 min) emailed via notifier-ms.</li>
 *   <li><b>ADMIN_INITIATED</b> — SUPER_ADMIN aids a target user.
 *   {@link #initiateAdminRecovery} resets the target's MFA factors (TOTP,
 *   PassKeys, recovery codes), generates an 8-digit token (lifetime 1h), and
 *   publishes the token over Redpanda for notifier-ms to email the target.</li>
 * </ul>
 *
 * <p>{@link #completeRecovery} validates a token, marks it consumed, sets
 * {@code users.must_reenroll_mfa = true} and returns a degraded AAL1 session
 * representation. The actual Kratos session creation is delegated to the
 * caller (controller / login flow) which orchestrates Kratos + the BFF.
 */
@Service
public class AccountRecoveryService {

    private static final Logger log = LoggerFactory.getLogger(AccountRecoveryService.class);
    private static final SecureRandom RNG = new SecureRandom();

    /** Self-recovery token TTL (cf. delta §5.A — 30 minutes). */
    private static final long SELF_TTL_MIN = 30L;
    /** Admin-initiated 8-digit token TTL (cf. delta §5.B — 1 hour). */
    private static final long ADMIN_TTL_MIN = 60L;

    private final AccountRecoveryRequestRepository recoveryRepo;
    private final UserRepository userRepo;
    private final TotpEnrollmentRepository totpRepo;
    private final RecoveryCodeRepository recoveryCodeRepo;
    private final DeviceRegistrationRepository deviceRepo;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;
    private final MagicLinkTokenService magicLinkService;
    private final org.springframework.data.redis.core.StringRedisTemplate redis;

    @org.springframework.beans.factory.annotation.Value("${admin.recovery.public-base-url:https://admin.faso.bf}")
    private String publicBaseUrl;

    @org.springframework.beans.factory.annotation.Value("${admin.recovery.frontend-path:/auth/recovery}")
    private String recoveryPath;

    public AccountRecoveryService(AccountRecoveryRequestRepository recoveryRepo,
                                  UserRepository userRepo,
                                  TotpEnrollmentRepository totpRepo,
                                  RecoveryCodeRepository recoveryCodeRepo,
                                  DeviceRegistrationRepository deviceRepo,
                                  AdminAuditService auditService,
                                  AdminEventProducer eventProducer,
                                  MagicLinkTokenService magicLinkService,
                                  org.springframework.data.redis.core.StringRedisTemplate redis) {
        this.recoveryRepo = recoveryRepo;
        this.userRepo = userRepo;
        this.totpRepo = totpRepo;
        this.recoveryCodeRepo = recoveryCodeRepo;
        this.deviceRepo = deviceRepo;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
        this.magicLinkService = magicLinkService;
        this.redis = redis;
    }

    @Transactional
    public RecoveryResponse initiateSelfRecovery(String email) {
        return initiateSelfRecovery(email, null, null);
    }

    /**
     * Phase 4.b.4 — self-initiated recovery now issues a {@link
     * MagicLinkTokenService} JWT (HMAC-SHA256, single-use, TTL 30 min) instead
     * of an opaque URL token. The DB row still tracks the request lifecycle
     * (PENDING/USED/EXPIRED) so audit & rate-limit code remain unchanged ; the
     * stored {@code token_hash} now holds {@code sha256(jti)} so subsequent
     * endpoints can resolve a magic-link click back to its recovery row.
     */
    @Transactional
    public RecoveryResponse initiateSelfRecovery(String email, String ipAddress, String userAgent) {
        if (email == null || email.isBlank()) {
            throw new IllegalArgumentException("email required");
        }
        Optional<User> opt = userRepo.findByEmail(email.trim().toLowerCase());
        // Always behave the same to avoid email enumeration.
        if (opt.isEmpty()) {
            log.info("Self-recovery requested for unknown email — returning generic OK");
            RecoveryResponse rr = new RecoveryResponse();
            rr.requestId = UUID.randomUUID().toString();
            rr.expiresAt = Instant.now().plus(SELF_TTL_MIN, ChronoUnit.MINUTES);
            rr.delivery = "email";
            return rr;
        }
        User target = opt.get();

        Map<String, Object> claims = new java.util.HashMap<>();
        claims.put("userId", target.getId().toString());
        claims.put("email", target.getEmail());
        MagicLinkTokenService.IssuedLink link = magicLinkService.issue(
                MagicLinkTokenService.SCOPE_RECOVERY, claims,
                Duration.ofMinutes(SELF_TTL_MIN));

        // Store sha256(jti) so verifyRecoveryMagicLink can find the row by jti.
        String tokenHash = sha256(link.jti());
        AccountRecoveryRequest req = persistRequest(target.getId(), null,
                AccountRecoveryRequest.Type.SELF, tokenHash,
                Instant.now().plus(SELF_TTL_MIN, ChronoUnit.MINUTES), null);

        String recoveryLink = publicBaseUrl + recoveryPath + "?token=" + link.token();

        auditService.log(AuditAction.ACCOUNT_RECOVERY_SELF_INITIATED.key(),
                target.getId(), "user:" + target.getId(), null,
                Map.of("requestId", req.getId().toString(), "email", target.getEmail(),
                        "jti", link.jti()),
                ipAddress);
        eventProducer.publishRecoverySelfInitiated(target.getId(), target.getEmail(),
                req.getId().toString(), recoveryLink, SELF_TTL_MIN,
                target.getFirstName(), ipAddress, userAgent);

        RecoveryResponse rr = new RecoveryResponse();
        rr.requestId = req.getId().toString();
        rr.expiresAt = req.getExpiresAt();
        rr.delivery = "email";
        // Plain token returned ONLY for SELF so the controller can build the
        // magic link; it is never stored in DB (only sha256(jti) is).
        rr.token = link.token();
        return rr;
    }

    /**
     * Phase 4.b.4 §3 — magic-link verify entrypoint mirroring the onboarding
     * flow : verifies the JWT, allocates a 5-minute session storing an OTP id
     * and the resolved {@code userId}. Returns the OTP code to display on the
     * same browser tab and an opaque {@code sessionId}.
     *
     * @return KAYA-backed session whose values are consumed by
     *         {@link #completeRecoveryWithSession(String, String, String)}.
     */
    public RecoverySession verifyRecoveryMagicLink(String token, String ipAddress, String userAgent) {
        MagicLinkTokenService.VerifiedLink v = magicLinkService.verify(token,
                MagicLinkTokenService.SCOPE_RECOVERY);

        String tokenHash = sha256(v.jti());
        AccountRecoveryRequest req = recoveryRepo.findByTokenHash(tokenHash)
                .orElseThrow(() -> new IllegalStateException(
                        "magic-link not found in recovery store"));
        if (req.getStatus() != AccountRecoveryRequest.Status.PENDING) {
            throw new IllegalStateException("recovery request already consumed");
        }
        if (req.isExpired()) {
            req.setStatus(AccountRecoveryRequest.Status.EXPIRED);
            recoveryRepo.save(req);
            throw new IllegalStateException("recovery request expired");
        }

        String otpCode = generateNumericToken(8);
        String sessionId = UUID.randomUUID().toString();
        String key = "auth:recovery:session:" + sessionId;

        Map<String, String> session = new java.util.HashMap<>();
        session.put("requestId", req.getId().toString());
        session.put("userId", req.getUserId().toString());
        session.put("otpCode", otpCode);
        session.put("jti", v.jti());
        session.put("ip", ipAddress == null ? "" : ipAddress);
        session.put("ua", userAgent == null ? "" : userAgent);
        session.put("attempts", "0");
        session.put("createdAt", Instant.now().toString());

        redis.opsForHash().putAll(key, session);
        redis.expire(key, Duration.ofMinutes(5));

        RecoverySession out = new RecoverySession();
        out.sessionId = sessionId;
        out.otpDisplay = otpCode;
        out.expiresAt = Instant.now().plus(5, ChronoUnit.MINUTES);
        out.requestId = req.getId().toString();
        return out;
    }

    /**
     * Phase 4.b.4 — completion path for the magic-link channel-binding
     * recovery flow. Consumes the KAYA session created by
     * {@link #verifyRecoveryMagicLink} and runs the legacy completion logic
     * (mark request USED, force MFA reenroll, AAL1 session).
     */
    @Transactional
    public CompleteResponse completeRecoveryWithSession(String sessionId, String otpEntry,
                                                        String kratosFlowId) {
        if (sessionId == null) throw new IllegalArgumentException("sessionId required");
        if (otpEntry == null || !otpEntry.matches("\\d{8}")) {
            throw new IllegalArgumentException("otpEntry must be 8 digits");
        }
        String key = "auth:recovery:session:" + sessionId;
        Map<Object, Object> data = redis.opsForHash().entries(key);
        if (data == null || data.isEmpty()) {
            throw new IllegalStateException("recovery session expired or unknown");
        }
        String stored = (String) data.get("otpCode");
        boolean ok = stored != null && stored.length() == otpEntry.length()
                && constantTimeEqual(stored, otpEntry);
        if (!ok) {
            Long attempts = redis.opsForHash().increment(key, "attempts", 1L);
            if (attempts != null && attempts >= 3L) redis.delete(key);
            throw new IllegalArgumentException("otp invalid");
        }
        String requestIdStr = (String) data.get("requestId");
        redis.delete(key);

        AccountRecoveryRequest req = recoveryRepo.findById(UUID.fromString(requestIdStr))
                .orElseThrow(() -> new IllegalStateException("request gone"));
        if (req.getStatus() != AccountRecoveryRequest.Status.PENDING) {
            throw new IllegalStateException("recovery already consumed");
        }
        req.setStatus(AccountRecoveryRequest.Status.USED);
        req.setUsedAt(Instant.now());
        recoveryRepo.save(req);

        userRepo.findById(req.getUserId()).ifPresent(u -> {
            u.setMustReenrollMfa(true);
            userRepo.save(u);
        });

        auditService.log(AuditAction.ACCOUNT_RECOVERY_COMPLETED.key(), req.getUserId(),
                "user:" + req.getUserId(), null,
                Map.of("requestId", req.getId().toString(), "type", req.getRecoveryType().name(),
                        "channel", "magic_link"), null);
        eventProducer.publishRecoveryCompleted(req.getUserId(),
                req.getRecoveryType().name(), req.getId().toString());

        CompleteResponse out = new CompleteResponse();
        out.userId = req.getUserId().toString();
        out.aal = "AAL1";
        out.mustReenrollMfa = true;
        out.requestId = req.getId().toString();
        return out;
    }

    private static boolean constantTimeEqual(String a, String b) {
        int diff = 0;
        for (int i = 0; i < a.length(); i++) diff |= a.charAt(i) ^ b.charAt(i);
        return diff == 0;
    }

    /**
     * Reset the target's MFA factors and generate an 8-digit token. The
     * token is published over Redpanda for notifier-ms to email — never
     * returned in the HTTP response (see controller).
     */
    @Transactional
    public RecoveryResponse initiateAdminRecovery(UUID targetUserId, UUID initiatorId, String motif) {
        User target = userRepo.findById(targetUserId)
                .orElseThrow(() -> new IllegalArgumentException("target user not found"));

        // Reset MFA factors of the target (cf. delta §5.B).
        totpRepo.findByUserIdAndDisabledAtIsNull(targetUserId).ifPresent(t -> {
            t.setDisabledAt(Instant.now());
            totpRepo.save(t);
        });
        deviceRepo.findByUserIdAndRevokedAtIsNull(targetUserId).forEach(d -> {
            d.setRevokedAt(Instant.now());
            deviceRepo.save(d);
        });
        Instant now = Instant.now();
        recoveryCodeRepo.findUnusedByUserId(targetUserId).forEach(rc -> {
            rc.setUsedAt(now);
            recoveryCodeRepo.save(rc);
        });

        String token = generateNumericToken(8);
        String tokenHash = sha256(token);

        AccountRecoveryRequest req = persistRequest(targetUserId, initiatorId,
                AccountRecoveryRequest.Type.ADMIN_INITIATED, tokenHash,
                Instant.now().plus(ADMIN_TTL_MIN, ChronoUnit.MINUTES), motif);

        auditService.log(AuditAction.ACCOUNT_RECOVERY_ADMIN_INITIATED.key(),
                initiatorId, "user:" + targetUserId, null,
                Map.of("requestId", req.getId().toString(),
                        "motif", motif == null ? "" : motif), null);
        eventProducer.publishRecoveryAdminInitiated(targetUserId, target.getEmail(),
                initiatorId, req.getId().toString(), token, motif);

        RecoveryResponse rr = new RecoveryResponse();
        rr.requestId = req.getId().toString();
        rr.expiresAt = req.getExpiresAt();
        rr.delivery = "email_via_notifier";
        // Token deliberately NOT returned here — only emailed to the user.
        return rr;
    }

    @Transactional
    public CompleteResponse completeRecovery(String tokenOrCode) {
        if (tokenOrCode == null || tokenOrCode.isBlank()) {
            throw new IllegalArgumentException("token required");
        }
        String tokenHash = sha256(tokenOrCode.trim());
        AccountRecoveryRequest req = recoveryRepo.findByTokenHash(tokenHash)
                .orElseThrow(() -> new IllegalArgumentException("token invalid"));

        if (req.getStatus() != AccountRecoveryRequest.Status.PENDING) {
            throw new IllegalStateException("token already consumed or rejected");
        }
        if (req.isExpired()) {
            req.setStatus(AccountRecoveryRequest.Status.EXPIRED);
            recoveryRepo.save(req);
            throw new IllegalStateException("token expired");
        }

        req.setStatus(AccountRecoveryRequest.Status.USED);
        req.setUsedAt(Instant.now());
        recoveryRepo.save(req);

        // Force MFA re-enrolment.
        userRepo.findById(req.getUserId()).ifPresent(u -> {
            u.setMustReenrollMfa(true);
            userRepo.save(u);
        });

        auditService.log(AuditAction.ACCOUNT_RECOVERY_COMPLETED.key(), req.getUserId(),
                "user:" + req.getUserId(), null,
                Map.of("requestId", req.getId().toString(),
                        "type", req.getRecoveryType().name()), null);
        eventProducer.publishRecoveryCompleted(req.getUserId(),
                req.getRecoveryType().name(), req.getId().toString());

        CompleteResponse out = new CompleteResponse();
        out.userId = req.getUserId().toString();
        out.aal = "AAL1";
        out.mustReenrollMfa = true;
        out.requestId = req.getId().toString();
        return out;
    }

    public List<AccountRecoveryRequest> listForUser(UUID userId) {
        return recoveryRepo.findByUserIdOrderByCreatedAtDesc(userId);
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    private AccountRecoveryRequest persistRequest(UUID userId, UUID initiatorId,
                                                  AccountRecoveryRequest.Type type,
                                                  String tokenHash, Instant expiresAt,
                                                  String motif) {
        AccountRecoveryRequest req = new AccountRecoveryRequest();
        req.setUserId(userId);
        req.setInitiatedBy(initiatorId);
        req.setRecoveryType(type);
        req.setTokenHash(tokenHash);
        req.setExpiresAt(expiresAt);
        req.setStatus(AccountRecoveryRequest.Status.PENDING);
        req.setCreatedAt(Instant.now());
        req.setMotif(motif);
        String traceId = MDC.get("traceId");
        if (traceId != null && traceId.length() > 32) traceId = traceId.substring(0, 32);
        req.setTraceId(traceId);
        return recoveryRepo.save(req);
    }

    private static String generateNumericToken(int length) {
        StringBuilder sb = new StringBuilder(length);
        for (int i = 0; i < length; i++) sb.append(RNG.nextInt(10));
        return sb.toString();
    }

    private static String generateUrlToken() {
        byte[] buf = new byte[32];
        RNG.nextBytes(buf);
        return java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(buf);
    }

    private static String sha256(String s) {
        try {
            byte[] d = MessageDigest.getInstance("SHA-256")
                    .digest(s.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(d);
        } catch (Exception e) {
            throw new IllegalStateException("sha256 unavailable", e);
        }
    }

    // ── DTOs ────────────────────────────────────────────────────────────────

    public static class RecoveryResponse {
        public String requestId;
        public Instant expiresAt;
        public String delivery;
        /** Plain token, only populated for SELF flow (magic link). */
        public String token;

        public Map<String, Object> toPublicMap() {
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("requestId", requestId);
            m.put("expiresAt", expiresAt == null ? null : expiresAt.toString());
            m.put("delivery", delivery);
            return m;
        }
    }

    public static class CompleteResponse {
        public String userId;
        public String aal;
        public boolean mustReenrollMfa;
        public String requestId;
    }

    /** Phase 4.b.4 — magic-link verify-link response. */
    public static class RecoverySession {
        public String sessionId;
        public String otpDisplay;
        public Instant expiresAt;
        public String requestId;
    }
}
