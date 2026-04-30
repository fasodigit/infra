// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.admin.WebAuthnService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/users/{userId}/passkeys")
public class AdminPasskeyController {

    private final WebAuthnService webAuthnService;
    private final UserRepository userRepository;
    private final AdminAuthHelper auth;

    public AdminPasskeyController(WebAuthnService webAuthnService,
                                  UserRepository userRepository,
                                  AdminAuthHelper auth) {
        this.webAuthnService = webAuthnService;
        this.userRepository = userRepository;
        this.auth = auth;
    }

    @PostMapping("/enroll/begin")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<String> registerBegin(@PathVariable UUID userId) {
        var u = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("user not found"));
        return ResponseEntity.ok(webAuthnService.registerBegin(userId,
                u.getEmail(), u.getFirstName() + " " + u.getLastName()));
    }

    @PostMapping("/enroll/finish")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> registerFinish(
            @PathVariable UUID userId,
            @org.springframework.web.bind.annotation.RequestBody String responseJson) {
        boolean ok = webAuthnService.registerFinish(userId, responseJson);
        return ResponseEntity.ok(Map.of("registered", ok));
    }

    @PostMapping("/authenticate/begin")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<String> authBegin(@PathVariable UUID userId) {
        return ResponseEntity.ok(webAuthnService.authenticateBegin(userId));
    }

    @PostMapping("/authenticate/finish")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> authFinish(
            @PathVariable UUID userId,
            @org.springframework.web.bind.annotation.RequestBody String assertionJson) {
        boolean ok = webAuthnService.authenticateFinish(userId, assertionJson);
        return ResponseEntity.ok(Map.of("authenticated", ok));
    }

    @DeleteMapping("/{credentialId}")
    @PreAuthorize("hasRole('SUPER_ADMIN') or hasRole('ADMIN')")
    public ResponseEntity<Map<String, Object>> revoke(
            @PathVariable UUID userId,
            @PathVariable String credentialId) {
        UUID actor = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(Map.of("revoked",
                webAuthnService.revoke(userId, credentialId, actor)));
    }

    public static class RenameRequest {
        @NotBlank public String name;
    }
}
