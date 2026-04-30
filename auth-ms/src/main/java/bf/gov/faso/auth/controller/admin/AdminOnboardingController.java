// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.AdminOnboardingService;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.server.ResponseStatusException;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Public (no-auth) magic-link onboarding endpoints (Phase 4.b.4 §2).
 *
 * <ul>
 *   <li>{@code POST /admin/auth/onboard/begin}        — invoked by the BFF
 *       proxy when SUPER-ADMIN clicks "Inviter".</li>
 *   <li>{@code POST /admin/auth/onboard/verify-link}  — first hit from the
 *       invitee's browser ; returns the 8-digit OTP shown on screen.</li>
 *   <li>{@code POST /admin/auth/onboard/verify-otp}   — confirms the OTP and
 *       returns the Kratos settings-flow descriptor for forced MFA enrol.</li>
 * </ul>
 *
 * <p>All endpoints are wired in {@code SecurityConfig} as {@code permitAll()}
 * — they are rate-limited at the BFF and at ARMAGEDDON.
 */
@RestController
public class AdminOnboardingController {

    private final AdminOnboardingService service;

    public AdminOnboardingController(AdminOnboardingService service) {
        this.service = service;
    }

    @PostMapping("/admin/auth/onboard/begin")
    public ResponseEntity<Map<String, Object>> begin(@RequestBody BeginRequest req,
                                                     HttpServletRequest http) {
        if (req.invitationId == null) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "invitationId required");
        }
        if (req.email == null || req.email.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "email required");
        }
        String ip = clientIp(http);
        var inv = service.initiateOnboarding(req.invitationId, req.email, req.role,
                req.inviterName, req.inviterId, ip, req.lang);
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("invitationId", inv.invitationId.toString());
        body.put("expiresAt", inv.expiresAt.toString());
        // The plain magic link is only returned to the trusted SUPER-ADMIN
        // caller (the BFF) so it can be displayed in the invitation modal as
        // a fallback. notifier-ms also receives it via the topic.
        body.put("magicLink", inv.magicLink);
        return ResponseEntity.ok(body);
    }

    @PostMapping("/admin/auth/onboard/verify-link")
    public ResponseEntity<Map<String, Object>> verifyLink(@RequestBody VerifyLinkRequest req,
                                                          HttpServletRequest http) {
        if (req.token == null || req.token.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "token required");
        }
        try {
            String ua = http.getHeader("User-Agent");
            var sess = service.verifyMagicLink(req.token, clientIp(http), ua);
            Map<String, Object> body = new LinkedHashMap<>();
            body.put("sessionId", sess.sessionId);
            body.put("otpDisplay", sess.otpDisplay);
            body.put("expiresAt", sess.expiresAt.toString());
            body.put("email", sess.email);
            return ResponseEntity.ok(body);
        } catch (IllegalArgumentException e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, e.getMessage());
        } catch (IllegalStateException e) {
            throw new ResponseStatusException(HttpStatus.GONE, e.getMessage());
        }
    }

    @PostMapping("/admin/auth/onboard/verify-otp")
    public ResponseEntity<Map<String, Object>> verifyOtp(@RequestBody VerifyOtpRequest req,
                                                         HttpServletRequest http) {
        if (req.sessionId == null || req.otpEntry == null) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "sessionId and otpEntry required");
        }
        try {
            var out = service.verifyOnboardingOtp(req.sessionId, req.otpEntry, clientIp(http));
            Map<String, Object> body = new LinkedHashMap<>();
            body.put("kratosSettingsFlowId", out.kratosSettingsFlowId);
            body.put("redirectPath", out.redirectPath);
            body.put("email", out.email);
            body.put("invitationId", out.invitationId);
            body.put("mustEnrollPasskey", out.mustEnrollPasskey);
            body.put("mustEnrollTotp", out.mustEnrollTotp);
            body.put("mustGenerateRecoveryCodes", out.mustGenerateRecoveryCodes);
            return ResponseEntity.ok(body);
        } catch (IllegalArgumentException e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, e.getMessage());
        } catch (IllegalStateException e) {
            throw new ResponseStatusException(HttpStatus.GONE, e.getMessage());
        }
    }

    private static String clientIp(HttpServletRequest http) {
        String fwd = http.getHeader("X-Forwarded-For");
        if (fwd != null && !fwd.isBlank()) {
            int comma = fwd.indexOf(',');
            return (comma > 0 ? fwd.substring(0, comma) : fwd).trim();
        }
        return http.getRemoteAddr();
    }

    // ── Request DTOs ───────────────────────────────────────────────────────

    public static class BeginRequest {
        @NotNull public UUID invitationId;
        @NotBlank public String email;
        public String role;
        public String inviterName;
        public UUID inviterId;
        public String lang;
    }

    public static class VerifyLinkRequest {
        @NotBlank public String token;
    }

    public static class VerifyOtpRequest {
        @NotBlank public String sessionId;
        @NotBlank public String otpEntry;
    }
}
