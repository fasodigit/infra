// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.BreakGlassService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/break-glass")
public class AdminBreakGlassController {

    private final BreakGlassService breakGlassService;
    private final AdminAuthHelper auth;

    public AdminBreakGlassController(BreakGlassService breakGlassService, AdminAuthHelper auth) {
        this.breakGlassService = breakGlassService;
        this.auth = auth;
    }

    @PostMapping("/activate")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    @RequiresStepUp(maxAgeSeconds = 300)
    public ResponseEntity<Map<String, Object>> activate(
            @org.springframework.web.bind.annotation.RequestBody ActivateRequest req,
            @RequestHeader(value = "Idempotency-Key", required = false) String idempotencyKey) {
        if (!auth.acquireIdempotency(idempotencyKey)) {
            return ResponseEntity.status(409).body(Map.of("error", "duplicate idempotency-key"));
        }
        UUID userId = auth.currentUserId().orElseThrow();
        String token = breakGlassService.activate(userId, req.capability,
                req.justification, req.otpId, req.otpCode);
        return ResponseEntity.accepted().body(Map.of("token", token));
    }

    @GetMapping("/status")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Object> status() {
        UUID userId = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(breakGlassService.status(userId));
    }

    @PostMapping("/revoke")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public ResponseEntity<Map<String, Object>> revoke(
            @org.springframework.web.bind.annotation.RequestBody RevokeRequest req) {
        UUID actor = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(Map.of("revoked",
                breakGlassService.revokeManual(req.userId, actor)));
    }

    public static class ActivateRequest {
        @NotBlank public String capability;
        @NotBlank public String justification;
        @NotBlank public String otpId;
        @NotBlank public String otpCode;
    }

    public static class RevokeRequest {
        public UUID userId;
    }
}
