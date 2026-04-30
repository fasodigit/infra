// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.infra.kafka;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.kafka.core.KafkaTemplate;
import org.springframework.stereotype.Component;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;

/**
 * Typed wrapper around {@link KafkaTemplate} for the 9 admin topics.
 * <p>
 * Phase 4.b iteration 1: JSON payloads (see {@code AdminTopics}). Iteration 2
 * will switch to Avro POJOs generated from {@code .avsc} schemas via
 * {@code avro-maven-plugin} (registered in Schema Registry :18081).
 * <p>
 * Producer is idempotent + acks=all (see {@link KafkaProducerConfig}); failures
 * after retry exhaustion are logged at ERROR but do not block the transaction
 * — callers should treat publishing as best-effort and reconcile via the
 * outbox pattern if strict at-least-once is required.
 */
@Component
public class AdminEventProducer {

    private static final Logger log = LoggerFactory.getLogger(AdminEventProducer.class);

    /** Optional template — autowired only when spring-kafka beans exist. */
    private final KafkaTemplate<String, Object> kafkaTemplate;

    @Autowired(required = false)
    public AdminEventProducer(KafkaTemplate<String, Object> kafkaTemplate) {
        this.kafkaTemplate = kafkaTemplate;
    }

    // ── OTP ──────────────────────────────────────────────────────────────────

    public void publishOtpIssued(UUID userId, String otpId, String method, String email) {
        publish(AdminTopics.OTP_ISSUE, userId.toString(), event("otp.issued", Map.of(
                "userId", userId.toString(),
                "otpId", otpId,
                "method", method,
                "email", email
        )));
    }

    public void publishOtpVerified(UUID userId, String otpId, boolean success) {
        publish(AdminTopics.OTP_VERIFIED, userId.toString(), event("otp.verified", Map.of(
                "userId", userId.toString(),
                "otpId", otpId,
                "success", success
        )));
    }

    // ── Roles & grants ───────────────────────────────────────────────────────

    public void publishRoleGranted(UUID granteeId, String roleName, UUID approverId) {
        publish(AdminTopics.ROLE_GRANTED, granteeId.toString(), event("role.granted", Map.of(
                "granteeId", granteeId.toString(),
                "roleName", roleName,
                "approverId", approverId == null ? "" : approverId.toString()
        )));
    }

    public void publishRoleRevoked(UUID granteeId, String roleName, UUID actorId) {
        publish(AdminTopics.ROLE_REVOKED, granteeId.toString(), event("role.revoked", Map.of(
                "granteeId", granteeId.toString(),
                "roleName", roleName,
                "actorId", actorId == null ? "" : actorId.toString()
        )));
    }

    // ── Device trust ─────────────────────────────────────────────────────────

    public void publishDeviceTrusted(UUID userId, String fingerprint) {
        publish(AdminTopics.DEVICE_TRUSTED, userId.toString(), event("device.trusted", Map.of(
                "userId", userId.toString(),
                "fingerprint", fingerprint
        )));
    }

    // ── Sessions ─────────────────────────────────────────────────────────────

    public void publishSessionRevoked(UUID userId, String jti, String reason) {
        publish(AdminTopics.SESSION_REVOKED, userId.toString(), event("session.revoked", Map.of(
                "userId", userId.toString(),
                "jti", jti,
                "reason", reason == null ? "manual" : reason
        )));
    }

    // ── Break-glass ──────────────────────────────────────────────────────────

    public void publishBreakGlassActivated(UUID userId, String capability,
                                           String justification, long ttlSeconds) {
        publish(AdminTopics.BREAK_GLASS_ACTIVATED, userId.toString(),
                event("break_glass.activated", Map.of(
                        "userId", userId.toString(),
                        "capability", capability,
                        "justification", justification,
                        "ttlSeconds", ttlSeconds
                )));
    }

    // ── Settings ─────────────────────────────────────────────────────────────

