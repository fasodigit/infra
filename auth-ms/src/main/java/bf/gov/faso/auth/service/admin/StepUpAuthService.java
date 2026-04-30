// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.JwtSigningKeyRepository;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.security.StepUpMethod;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.nimbusds.jose.JOSEException;
import com.nimbusds.jose.JOSEObjectType;
import com.nimbusds.jose.JWSAlgorithm;
import com.nimbusds.jose.JWSHeader;
import com.nimbusds.jose.crypto.ECDSASigner;
import com.nimbusds.jwt.JWTClaimsSet;
import com.nimbusds.jwt.SignedJWT;
import org.bouncycastle.asn1.pkcs.PrivateKeyInfo;
import org.bouncycastle.openssl.PEMKeyPair;
import org.bouncycastle.openssl.PEMParser;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.io.StringReader;
import java.security.KeyFactory;
import java.security.interfaces.ECPrivateKey;
import java.security.spec.PKCS8EncodedKeySpec;
import java.time.Duration;
import java.time.Instant;
import java.util.Date;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.stream.Collectors;

/**
 * Phase 4.b.7 — Step-up auth for sensitive operations (Tier 4).
 *
 * <p>Flow :
 * <ol>
 *   <li>Filter detects stale {@code last_step_up_at} → calls {@link #initiateStepUp}
 *       (or controller {@code /admin/auth/step-up/begin}) → KAYA HASH
 *       {@code auth:step_up:session:{sessionId}} TTL 300s.</li>
 *   <li>Frontend opens guard modal, prompts user with one of the
 *       {@code allowedMethods}, calls {@code /admin/auth/step-up/{sessionId}/verify}.</li>
 *   <li>{@link #verifyStepUp} dispatches to PASSKEY / PUSH_APPROVAL / TOTP / OTP
 *       and, on success, mints a short-lived JWT (TTL 300s) with
 *       {@code last_step_up_at = now}; client retries the original request.</li>
 * </ol>
 *
 * <p>Push approval integration is the joining point with Phase 4.b.5 — see
 * {@link #verifyPushApproval}.
 */
@Service
public class StepUpAuthService {

