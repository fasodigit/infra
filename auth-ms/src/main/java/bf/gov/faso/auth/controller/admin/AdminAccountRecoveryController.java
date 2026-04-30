// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.AccountRecoveryService;
import bf.gov.faso.auth.service.admin.OtpService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Account recovery endpoints (delta amendment 2026-04-30 §5).
 *
 * <p>Two paths:
 * <ul>
 *   <li>{@code POST /admin/auth/recovery/initiate}     — public, self-recovery.</li>
 *   <li>{@code POST /admin/auth/recovery/complete}     — public, consume token.</li>
 *   <li>{@code POST /admin/users/{id}/recovery/initiate} — SUPER_ADMIN-only,
 *       admin-initiated recovery for a target user.</li>
 * </ul>
 *
 * <p>The two public endpoints are wired in SecurityConfig as
 * {@code permitAll()} but rate-limited at the gateway / KAYA layer.
 */
@RestController
public class AdminAccountRecoveryController {

    private final AccountRecoveryService recoveryService;
    private final OtpService otpService;
    private final AdminAuthHelper auth;

    public AdminAccountRecoveryController(AccountRecoveryService recoveryService,
                                          OtpService otpService,
                                          AdminAuthHelper auth) {
        this.recoveryService = recoveryService;
        this.otpService = otpService;
        this.auth = auth;
    }

    // ── Public — self-initiated ─────────────────────────────────────────────

    @PostMapping("/admin/auth/recovery/initiate")
    public ResponseEntity<Map<String, Object>> initiateSelf(
            @org.springframework.web.bind.annotation.RequestBody InitiateSelfRequest req,
            jakarta.servlet.http.HttpServletRequest http) {
        String ip = clientIp(http);
        String ua = http.getHeader("User-Agent");
        var resp = recoveryService.initiateSelfRecovery(req.email, ip, ua);
        Map<String, Object> body = new LinkedHashMap<>(resp.toPublicMap());
        // Plain token never returned over the public surface — only emailed.
        body.remove("token");
        return ResponseEntity.ok(body);
    }

    /**
     * Phase 4.b.4 — magic-link verify entrypoint mirroring the onboarding
     * pattern. Returns the 8-digit OTP shown on the same browser tab.
     */
    @PostMapping("/admin/auth/recovery/verify-link")
    public ResponseEntity<Map<String, Object>> verifyRecoveryLink(
            @org.springframework.web.bind.annotation.RequestBody VerifyLinkRequest req,
            jakarta.servlet.http.HttpServletRequest http) {
        if (req.token == null || req.token.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "token required");
        }
        try {
            String ua = http.getHeader("User-Agent");
            var sess = recoveryService.verifyRecoveryMagicLink(req.token, clientIp(http), ua);
            Map<String, Object> body = new LinkedHashMap<>();
            body.put("sessionId", sess.sessionId);
            body.put("otpDisplay", sess.otpDisplay);
            body.put("expiresAt", sess.expiresAt.toString());
            body.put("requestId", sess.requestId);
            return ResponseEntity.ok(body);
        } catch (IllegalArgumentException e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, e.getMessage());
        } catch (IllegalStateException e) {
            throw new ResponseStatusException(HttpStatus.GONE, e.getMessage());
        }
    }

    @PostMapping("/admin/auth/recovery/verify-otp")
    public ResponseEntity<Map<String, Object>> verifyRecoveryOtp(
            @org.springframework.web.bind.annotation.RequestBody VerifyRecoveryOtpRequest req) {
        try {
            var out = recoveryService.completeRecoveryWithSession(
                    req.sessionId, req.otpEntry, req.kratosFlowId);
            Map<String, Object> body = new LinkedHashMap<>();
            body.put("userId", out.userId);
            body.put("aal", out.aal);
            body.put("mustReenrollMfa", out.mustReenrollMfa);
            body.put("requestId", out.requestId);
            if (req.kratosFlowId != null) body.put("kratosFlowId", req.kratosFlowId);
            return ResponseEntity.ok(body);
        } catch (IllegalArgumentException e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, e.getMessage());
        } catch (IllegalStateException e) {
            throw new ResponseStatusException(HttpStatus.GONE, e.getMessage());
        }
    }

    private static String clientIp(jakarta.servlet.http.HttpServletRequest http) {
        String fwd = http.getHeader("X-Forwarded-For");
        if (fwd != null && !fwd.isBlank()) {
            int comma = fwd.indexOf(',');
            return (comma > 0 ? fwd.substring(0, comma) : fwd).trim();
        }
        return http.getRemoteAddr();
    }

    @PostMapping("/admin/auth/recovery/complete")
    public ResponseEntity<Map<String, Object>> complete(
            @org.springframework.web.bind.annotation.RequestBody CompleteRequest req) {
        try {
            var out = recoveryService.completeRecovery(req.tokenOrCode);
            Map<String, Object> body = new LinkedHashMap<>();
            body.put("userId", out.userId);
            body.put("aal", out.aal);
            body.put("mustReenrollMfa", out.mustReenrollMfa);
            body.put("requestId", out.requestId);
            if (req.kratosFlowId != null) body.put("kratosFlowId", req.kratosFlowId);
            return ResponseEntity.ok(body);
        } catch (IllegalArgumentException e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, e.getMessage());
        } catch (IllegalStateException e) {
            throw new ResponseStatusException(HttpStatus.GONE, e.getMessage());
        }
    }

    // ── Authenticated — admin-initiated for a target user ───────────────────

    @PostMapping("/admin/users/{userId}/recovery/initiate")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    @RequiresStepUp(maxAgeSeconds = 300)
    public ResponseEntity<Map<String, Object>> initiateAdmin(
            @PathVariable UUID userId,
            @org.springframework.web.bind.annotation.RequestBody InitiateAdminRequest req,
            @RequestHeader(value = "Idempotency-Key", required = false) String idempotencyKey) {
        if (!auth.acquireIdempotency(idempotencyKey)) {
            return ResponseEntity.status(409).body(Map.of("error", "duplicate idempotency-key"));
        }
        if (req.motif == null || req.motif.length() < 50) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST,
                    "motif must be at least 50 characters");
        }
        UUID initiatorId = auth.currentUserId().orElseThrow(() ->
                new ResponseStatusException(HttpStatus.UNAUTHORIZED, "JWT principal missing"));

        // Confirm initiator identity via OTP proof (delta §5.B step 2).
        if (req.otpProof == null || req.otpProof.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "otpProof required");
        }
        boolean otpOk = otpService.getUserIdForOtp(req.otpProof)
                .map(uid -> uid.equals(initiatorId))
                .orElse(false);
        if (!otpOk) {
            throw new ResponseStatusException(HttpStatus.FORBIDDEN, "otpProof invalid");
        }

        var resp = recoveryService.initiateAdminRecovery(userId, initiatorId, req.motif);
        return ResponseEntity.ok(resp.toPublicMap());
    }

    // ── Request DTOs ───────────────────────────────────────────────────────

    public static class InitiateSelfRequest {
        @NotBlank public String email;
    }

    public static class CompleteRequest {
        @NotBlank public String tokenOrCode;
        public String kratosFlowId;
    }

    public static class InitiateAdminRequest {
        @NotBlank public String motif;
        @NotBlank public String otpProof;
    }

    public static class VerifyLinkRequest {
        @NotBlank public String token;
    }

    public static class VerifyRecoveryOtpRequest {
        @NotBlank public String sessionId;
        @NotBlank public String otpEntry;
        public String kratosFlowId;
    }
}