    public void publishSettingsChanged(String key, long version,
                                       String oldValueJson, String newValueJson,
                                       UUID changedBy, String motif) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("key", key);
        payload.put("version", version);
        payload.put("oldValue", oldValueJson);
        payload.put("newValue", newValueJson);
        payload.put("changedBy", changedBy == null ? "" : changedBy.toString());
        payload.put("motif", motif);
        publish(AdminTopics.SETTINGS_CHANGED, key, event("settings.changed", payload));
    }

    // ── User lifecycle ───────────────────────────────────────────────────────

    public void publishUserSuspended(UUID userId, UUID actorId, String reason) {
        publish(AdminTopics.USER_SUSPENDED, userId.toString(), event("user.suspended", Map.of(
                "userId", userId.toString(),
                "actorId", actorId == null ? "" : actorId.toString(),
                "reason", reason == null ? "" : reason
        )));
    }

    public void publishUserReactivated(UUID userId, UUID actorId) {
        publish(AdminTopics.USER_REACTIVATED, userId.toString(), event("user.reactivated", Map.of(
                "userId", userId.toString(),
                "actorId", actorId == null ? "" : actorId.toString()
        )));
    }

    // ── Account recovery (delta amendment 2026-04-30) ───────────────────────

    public void publishRecoverySelfInitiated(UUID userId, String email, String requestId) {
        publishRecoverySelfInitiated(userId, email, requestId, null, 30, null, null, null);
    }

    /**
     * Phase 4.b.4 — extended payload now carries the magic-link URL
     * (channel-binding) so notifier-ms can build the recovery email body. The
     * legacy 8-digit token used for admin-initiated recovery still flows via
     * {@link #publishRecoveryAdminInitiated(UUID, String, UUID, String, String, String)}.
     */
    public void publishRecoverySelfInitiated(UUID userId, String email, String requestId,
                                             String recoveryLink, long expiresInMinutes,
                                             String userFirstName, String ipAddress,
                                             String userAgent) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("userId", userId.toString());
        payload.put("userEmail", email == null ? "" : email);
        payload.put("email", email == null ? "" : email);
        payload.put("requestId", requestId);
        if (recoveryLink != null) payload.put("recoveryLink", recoveryLink);
        payload.put("expiresInMinutes", expiresInMinutes);
        if (userFirstName != null) payload.put("userFirstName", userFirstName);
        if (ipAddress != null) payload.put("ipAddress", ipAddress);
        if (userAgent != null) payload.put("userAgent", userAgent);
        publish(AdminTopics.RECOVERY_SELF_INITIATED, userId.toString(),
                event("recovery.self_initiated", payload));
    }

    public void publishRecoveryAdminInitiated(UUID targetUserId, String targetEmail,
                                              UUID initiatorId, String requestId,
                                              String token, String motif) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("userId", targetUserId.toString());
        payload.put("email", targetEmail == null ? "" : targetEmail);
        payload.put("initiatorId", initiatorId == null ? "" : initiatorId.toString());
        payload.put("requestId", requestId);
        // Token is plain text — only consumed by notifier-ms; never logged.
        payload.put("token", token);
        payload.put("motif", motif == null ? "" : motif);
        publish(AdminTopics.RECOVERY_ADMIN_INITIATED, targetUserId.toString(),
                event("recovery.admin_initiated", payload));
    }

    public void publishRecoveryCompleted(UUID userId, String recoveryType, String requestId) {
        publish(AdminTopics.RECOVERY_COMPLETED, userId.toString(),
                event("recovery.completed", Map.of(
                        "userId", userId.toString(),
                        "recoveryType", recoveryType,
                        "requestId", requestId
                )));
    }

    public void publishRecoveryUsed(UUID userId, String requestId) {
        publish(AdminTopics.RECOVERY_USED, userId.toString(),
                event("recovery.used", Map.of(
                        "userId", userId.toString(),
                        "requestId", requestId
                )));
    }

    // ── Capability grants (delta amendment 2026-04-30) ──────────────────────

    public void publishCapabilityGranted(UUID userId, String capabilityKey,
                                         String forRole, UUID grantorId, String motif) {
        publish(AdminTopics.CAPABILITY_GRANTED, userId.toString(),
                event("capability.granted", Map.of(
                        "userId", userId.toString(),
                        "capability", capabilityKey,
                        "forRole", forRole == null ? "" : forRole,
                        "grantorId", grantorId == null ? "" : grantorId.toString(),
                        "motif", motif == null ? "" : motif
                )));
    }

    public void publishCapabilityRevoked(UUID userId, String capabilityKey,
                                         UUID actorId, String motif) {
        publish(AdminTopics.CAPABILITY_REVOKED, userId.toString(),
                event("capability.revoked", Map.of(
                        "userId", userId.toString(),
                        "capability", capabilityKey,
                        "actorId", actorId == null ? "" : actorId.toString(),
                        "motif", motif == null ? "" : motif
                )));
    }

    // ── Push approval (Phase 4.b.5) ──────────────────────────────────────────

    public void publishPushApprovalRequested(UUID userId, UUID requestId,
                                              bf.gov.faso.auth.service.admin.PushApprovalService.LoginContext ctx) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("userId", userId.toString());
        payload.put("requestId", requestId.toString());
        payload.put("ip", ctx.ip());
        payload.put("ua", ctx.userAgent());
        payload.put("city", ctx.city() != null ? ctx.city() : "");
        publish(AdminTopics.AUTH_PUSH_REQUESTED, requestId.toString(),
                event("push_approval.requested", payload));
    }

    public void publishPushApprovalGranted(UUID requestId, UUID userId) {
        publish(AdminTopics.AUTH_PUSH_GRANTED, requestId.toString(),
                event("push_approval.granted", Map.of(
                        "requestId", requestId.toString(),
                        "userId", userId.toString()
                )));
    }

    public void publishPushApprovalNumberMismatch(UUID requestId, UUID userId,
                                                   int chosenNumber, int correctNumber) {
        publish(AdminTopics.AUTH_PUSH_NUMBER_MISMATCH, requestId.toString(),
                event("push_approval.number_mismatch", Map.of(
                        "requestId", requestId.toString(),
                        "userId", userId.toString(),
                        "chosenNumber", chosenNumber,
                        "correctNumber", correctNumber
                )));
    }

    public void publishPushApprovalTimeout(UUID requestId, UUID userId) {
        publish(AdminTopics.AUTH_PUSH_TIMEOUT, requestId.toString(),
                event("push_approval.timeout", Map.of(
                        "requestId", requestId.toString(),
                        "userId", userId.toString()
                )));
    }

    public void publishPushApprovalDenied(UUID requestId, UUID userId) {
        publish(AdminTopics.AUTH_PUSH_DENIED, requestId.toString(),
                event("push_approval.denied", Map.of(
                        "requestId", requestId.toString(),
                        "userId", userId.toString()
                )));
    }

    // ── Onboarding (Phase 4.b.4 — magic-link channel-binding) ───────────────

    /**
     * Published when a SUPER-ADMIN issues an invitation magic-link. The
     * notifier-ms {@code OnboardingEventConsumer} renders the
     * {@code admin/admin-onboard-magic-link.hbs} template and dispatches the
     * email. The {@code magicLink} contains the signed JWT — never logged at
     * INFO/DEBUG outside of the consumer itself.
     */
    public void publishOnboardInvitation(UUID invitationId,
                                         String targetEmail,
                                         String targetRole,
                                         String inviterName,
                                         UUID inviterId,
                                         String magicLink,
                                         long expiresInMinutes,
                                         String ipAddress,
                                         String lang) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("invitationId", invitationId.toString());
        payload.put("userEmail", targetEmail == null ? "" : targetEmail);
        payload.put("targetRole", targetRole == null ? "" : targetRole);
        payload.put("inviterName", inviterName == null ? "" : inviterName);
        payload.put("inviterId", inviterId == null ? "" : inviterId.toString());
        payload.put("magicLink", magicLink);
        payload.put("expiresInMinutes", expiresInMinutes);
        payload.put("ipAddress", ipAddress == null ? "" : ipAddress);
        payload.put("lang", lang == null ? "fr" : lang);
        publish(AdminTopics.AUTH_ONBOARD_INVITATION_SENT, invitationId.toString(),
                event("onboard.invitation_sent", payload));
    }

    /**
     * Published once the target completes the magic-link → OTP → MFA-enroll
     * funnel. Consumers (audit fan-out, notifier confirmation copy) listen.
     */
    public void publishOnboardCompleted(UUID invitationId,
                                        UUID userId,
                                        String email,
                                        String sessionId) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("invitationId", invitationId == null ? "" : invitationId.toString());
        payload.put("userId", userId == null ? "" : userId.toString());
        payload.put("userEmail", email == null ? "" : email);
        payload.put("sessionId", sessionId == null ? "" : sessionId);
        publish(AdminTopics.AUTH_ONBOARD_COMPLETED,
                userId == null ? UUID.randomUUID().toString() : userId.toString(),
                event("onboard.completed", payload));
    }

    // ── Risk scoring (Phase 4.b.6) ───────────────────────────────────────────

    /**
     * Publishes a per-login risk assessment from
     * {@link bf.gov.faso.auth.service.admin.RiskScoringService} to
     * {@link AdminTopics#AUTH_RISK_ASSESSED}. Consumed by analytics, threat
     * intel, and notifier-ms (alerts on STEP_UP / BLOCK).
     */
    public void publishRiskAssessed(UUID userId, int score, String decision,
                                    String ipAddress, String country,
                                    java.util.List<Map<String, Object>> signals,
                                    String loginHistoryId) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("userId", userId.toString());
        payload.put("score", score);
        payload.put("decision", decision);
        payload.put("ip", ipAddress == null ? "" : ipAddress);
        payload.put("country", country == null ? "" : country);
        payload.put("signals", signals == null ? java.util.List.of() : signals);
        payload.put("loginHistoryId", loginHistoryId == null ? "" : loginHistoryId);
        publish(AdminTopics.AUTH_RISK_ASSESSED, userId.toString(),
                event("risk.assessed", payload));
    }

    /**
     * Subset publication for {@link AdminTopics#AUTH_RISK_BLOCKED} — only emitted
     * when the decision is BLOCK. SIEM-friendly (1 partition, 90d retention).
     */
    public void publishRiskBlocked(UUID userId, int score, String ipAddress,
                                   String country, String reason) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("userId", userId.toString());
        payload.put("score", score);
        payload.put("ip", ipAddress == null ? "" : ipAddress);
        payload.put("country", country == null ? "" : country);
        payload.put("reason", reason == null ? "" : reason);
        publish(AdminTopics.AUTH_RISK_BLOCKED, userId.toString(),
                event("risk.blocked", payload));
    }

    // ── Step-up auth (Phase 4.b.7) ──────────────────────────────────────────

    public void publishStepUpRequested(UUID userId, String sessionId, String requestedFor) {
        publish(AdminTopics.AUTH_STEP_UP_REQUESTED, userId.toString(),
                event("step_up.requested", Map.of(
                        "userId", userId.toString(),
                        "sessionId", sessionId,
                        "requestedFor", requestedFor == null ? "" : requestedFor
                )));
    }

    public void publishStepUpVerified(UUID userId, String sessionId, String method) {
        publish(AdminTopics.AUTH_STEP_UP_VERIFIED, userId.toString(),
                event("step_up.verified", Map.of(
                        "userId", userId.toString(),
                        "sessionId", sessionId,
                        "method", method == null ? "" : method
                )));
    }

    public void publishStepUpFailed(UUID userId, String sessionId, String method, String reason) {
        publish(AdminTopics.AUTH_STEP_UP_FAILED, userId.toString(),
                event("step_up.failed", Map.of(
                        "userId", userId.toString(),
                        "sessionId", sessionId,
                        "method", method == null ? "" : method,
                        "reason", reason == null ? "" : reason
                )));
    }

    // ── Audit (sink for AdminAuditService when fan-out is enabled) ──────────

    public void publishAuditEvent(Map<String, Object> payload) {
        Object id = payload.getOrDefault("id", UUID.randomUUID().toString());
        publish(AdminTopics.AUDIT_EVENT, id.toString(), event("audit.event", payload));
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    private Map<String, Object> event(String type, Map<String, Object> payload) {
        Map<String, Object> envelope = new LinkedHashMap<>();
        envelope.put("eventType", type);
        envelope.put("eventId", UUID.randomUUID().toString());
        envelope.put("publishedAt", Instant.now().toString());
        envelope.put("payload", payload);
        return envelope;
    }

    private void publish(String topic, String key, Map<String, Object> envelope) {
        if (kafkaTemplate == null) {
            log.warn("KafkaTemplate not available — skipping publish to topic={} key={}", topic, key);
            return;
        }
        try {
            CompletableFuture<?> future = kafkaTemplate.send(topic, key, envelope);
            future.whenComplete((meta, ex) -> {
                if (ex != null) {
                    log.error("Kafka publish FAILED topic={} key={}: {}", topic, key, ex.getMessage());
                } else {
                    log.debug("Kafka publish OK topic={} key={}", topic, key);
                }
            });
        } catch (Exception e) {
            log.error("Kafka publish threw synchronously topic={} key={}: {}", topic, key, e.getMessage());
        }
    }
}