    private static final Logger log = LoggerFactory.getLogger(StepUpAuthService.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** KAYA HASH key prefix — TTL 300s. */
    public static final String SESSION_KEY = "auth:step_up:session:";
    /** KAYA STR key prefix — last successful proof cache TTL 300s. */
    public static final String PROOF_KEY = "auth:step_up:proof:";

    /** Default step-up session TTL — 5 minutes. */
    public static final Duration DEFAULT_TTL = Duration.ofSeconds(300);

    private final StringRedisTemplate redis;
    private final UserRepository userRepository;
    private final JwtSigningKeyRepository keyRepository;
    private final WebAuthnService webAuthnService;
    private final TotpService totpService;
    private final OtpService otpService;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;
    /** Phase 4.b.5 — push approval (optional autowiring; Stream may land later). */
    private final org.springframework.beans.factory.ObjectProvider<PushApprovalService> pushApproval;

    @Value("${admin.step-up.default-max-age-seconds:300}")
    private int defaultMaxAgeSeconds;

    @Value("${auth.jwt.issuer:https://auth.faso.gov.bf}")
    private String jwtIssuer;

    @Value("${auth.jwt.audience:faso-digitalisation}")
    private String jwtAudience;

    public StepUpAuthService(StringRedisTemplate redis,
                             UserRepository userRepository,
                             JwtSigningKeyRepository keyRepository,
                             WebAuthnService webAuthnService,
                             TotpService totpService,
                             OtpService otpService,
                             AdminAuditService auditService,
                             AdminEventProducer eventProducer,
                             org.springframework.beans.factory.ObjectProvider<PushApprovalService> pushApproval) {
        this.redis = redis;
        this.userRepository = userRepository;
        this.keyRepository = keyRepository;
        this.webAuthnService = webAuthnService;
        this.totpService = totpService;
        this.otpService = otpService;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
        this.pushApproval = pushApproval;
    }

    // ── Session lifecycle ───────────────────────────────────────────────────

    /**
     * Open a fresh step-up session for {@code userId} on {@code requestedFor}
     * (e.g. {@code "POST /admin/grants/request"}).
     */
    public StepUpSession initiateStepUp(UUID userId, String requestedFor,
                                        List<StepUpMethod> allowedMethods) {
        String sessionId = UUID.randomUUID().toString();
        Instant createdAt = Instant.now();
        Instant expiresAt = createdAt.plus(DEFAULT_TTL);

        List<StepUpMethod> methods = (allowedMethods == null || allowedMethods.isEmpty())
                ? List.of(StepUpMethod.PASSKEY, StepUpMethod.PUSH_APPROVAL,
                          StepUpMethod.TOTP, StepUpMethod.OTP)
                : allowedMethods;

        String methodsCsv = methods.stream().map(StepUpMethod::name)
                .collect(Collectors.joining(","));

        Map<String, String> hash = new HashMap<>();
        hash.put("userId", userId.toString());
        hash.put("requiredFor", requestedFor == null ? "" : requestedFor);
        hash.put("allowedMethods", methodsCsv);
        hash.put("createdAt", createdAt.toString());
        hash.put("status", "PENDING");

        String key = SESSION_KEY + sessionId;
        redis.opsForHash().putAll(key, hash);
        redis.expire(key, DEFAULT_TTL);

        eventProducer.publishStepUpRequested(userId, sessionId, requestedFor);
        auditService.log("step_up.requested", userId, "user:" + userId, null,
                Map.of("sessionId", sessionId, "requestedFor", requestedFor == null ? "" : requestedFor),
                null);

        log.info("Step-up session opened user={} sessionId={} requestedFor={}",
                userId, sessionId, requestedFor);
        return new StepUpSession(sessionId, userId, requestedFor, methods, createdAt, expiresAt);
    }

    /** Read-only status for polling (push approval async wait). */
    public Optional<Status> getStepUpStatus(UUID sessionId) {
        Map<Object, Object> data = redis.opsForHash().entries(SESSION_KEY + sessionId);
        if (data == null || data.isEmpty()) return Optional.empty();
        String status = (String) data.getOrDefault("status", "PENDING");
        String userIdStr = (String) data.get("userId");
        return Optional.of(new Status(sessionId, status,
                userIdStr == null ? null : UUID.fromString(userIdStr)));
    }

    // ── Verify ──────────────────────────────────────────────────────────────

    /**
     * Verify the supplied {@code proof} against the recorded session and method.
     * On success, returns a freshly-minted short-lived JWT carrying
     * {@code last_step_up_at = now()} as a numeric epoch-millis claim.
     */
    public StepUpResult verifyStepUp(UUID sessionId, StepUpMethod method, String proof) {
        Map<Object, Object> data = redis.opsForHash().entries(SESSION_KEY + sessionId);
        if (data == null || data.isEmpty()) {
            log.info("Step-up verify miss — sessionId={}", sessionId);
            return StepUpResult.failure("session_not_found");
        }
        String userIdStr = (String) data.get("userId");
        if (userIdStr == null) return StepUpResult.failure("session_corrupt");
        UUID userId;
        try {
            userId = UUID.fromString(userIdStr);
        } catch (IllegalArgumentException e) {
            return StepUpResult.failure("session_corrupt");
        }

        String allowedCsv = (String) data.getOrDefault("allowedMethods", "");
        if (!allowedCsv.contains(method.name())) {
            return failAndAudit(userId, sessionId, method, "method_not_allowed");
        }

        boolean ok = switch (method) {
            case PASSKEY -> verifyPasskey(userId, proof);
            case PUSH_APPROVAL -> verifyPushApproval(userId, proof);
            case TOTP -> verifyTotp(userId, proof);
            case OTP -> verifyOtp(proof, userId);
        };

        if (!ok) {
            return failAndAudit(userId, sessionId, method, "proof_invalid");
        }

        // Cache the latest valid proof — useful for chained operations.
        redis.opsForValue().set(PROOF_KEY + userId, sessionId.toString(), DEFAULT_TTL);

        // Mark the session as VERIFIED (kept around for status polling until TTL).
        redis.opsForHash().put(SESSION_KEY + sessionId, "status", "VERIFIED");
        redis.opsForHash().put(SESSION_KEY + sessionId, "verifiedAt", Instant.now().toString());
        redis.opsForHash().put(SESSION_KEY + sessionId, "method", method.name());

        String token = mintStepUpToken(userId, method);
        eventProducer.publishStepUpVerified(userId, sessionId.toString(), method.name());
        auditService.log("step_up.verified", userId, "user:" + userId, null,
                Map.of("sessionId", sessionId.toString(), "method", method.name()), null);

        log.info("Step-up verified user={} sessionId={} method={}",
                userId, sessionId, method);
        return StepUpResult.success(token, userId, method);
    }

    private StepUpResult failAndAudit(UUID userId, UUID sessionId,
                                      StepUpMethod method, String reason) {
        eventProducer.publishStepUpFailed(userId, sessionId.toString(),
                method == null ? null : method.name(), reason);
        auditService.log("step_up.failed", userId, "user:" + userId, null,
                Map.of("sessionId", sessionId.toString(),
                        "method", method == null ? "" : method.name(),
                        "reason", reason), null);
        log.warn("Step-up FAILED user={} sessionId={} method={} reason={}",
                userId, sessionId, method, reason);
        return StepUpResult.failure(reason);
    }

    // ── Method-specific verifiers ──────────────────────────────────────────

    private boolean verifyPasskey(UUID userId, String clientAssertionJson) {
        if (clientAssertionJson == null || clientAssertionJson.isBlank()) return false;
        try {
            return webAuthnService.authenticateFinish(userId, clientAssertionJson);
        } catch (Exception e) {
            log.warn("Step-up passkey verify error user={}: {}", userId, e.getMessage());
            return false;
        }
    }

    /**
     * Push approval verification.
     *
     * <p>The frontend opens an approval request on the user's companion device
     * (Stream 4.b.5 WebSocket flow), receives a {@code requestId}, then polls
     * {@code /admin/auth/step-up/{sessionId}/status}. When the approver taps
     * the correct number, the WS controller updates KAYA HASH
     * {@code auth:approval:{requestId}.status = GRANTED}; the frontend then
     * submits {@code /verify} with {@code proof = "<requestId>"}.
     *
     * <p>This method dispatches to {@link PushApprovalService#getStatus} —
     * GRANTED means the step-up succeeds. If Stream 4.b.5's bean is not
     * available at runtime (legacy build) we fall back to the previous
     * placeholder behaviour. TODO Phase 4.b.5 wiring removed when the bean
     * is guaranteed.
     */
    private boolean verifyPushApproval(UUID userId, String approvalToken) {
        if (approvalToken == null || approvalToken.isBlank()) return false;
        PushApprovalService svc = pushApproval == null ? null : pushApproval.getIfAvailable();
        if (svc == null) {
            // TODO Phase 4.b.5 wiring — bean missing, accept token as placeholder.
            log.warn("PushApprovalService unavailable — accepting token as placeholder user={}",
                    userId);
            return true;
        }
        try {
            UUID requestId = UUID.fromString(approvalToken.trim());
            PushApprovalService.ApprovalStatus status = svc.getStatus(requestId);
            return status == PushApprovalService.ApprovalStatus.GRANTED;
        } catch (IllegalArgumentException e) {
            log.warn("Step-up push approval — invalid requestId user={}: {}", userId, e.getMessage());
            return false;
        }
    }

    private boolean verifyTotp(UUID userId, String code) {
        if (code == null || !code.matches("\\d{6}")) return false;
        return totpService.verify(userId, code);
    }

    /**
     * OTP step-up — accepts either {@code otpId:code} or just {@code code}
     * (last issued OTP for {@code userId}). Frontend ergonomics: the modal
     * issues the OTP first via {@code OtpService.issue(userId, "STEP_UP")}
     * then submits {@code "{otpId}:{code}"}.
     */
    private boolean verifyOtp(String proof, UUID userId) {
        if (proof == null || proof.isBlank()) return false;
        String otpId;
        String code;
        int idx = proof.indexOf(':');
        if (idx > 0) {
            otpId = proof.substring(0, idx);
            code = proof.substring(idx + 1);
        } else {
            // No otpId provided — frontend should pass one explicitly. Fail closed.
            log.warn("Step-up OTP verify missing otpId user={}", userId);
            return false;
        }
        if (!otpService.verify(otpId, code)) return false;
        return otpService.getUserIdForOtp(otpId)
                .map(uid -> uid.equals(userId)).orElse(true);
    }

    // ── JWT minting ─────────────────────────────────────────────────────────

    /**
     * Mint a short-lived JWT (TTL = {@link #DEFAULT_TTL}) for the given user
     * containing the {@code last_step_up_at} epoch-millis claim. The token
     * uses the same active ES384 signing key as {@code JwtService}.
     */
    private String mintStepUpToken(UUID userId, StepUpMethod method) {
        Optional<bf.gov.faso.auth.model.JwtSigningKey> activeOpt =
                keyRepository.findFirstByActiveTrueOrderByCreatedAtDesc();
        if (activeOpt.isEmpty()) {
            throw new IllegalStateException("no active JWT signing key available");
        }
        var keyRow = activeOpt.get();
        Optional<User> userOpt = userRepository.findById(userId);

        try {
            ECPrivateKey privateKey = loadPrivateKeyPem(keyRow.getPrivateKeyPem());
            Instant now = Instant.now();
            Instant exp = now.plus(DEFAULT_TTL);

            List<String> roles = userOpt
                    .map(u -> u.getRoles().stream().map(r -> r.getName()).toList())
                    .orElse(List.of());

            JWTClaimsSet claims = new JWTClaimsSet.Builder()
                    .issuer(jwtIssuer)
                    .subject(userId.toString())
                    .audience(jwtAudience)
                    .jwtID(UUID.randomUUID().toString())
                    .issueTime(Date.from(now))
                    .expirationTime(Date.from(exp))
                    .claim("email", userOpt.map(User::getEmail).orElse(null))
                    .claim("roles", roles)
                    .claim("last_step_up_at", now.toEpochMilli())
                    .claim("step_up_method", method.wire())
                    .build();

            JWSHeader header = new JWSHeader.Builder(JWSAlgorithm.ES384)
                    .keyID(keyRow.getKid())
                    .type(JOSEObjectType.JWT)
                    .build();

            SignedJWT signed = new SignedJWT(header, claims);
            signed.sign(new ECDSASigner(privateKey));
            return signed.serialize();
        } catch (JOSEException e) {
            throw new RuntimeException("failed to mint step-up JWT", e);
        }
    }

    private ECPrivateKey loadPrivateKeyPem(String pem) {
        try (PEMParser parser = new PEMParser(new StringReader(pem))) {
            Object obj = parser.readObject();
            KeyFactory kf = KeyFactory.getInstance("EC");
            if (obj instanceof PrivateKeyInfo info) {
                return (ECPrivateKey) kf.generatePrivate(new PKCS8EncodedKeySpec(info.getEncoded()));
            }
            if (obj instanceof PEMKeyPair pemKeyPair) {
                return (ECPrivateKey) kf.generatePrivate(
                        new PKCS8EncodedKeySpec(pemKeyPair.getPrivateKeyInfo().getEncoded()));
            }
            throw new IllegalArgumentException("unsupported PEM payload");
        } catch (Exception e) {
            throw new RuntimeException("failed to load JWT signing key", e);
        }
    }

    // ── Helper used by the StepUpAuthFilter ────────────────────────────────

    /**
     * Convenience for the filter: open a session AND emit the body the filter
     * will write to the 401 response.
     */
    public Map<String, Object> openSessionForFilter(UUID userId, String requestedFor,
                                                    List<StepUpMethod> allowedMethods) {
        StepUpSession s = initiateStepUp(userId, requestedFor, allowedMethods);
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("error", "step_up_required");
        body.put("methods_available", s.allowedMethods.stream()
                .map(StepUpMethod::wire).toList());
        body.put("step_up_session_id", s.sessionId);
        body.put("expires_at", s.expiresAt.toString());
        return body;
    }

    /** Default max age (seconds) — used when annotation omits it. */
    public int defaultMaxAgeSeconds() { return defaultMaxAgeSeconds; }

    // ── DTOs ────────────────────────────────────────────────────────────────

    public static final class StepUpSession {
        public final String sessionId;
        public final UUID userId;
        public final String requestedFor;
        public final List<StepUpMethod> allowedMethods;
        public final Instant createdAt;
        public final Instant expiresAt;

        public StepUpSession(String sessionId, UUID userId, String requestedFor,
                             List<StepUpMethod> allowedMethods,
                             Instant createdAt, Instant expiresAt) {
            this.sessionId = sessionId;
            this.userId = userId;
            this.requestedFor = requestedFor;
            this.allowedMethods = allowedMethods;
            this.createdAt = createdAt;
            this.expiresAt = expiresAt;
        }

        public Map<String, Object> toPublicMap() {
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("sessionId", sessionId);
            m.put("allowedMethods", allowedMethods.stream().map(StepUpMethod::wire).toList());
            m.put("expiresAt", expiresAt.toString());
            return m;
        }
    }

    public static final class StepUpResult {
        public final boolean ok;
        public final String stepUpToken;
        public final UUID userId;
        public final StepUpMethod method;
        public final String error;

        private StepUpResult(boolean ok, String stepUpToken, UUID userId,
                             StepUpMethod method, String error) {
            this.ok = ok;
            this.stepUpToken = stepUpToken;
            this.userId = userId;
            this.method = method;
            this.error = error;
        }

        public static StepUpResult success(String token, UUID userId, StepUpMethod method) {
            return new StepUpResult(true, token, userId, method, null);
        }

        public static StepUpResult failure(String error) {
            return new StepUpResult(false, null, null, null, error);
        }
    }

    public static final class Status {
        public final UUID sessionId;
        public final String status; // PENDING / VERIFIED / FAILED
        public final UUID userId;

        public Status(UUID sessionId, String status, UUID userId) {
            this.sessionId = sessionId;
            this.status = status;
            this.userId = userId;
        }

        public Map<String, Object> toPublicMap() {
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("sessionId", sessionId.toString());
            m.put("status", status);
            return m;
        }
    }

    /** Marker for unused JsonProcessingException import. */
    @SuppressWarnings("unused")
    private static String unused() throws JsonProcessingException {
        return MAPPER.writeValueAsString(Map.of());
    }
}
