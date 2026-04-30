// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.RecoveryCodeService;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/recovery-codes")
public class AdminRecoveryCodeController {

    private final RecoveryCodeService recoveryCodeService;
    private final AdminAuthHelper auth;

    public AdminRecoveryCodeController(RecoveryCodeService recoveryCodeService,
                                       AdminAuthHelper auth) {
        this.recoveryCodeService = recoveryCodeService;
        this.auth = auth;
    }

    @PostMapping("/generate")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> generate(
            @org.springframework.web.bind.annotation.RequestBody GenerateRequest req,
            @RequestHeader(value = "Idempotency-Key", required = false) String idempotencyKey) {
        if (!auth.acquireIdempotency(idempotencyKey)) {
            return ResponseEntity.status(409).body(Map.of("error", "duplicate idempotency-key"));
        }
        UUID target = req.userId != null ? req.userId : auth.currentUserId().orElseThrow();
        List<String> codes = recoveryCodeService.generate(target, req.motif);
        return ResponseEntity.ok(Map.of("codes", codes));
    }

    @PostMapping("/use")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> use(
            @org.springframework.web.bind.annotation.RequestBody UseRequest req) {
        UUID target = req.userId != null ? req.userId : auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(Map.of("consumed",
                recoveryCodeService.use(target, req.code)));
    }

    public static class GenerateRequest {
        public UUID userId;
        @NotBlank public String motif;
    }

    public static class UseRequest {
        public UUID userId;
        @NotNull public String code;
    }
}
