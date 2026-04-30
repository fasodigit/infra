// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.infra.kafka.AdminTopics;
import bf.gov.faso.auth.service.admin.AdminSettingsService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;

import java.io.IOException;
import java.security.SecureRandom;
import java.time.Duration;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Collectors;

/**
 * Push-approval service for Phase 4.b.5 — sovereign WebSocket MFA.
 *
 * <h2>Number-matching anti-MFA-bombing</h2>
 * When a login request triggers push approval the server:
 * <ol>
 *   <li>Picks a {@code displayedNumber} (0–9) shown to the login browser.</li>
 *   <li>Generates two decoy numbers (different from the correct one).</li>
 *   <li>Shuffles the three numbers into a random order for the modal.</li>
 *   <li>Stores {@code correctNumber} server-side in KAYA (TTL 30 s).</li>
 *   <li>The approving device taps one number; only the correct match grants.</li>
 * </ol>
 *
 * <h2>KAYA key schema</h2>
 * {@code auth:approval:{requestId}} — HASH, TTL 30 s.
 * Fields: {@code userId, displayedNumber, correctNumber, phoneNumbers,
 * status, ip, ua, city}.
 *
 * <h2>Failure modes</h2>
 * <ul>
 *   <li>User has no live WS session → returns {@code available=false, fallback=OTP}.</li>
 *   <li>KAYA unavailable → {@code requestApproval} throws; caller falls back to OTP.</li>
 *   <li>TTL expired → {@code respondApproval} returns {@code TIMEOUT},
 *       publishes {@code auth.push.timeout}, caller triggers OTP fallback.</li>
 *   <li>Number mismatch → {@code DENIED}, publishes {@code auth.push.number_mismatch}.</li>
 * </ul>
 */
@Service
public class PushApprovalService {

