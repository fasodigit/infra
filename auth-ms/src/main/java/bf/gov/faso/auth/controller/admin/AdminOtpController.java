// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.OtpService;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/otp")
public class AdminOtpController {

    private final OtpService otpService;
    private final AdminAuthHelper auth;

    public AdminOtpController(OtpService otpService, AdminAuthHelper auth) {
        this.otpService = otpService;
        this.auth = auth;
    }

    @PostMapping("/issue")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> issue(
            @org.springframework.web.bind.annotation.RequestBody IssueRequest req,
            @RequestHeader(value = "Idempotency-Key", required = false) String idempotencyKey) {
        if (!auth.acquireIdempotency(idempotencyKey)) {
            return ResponseEntity.status(409).body(Map.of("error", "duplicate idempotency-key"));
        }
        UUID target = req.userId != null ? req.userId : auth.currentUserId().orElseThrow();
        String otpId = otpService.issue(target, req.method);
        return ResponseEntity.accepted().body(Map.of("otpId", otpId));
    }

    @PostMapping("/verify")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> verify(
            @org.springframework.web.bind.annotation.RequestBody VerifyRequest req) {
        boolean ok = otpService.verify(req.otpId, req.code);
        return ResponseEntity.ok(Map.of("verified", ok));
    }

    public static class IssueRequest {
        public UUID userId;
        @NotBlank public String method = "EMAIL";
    }

    public static class VerifyRequest {
        @NotBlank public String otpId;
        @NotNull  public String code;
    }
}
