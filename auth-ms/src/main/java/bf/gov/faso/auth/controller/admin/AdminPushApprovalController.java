// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.PushApprovalService;
import bf.gov.faso.auth.service.admin.PushApprovalService.ApprovalResult;
import bf.gov.faso.auth.service.admin.PushApprovalService.ApprovalRequest;
import bf.gov.faso.auth.service.admin.PushApprovalService.ApprovalStatus;
import bf.gov.faso.auth.service.admin.PushApprovalService.LoginContext;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * REST endpoints for the push-approval MFA flow (Phase 4.b.5).
 *
 * <h2>Endpoints</h2>
 * <ul>
 *   <li>{@code POST /admin/auth/push-approval/initiate} — initiate push approval.</li>
 *   <li>{@code GET  /admin/auth/push-approval/{requestId}/status} — poll status.</li>
 *   <li>{@code POST /admin/auth/push-approval/{requestId}/respond} — REST fallback respond.</li>
 * </ul>
 *
 * <p>These endpoints are called by the BFF (behind ARMAGEDDON).  The caller's
 * identity is resolved from the {@code X-Forwarded-User} header set by
 * ARMAGEDDON after JWT validation.
 *
 * <h2>Failure modes</h2>
 * <ul>
 *   <li>{@code initiate}: if no WS sessions exist → 200 with {@code available=false}.</li>
 *   <li>{@code status}: if TTL expired → 200 with {@code status=TIMEOUT}.</li>
 *   <li>{@code respond}: number mismatch → 200 with {@code granted=false}.</li>
 * </ul>
 */
@RestController
@RequestMapping("/admin/auth/push-approval")
public class AdminPushApprovalController {

    private static final Logger log = LoggerFactory.getLogger(AdminPushApprovalController.class);

    private final PushApprovalService pushApprovalService;

    @Autowired
    public AdminPushApprovalController(PushApprovalService pushApprovalService) {
        this.pushApprovalService = pushApprovalService;
    }

    /**
     * Initiate a push-approval request.
     *
     * <p>Body: {@code { "userId": "<uuid>", "ip": "...", "ua": "...", "city": "..." }}
     */
    @PostMapping("/initiate")
    public ResponseEntity<Map<String, Object>> initiate(
            @RequestBody InitiateRequest body,
            @RequestHeader(value = "X-Forwarded-User", required = false) String forwardedUser,
            @RequestHeader(value = "X-Trace-Id", defaultValue = "") String traceId
    ) {
        UUID userId;
        try {
            userId = UUID.fromString(body.userId());
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", "invalid_user_id"));
        }

        LoginContext ctx = new LoginContext(
                body.ip() != null ? body.ip() : "unknown",
                body.ua() != null ? body.ua() : "",
                body.city()
        );

        ApprovalRequest request = pushApprovalService.requestApproval(userId, ctx);

        Map<String, Object> response = new LinkedHashMap<>();
        response.put("available", request.available());
        if (request.available()) {
            response.put("requestId", request.requestId().toString());
            response.put("displayedNumber", request.displayedNumber());
            response.put("expiresAt", request.expiresAt().toEpochMilli());
        } else {
            response.put("fallback", request.fallback());
        }

        log.info("push-approval initiate: userId={} available={} traceId={}",
                userId, request.available(), traceId);

        return ResponseEntity.ok(response);
    }

    /**
     * Poll the current status of a push-approval request.
     * Used as fallback when the client cannot maintain a WebSocket.
     */
    @GetMapping("/{requestId}/status")
    public ResponseEntity<Map<String, Object>> status(@PathVariable String requestId) {
        UUID requestUuid;
        try {
            requestUuid = UUID.fromString(requestId);
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", "invalid_request_id"));
        }

        ApprovalStatus status = pushApprovalService.getStatus(requestUuid);

        Map<String, Object> response = new LinkedHashMap<>();
        response.put("requestId", requestId);
        response.put("status", status.name());
        response.put("granted", status == ApprovalStatus.GRANTED);

        return ResponseEntity.ok(response);
    }

    /**
     * REST fallback to respond to an approval request (for clients without WS).
     *
     * <p>Body: {@code { "chosenNumber": 7 }}
     */
    @PostMapping("/{requestId}/respond")
    public ResponseEntity<Map<String, Object>> respond(
            @PathVariable String requestId,
            @RequestBody RespondRequest body,
            @RequestHeader(value = "X-Forwarded-User", required = false) String forwardedUser
    ) {
        UUID requestUuid;
        try {
            requestUuid = UUID.fromString(requestId);
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", "invalid_request_id"));
        }

        if (forwardedUser == null || forwardedUser.isBlank()) {
            return ResponseEntity.status(401).body(Map.of("error", "missing_user_identity"));
        }

        UUID userId;
        try {
            userId = UUID.fromString(forwardedUser.trim());
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", "invalid_forwarded_user"));
        }

        if (body.chosenNumber() < 0 || body.chosenNumber() > 9) {
            return ResponseEntity.badRequest().body(Map.of("error", "invalid_chosen_number"));
        }

        ApprovalResult result = pushApprovalService.respondApproval(requestUuid, body.chosenNumber(), userId);

        Map<String, Object> response = new LinkedHashMap<>();
        response.put("requestId", requestId);
        response.put("granted", result.granted());
        response.put("status", result.status().name());
        if (result.mfaProof() != null) {
            response.put("mfaProof", result.mfaProof());
        }

        return ResponseEntity.ok(response);
    }

    // ── request DTOs ──────────────────────────────────────────────────────────

    public record InitiateRequest(String userId, String ip, String ua, String city) {}

    public record RespondRequest(int chosenNumber) {}
}
