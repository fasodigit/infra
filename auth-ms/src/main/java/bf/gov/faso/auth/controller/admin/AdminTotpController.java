// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.admin.TotpService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/users/{userId}/totp")
public class AdminTotpController {

    private final TotpService totpService;
    private final UserRepository userRepository;
    private final AdminAuthHelper auth;

    public AdminTotpController(TotpService totpService,
                               UserRepository userRepository,
                               AdminAuthHelper auth) {
        this.totpService = totpService;
        this.userRepository = userRepository;
        this.auth = auth;
    }

    @PostMapping("/enroll/begin")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, String>> enrollBegin(@PathVariable UUID userId) {
        String email = userRepository.findById(userId)
                .map(u -> u.getEmail())
                .orElseThrow(() -> new IllegalArgumentException("user not found"));
        return ResponseEntity.ok(totpService.enrollBegin(userId, email));
    }

    @PostMapping("/enroll/finish")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> enrollFinish(
            @PathVariable UUID userId,
            @org.springframework.web.bind.annotation.RequestBody EnrollFinishRequest req) {
        boolean ok = totpService.enrollFinish(userId, req.secret, req.code);
        return ResponseEntity.ok(Map.of("enrolled", ok));
    }

    @DeleteMapping
    @PreAuthorize("hasRole('SUPER_ADMIN') or hasRole('ADMIN')")
    public ResponseEntity<Map<String, Object>> disable(@PathVariable UUID userId) {
        UUID actor = auth.currentUserId().orElseThrow();
        boolean ok = totpService.disable(userId, actor);
        return ResponseEntity.ok(Map.of("disabled", ok));
    }

    public static class EnrollFinishRequest {
        @NotBlank public String secret;
        @NotBlank public String code;
    }
}