    private static final Logger log = LoggerFactory.getLogger(PushApprovalService.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** TTL for each approval request in KAYA. */
    private static final Duration APPROVAL_TTL = Duration.ofSeconds(30);

    /** KAYA key prefix for approval state. */
    private static final String KAYA_PREFIX = "auth:approval:";

    // ── in-memory WS session registry ─────────────────────────────────────────

    /**
     * Live WebSocket sessions per userId.
     *
     * Thread-safety: ConcurrentHashMap + CopyOnWriteArraySet-like
     * {@code ConcurrentHashMap<UUID, ConcurrentHashMap<String, WebSocketSession>>}
     * (inner map keyed by session ID to allow easy removal on disconnect).
     */
    private final ConcurrentHashMap<UUID, ConcurrentHashMap<String, WebSocketSession>> sessions
            = new ConcurrentHashMap<>();

    private final StringRedisTemplate redis;
    private final AdminEventProducer eventProducer;
    private final AdminSettingsService settingsService;
    private final SecureRandom rng = new SecureRandom();

    @Autowired
    public PushApprovalService(StringRedisTemplate redis,
                                AdminEventProducer eventProducer,
                                AdminSettingsService settingsService) {
        this.redis = redis;
        this.eventProducer = eventProducer;
        this.settingsService = settingsService;
    }

    // ── session registry ───────────────────────────────────────────────────────

    /**
     * Register a WebSocket session for a user.
     * Called from {@link bf.gov.faso.auth.controller.admin.PushApprovalWsController}
     * on handshake.
     */
    public void registerSession(UUID userId, WebSocketSession session) {
        sessions.computeIfAbsent(userId, k -> new ConcurrentHashMap<>())
                .put(session.getId(), session);
        log.info("push-approval: WS session registered userId={} sessionId={} totalSessions={}",
                userId, session.getId(),
                sessions.getOrDefault(userId, new ConcurrentHashMap<>()).size());
    }

    /**
     * Remove a WebSocket session on close / error.
     */
    public void unregisterSession(UUID userId, WebSocketSession session) {
        ConcurrentHashMap<String, WebSocketSession> userSessions = sessions.get(userId);
        if (userSessions != null) {
            userSessions.remove(session.getId());
            if (userSessions.isEmpty()) {
                sessions.remove(userId, userSessions);
            }
        }
        log.info("push-approval: WS session unregistered userId={} sessionId={}",
                userId, session.getId());
    }

    /**
     * Returns {@code true} if the user has at least one live WS session.
     */
    public boolean hasActiveSessions(UUID userId) {
        ConcurrentHashMap<String, WebSocketSession> userSessions = sessions.get(userId);
        if (userSessions == null || userSessions.isEmpty()) return false;
        // Prune sessions that are no longer open.
        userSessions.values().removeIf(s -> !s.isOpen());
        return !userSessions.isEmpty();
    }

    // ── approval lifecycle ─────────────────────────────────────────────────────

    /**
     * Initiate a push-approval request for the given user.
     *
     * @param userId       UUID of the user being authenticated.
     * @param loginContext metadata about the login attempt (ip, ua, city).
     * @return an {@link ApprovalRequest} if the user has live WS sessions,
     *         or an unavailable result indicating OTP fallback.
     */
    public ApprovalRequest requestApproval(UUID userId, LoginContext loginContext) {
        // Guard: setting must be enabled.
        boolean enabled = settingsService.getByKey("mfa.push_approval_enabled")
                .map(s -> Boolean.parseBoolean(s.getValue()))
                .orElse(true);
        if (!enabled) {
            return ApprovalRequest.unavailable();
        }

        // Guard: user must have live WS sessions.
        if (!hasActiveSessions(userId)) {
            log.debug("push-approval: no active WS sessions for userId={}", userId);
            return ApprovalRequest.unavailable();
        }

        // Generate number-matching data.
        UUID requestId = UUID.randomUUID();
        int displayedNumber = rng.nextInt(10); // 0–9, shown on login tab
        int correctNumber = displayedNumber;   // the number the approver must tap

        // Generate 2 unique decoy numbers.
        List<Integer> decoys = new ArrayList<>();
        while (decoys.size() < 2) {
            int candidate = rng.nextInt(10);
            if (candidate != correctNumber && !decoys.contains(candidate)) {
                decoys.add(candidate);
            }
        }

        // Build 3-number list and shuffle for display in the modal.
        List<Integer> phoneNumbers = new ArrayList<>(List.of(correctNumber, decoys.get(0), decoys.get(1)));
        Collections.shuffle(phoneNumbers, rng);

        Instant expiresAt = Instant.now().plus(APPROVAL_TTL);

        // Persist in KAYA.
        String key = KAYA_PREFIX + requestId;
        Map<String, String> hash = new LinkedHashMap<>();
        hash.put("userId", userId.toString());
        hash.put("displayedNumber", String.valueOf(displayedNumber));
        hash.put("correctNumber", String.valueOf(correctNumber));
        hash.put("phoneNumbers", phoneNumbers.stream().map(String::valueOf).collect(Collectors.joining(",")));
        hash.put("status", "PENDING");
        hash.put("ip", loginContext.ip());
        hash.put("ua", loginContext.userAgent());
        hash.put("city", loginContext.city() != null ? loginContext.city() : "");
        hash.put("expiresAt", expiresAt.toString());

        redis.opsForHash().putAll(key, hash);
        redis.expire(key, APPROVAL_TTL);

        // Push message to all live WS sessions.
        Map<String, Object> wsMessage = new LinkedHashMap<>();
        wsMessage.put("type", "approval-request");
        wsMessage.put("requestId", requestId.toString());
        wsMessage.put("numbers", phoneNumbers);
        wsMessage.put("ip", loginContext.ip());
        wsMessage.put("ua", loginContext.userAgent());
        wsMessage.put("city", loginContext.city() != null ? loginContext.city() : "");
        wsMessage.put("expiresAt", expiresAt.toEpochMilli());

        pushToSessions(userId, wsMessage);

        // Publish Redpanda event.
        eventProducer.publishPushApprovalRequested(userId, requestId, loginContext);

        log.info("push-approval: request created requestId={} userId={} ip={}",
                requestId, userId, loginContext.ip());

        return ApprovalRequest.available(requestId, displayedNumber, expiresAt);
    }

    /**
     * Process the user's response to a push-approval request.
     *
     * @param requestId    the UUID of the approval request.
     * @param chosenNumber the number tapped by the approving user.
     * @param userId       the UUID of the responding user (must match the request).
     * @return an {@link ApprovalResult}.
     */
    public ApprovalResult respondApproval(UUID requestId, int chosenNumber, UUID userId) {
        String key = KAYA_PREFIX + requestId;

        // Load state from KAYA.
        Map<Object, Object> raw = redis.opsForHash().entries(key);
        if (raw.isEmpty()) {
            // Key expired or never existed.
            log.warn("push-approval: requestId={} not found in KAYA — treating as TIMEOUT", requestId);
            eventProducer.publishPushApprovalTimeout(requestId, userId);
            return ApprovalResult.ofTimeout();
        }

        String storedUserId = (String) raw.get("userId");
        String status = (String) raw.get("status");
        String correctNumberStr = (String) raw.get("correctNumber");

        // Validate userId.
        if (!userId.toString().equals(storedUserId)) {
            log.warn("push-approval: userId mismatch requestId={} expected={} got={}",
                    requestId, storedUserId, userId);
            return ApprovalResult.ofDenied();
        }

        // Already processed.
        if (!"PENDING".equals(status)) {
            log.warn("push-approval: requestId={} already in status={}", requestId, status);
            return ApprovalResult.ofDenied();
        }

        int correctNumber = Integer.parseInt(correctNumberStr);

        if (chosenNumber == correctNumber) {
            redis.opsForHash().put(key, "status", "GRANTED");
            eventProducer.publishPushApprovalGranted(requestId, userId);
            log.info("push-approval: GRANTED requestId={} userId={}", requestId, userId);
            return ApprovalResult.ofGranted();
        } else {
            redis.opsForHash().put(key, "status", "DENIED");
            eventProducer.publishPushApprovalNumberMismatch(requestId, userId, chosenNumber, correctNumber);
            log.warn("push-approval: NUMBER_MISMATCH requestId={} userId={} chosen={} correct={}",
                    requestId, userId, chosenNumber, correctNumber);
            return ApprovalResult.ofDenied();
        }
    }

    /**
     * Poll the status of an approval request (fallback for clients without WS).
     */
    public ApprovalStatus getStatus(UUID requestId) {
        String key = KAYA_PREFIX + requestId;
        Object status = redis.opsForHash().get(key, "status");
        if (status == null) {
            return ApprovalStatus.TIMEOUT;
        }
        return switch (status.toString()) {
            case "GRANTED" -> ApprovalStatus.GRANTED;
            case "DENIED"  -> ApprovalStatus.DENIED;
            case "PENDING" -> ApprovalStatus.PENDING;
            default        -> ApprovalStatus.TIMEOUT;
        };
    }

    // ── private helpers ────────────────────────────────────────────────────────

    private void pushToSessions(UUID userId, Map<String, Object> message) {
        ConcurrentHashMap<String, WebSocketSession> userSessions = sessions.get(userId);
        if (userSessions == null) return;

        String json;
        try {
            json = MAPPER.writeValueAsString(message);
        } catch (IOException e) {
            log.error("push-approval: could not serialise WS message: {}", e.getMessage());
            return;
        }

        TextMessage textMessage = new TextMessage(json);
        userSessions.values().forEach(s -> {
            if (s.isOpen()) {
                try {
                    s.sendMessage(textMessage);
                    log.debug("push-approval: pushed message to sessionId={}", s.getId());
                } catch (IOException e) {
                    log.warn("push-approval: failed to push to sessionId={}: {}", s.getId(), e.getMessage());
                }
            }
        });
    }

    // ── value types ───────────────────────────────────────────────────────────

    /** Login context metadata attached to the push-approval request. */
    public record LoginContext(String ip, String userAgent, String city) {}

    /** Result of initiating a push-approval. */
    public record ApprovalRequest(
            boolean available,
            UUID requestId,
            Integer displayedNumber,
            Instant expiresAt,
            String fallback
    ) {
        public static ApprovalRequest available(UUID requestId, int displayedNumber, Instant expiresAt) {
            return new ApprovalRequest(true, requestId, displayedNumber, expiresAt, null);
        }

        public static ApprovalRequest unavailable() {
            return new ApprovalRequest(false, null, null, null, "OTP");
        }
    }

    /** Result of responding to a push-approval. */
    public record ApprovalResult(
            boolean granted,
            ApprovalStatus status,
            String mfaProof
    ) {
        public static ApprovalResult ofGranted() {
            // mfaProof would be a short-lived JWT claim in production.
            return new ApprovalResult(true, ApprovalStatus.GRANTED, "push-approved");
        }

        public static ApprovalResult ofDenied() {
            return new ApprovalResult(false, ApprovalStatus.DENIED, null);
        }

        public static ApprovalResult ofTimeout() {
            return new ApprovalResult(false, ApprovalStatus.TIMEOUT, null);
        }
    }

    /** Status values stored in KAYA and returned by {@link #getStatus}. */
    public enum ApprovalStatus {
        PENDING, GRANTED, DENIED, TIMEOUT
    }
}
