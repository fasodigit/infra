// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.PushApprovalService;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Component;
import org.springframework.web.socket.*;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import java.util.List;
import java.util.UUID;

/**
 * WebSocket handler for the push-approval flow (Phase 4.b.5).
 *
 * <h2>Protocol (text frames, JSON)</h2>
 *
 * <p>Client → Server frames:</p>
 * <pre>
 * { "type": "connect",  "userId": "<uuid>" }
 * { "type": "respond",  "requestId": "<uuid>", "chosenNumber": 7 }
 * </pre>
 *
 * <p>Server → Client frames (pushed by {@link PushApprovalService}):</p>
 * <pre>
 * { "type": "approval-request", "requestId": "...", "numbers": [3,7,21],
 *   "ip": "...", "ua": "...", "city": "...", "expiresAt": <epoch_ms> }
 * { "type": "approval-result",  "requestId": "...", "granted": true|false }
 * { "type": "error",            "reason": "..." }
 * </pre>
 *
 * <h2>Authentication</h2>
 * The ARMAGEDDON WS proxy injects {@code X-User-Id} into the HTTP upgrade
 * headers before the handshake reaches auth-ms.  The handler trusts this
 * header because ARMAGEDDON has already validated the JWT.
 *
 * <h2>Failure modes</h2>
 * <ul>
 *   <li>Missing {@code X-User-Id} header → session closed with 4401.</li>
 *   <li>Unknown / malformed frame type → error frame returned.</li>
 *   <li>Session closed abnormally → unregister called in {@code afterConnectionClosed}.</li>
 * </ul>
 */
@Component
public class PushApprovalWsController extends TextWebSocketHandler {

    private static final Logger log = LoggerFactory.getLogger(PushApprovalWsController.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /**
     * Header injected by ARMAGEDDON after JWT validation.
     * Trusted because it comes from the server-side proxy, not the client.
     */
    private static final String USER_ID_HEADER = "X-User-Id";

    private final PushApprovalService pushApprovalService;

    @Autowired
    public PushApprovalWsController(PushApprovalService pushApprovalService) {
        this.pushApprovalService = pushApprovalService;
    }

    // ── lifecycle ──────────────────────────────────────────────────────────────

    @Override
    public void afterConnectionEstablished(WebSocketSession session) throws Exception {
        String userId = extractUserId(session);
        if (userId == null) {
            log.warn("push-approval: WS handshake without X-User-Id header — closing");
            session.close(new CloseStatus(4401, "missing_user_id"));
            return;
        }

        try {
            UUID userUuid = UUID.fromString(userId);
            pushApprovalService.registerSession(userUuid, session);
            log.info("push-approval: WS connection established userId={} sessionId={}",
                    userId, session.getId());
        } catch (IllegalArgumentException e) {
            log.warn("push-approval: invalid userId UUID value='{}' — closing", userId);
            session.close(new CloseStatus(4400, "invalid_user_id"));
        }
    }

    @Override
    protected void handleTextMessage(WebSocketSession session, TextMessage message) throws Exception {
        String userId = extractUserId(session);
        if (userId == null) {
            sendError(session, "missing_user_id");
            return;
        }

        JsonNode payload;
        try {
            payload = MAPPER.readTree(message.getPayload());
        } catch (Exception e) {
            log.warn("push-approval: malformed JSON from userId={}: {}", userId, e.getMessage());
            sendError(session, "malformed_json");
            return;
        }

        String type = payload.path("type").asText("");

        switch (type) {
            case "respond" -> handleRespond(session, payload, UUID.fromString(userId));
            case "ping"    -> session.sendMessage(new TextMessage("{\"type\":\"pong\"}"));
            default -> {
                log.debug("push-approval: unknown frame type='{}' userId={}", type, userId);
                sendError(session, "unknown_frame_type");
            }
        }
    }

    @Override
    public void afterConnectionClosed(WebSocketSession session, CloseStatus status) {
        String userId = extractUserId(session);
        if (userId != null) {
            try {
                pushApprovalService.unregisterSession(UUID.fromString(userId), session);
            } catch (IllegalArgumentException ignored) {
                // invalid UUID — nothing to unregister
            }
        }
        log.info("push-approval: WS connection closed userId={} sessionId={} status={}",
                userId, session.getId(), status);
    }

    @Override
    public void handleTransportError(WebSocketSession session, Throwable exception) throws Exception {
        log.error("push-approval: WS transport error sessionId={}: {}",
                session.getId(), exception.getMessage());
        if (session.isOpen()) {
            session.close(CloseStatus.SERVER_ERROR);
        }
    }

    // ── frame handlers ─────────────────────────────────────────────────────────

    private void handleRespond(WebSocketSession session, JsonNode payload, UUID userId) throws Exception {
        JsonNode requestIdNode = payload.path("requestId");
        JsonNode chosenNumberNode = payload.path("chosenNumber");

        if (requestIdNode.isMissingNode() || chosenNumberNode.isMissingNode()) {
            sendError(session, "missing_fields");
            return;
        }

        UUID requestId;
        try {
            requestId = UUID.fromString(requestIdNode.asText());
        } catch (IllegalArgumentException e) {
            sendError(session, "invalid_request_id");
            return;
        }

        int chosenNumber = chosenNumberNode.asInt(-1);
        if (chosenNumber < 0 || chosenNumber > 9) {
            sendError(session, "invalid_chosen_number");
            return;
        }

        PushApprovalService.ApprovalResult result =
                pushApprovalService.respondApproval(requestId, chosenNumber, userId);

        var response = MAPPER.createObjectNode();
        response.put("type", "approval-result");
        response.put("requestId", requestId.toString());
        response.put("granted", result.granted());
        response.put("status", result.status().name());
        if (result.mfaProof() != null) {
            response.put("mfaProof", result.mfaProof());
        }

        session.sendMessage(new TextMessage(MAPPER.writeValueAsString(response)));
    }

    // ── helpers ────────────────────────────────────────────────────────────────

    private static String extractUserId(WebSocketSession session) {
        List<String> values = session.getHandshakeHeaders().get(USER_ID_HEADER);
        if (values == null || values.isEmpty()) return null;
        String v = values.get(0);
        return (v == null || v.isBlank()) ? null : v.trim();
    }

    private static void sendError(WebSocketSession session, String reason) throws Exception {
        if (session.isOpen()) {
            var node = MAPPER.createObjectNode();
            node.put("type", "error");
            node.put("reason", reason);
            session.sendMessage(new TextMessage(MAPPER.writeValueAsString(node)));
        }
    }
}
