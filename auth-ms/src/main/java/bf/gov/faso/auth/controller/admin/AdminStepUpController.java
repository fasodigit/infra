// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.security.StepUpMethod;
import bf.gov.faso.auth.service.admin.StepUpAuthService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Phase 4.b.7 — Step-up auth endpoints.
 *
 * <ul>
 *   <li>{@code POST /admin/auth/step-up/begin}       — open a session.</li>
 *   <li>{@code POST /admin/auth/step-up/{id}/verify} — verify proof, mint token.</li>
 *   <li>{@code GET  /admin/auth/step-up/{id}/status} — poll for async push approval.</li>
 * </ul>
 *
 * <p>The matching {@code StepUpAuthFilter} short-circuits any controller
 * method bearing {@link RequiresStepUp} when the JWT carries a fresh
 * {@code last_step_up_at}. After a successful verify, the client retries the
 * original request with the returned step-up JWT in the
 * {@code Authorization: Bearer ...} header.
 */
@RestController
@RequestMapping("/admin/auth/step-up")
public class AdminStepUpController {

    private final StepUpAuthService stepUpService;
    private final AdminAuthHelper auth;

    public AdminStepUpController(StepUpAuthService stepUpService, AdminAuthHelper auth) {
        this.stepUpService = stepUpService;
        this.auth = auth;
    }

    @PostMapping("/begin")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> begin(@RequestBody BeginRequest req) {
        UUID userId = auth.currentUserId().orElseThrow(() ->
                new ResponseStatusException(HttpStatus.UNAUTHORIZED, "JWT principal missing"));
        StepUpAuthService.StepUpSession session =
                stepUpService.initiateStepUp(userId, req.requestedFor, null);
        return ResponseEntity.ok(session.toPublicMap());
    }

    @PostMapping("/{sessionId}/verify")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> verify(
            @PathVariable UUID sessionId,
            @RequestBody VerifyRequest req) {
        if (req.method == null || req.method.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "method required");
        }
        StepUpMethod method;
        try {
            method = StepUpMethod.fromWire(req.method);
        } catch (IllegalArgumentException e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "unknown method: " + req.method);
        }
        StepUpAuthService.StepUpResult result =
                stepUpService.verifyStepUp(sessionId, method, req.proof);
        if (!result.ok) {
            Map<String, Object> body = new LinkedHashMap<>();
            body.put("error", "step_up_failed");
            body.put("reason", result.error);
            return ResponseEntity.status(HttpStatus.FORBIDDEN).body(body);
        }
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("stepUpToken", result.stepUpToken);
        body.put("method", result.method.wire());
        body.put("expiresInSeconds", StepUpAuthService.DEFAULT_TTL.toSeconds());
        return ResponseEntity.ok(body);
    }

    @GetMapping("/{sessionId}/status")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> status(@PathVariable UUID sessionId) {
        return stepUpService.getStepUpStatus(sessionId)
                .map(s -> ResponseEntity.ok(s.toPublicMap()))
                .orElseGet(() -> ResponseEntity.status(HttpStatus.NOT_FOUND)
                        .body(Map.of("error", "session_not_found")));
    }

    // ── DTOs ───────────────────────────────────────────────────────────────

    public static class BeginRequest {
        /** A free-form descriptor of the original request, e.g. "POST /admin/grants/request". */
        @NotBlank public String requestedFor;
    }

    public static class VerifyRequest {
        /** Wire-format method: passkey / push-approval / totp / otp. */
        @NotBlank public String method;
        /** Method-dependent proof — JSON assertion / requestId / digit-code / "{otpId}:{code}". */
        @NotBlank public String proof;
    }

    /** Fluent builder for use by tests (kept for API completeness). */
    @SuppressWarnings("unused")
    private static List<StepUpMethod> defaultMethods() {
        return List.of(StepUpMethod.PASSKEY, StepUpMethod.PUSH_APPROVAL,
                StepUpMethod.TOTP, StepUpMethod.OTP);
    }
}
