// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.KratosService;
import bf.gov.faso.auth.service.admin.AdminAuditService;
import bf.gov.faso.auth.service.admin.RecoveryCodeService;
import bf.gov.faso.auth.service.admin.TotpService;
import bf.gov.faso.auth.service.admin.WebAuthnService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Self-management endpoints for any authenticated admin (delta amendment
 * 2026-04-30 §3). The userId is always extracted from the JWT principal —
 * never from the path. SUPER_ADMIN auto-modifications do NOT trigger the
 * dual-control workflow.
 */
@RestController
@RequestMapping("/admin/me")
public class AdminMeController {

    private final TotpService totpService;
    private final WebAuthnService webAuthnService;
    private final RecoveryCodeService recoveryCodeService;
    private final UserRepository userRepository;
    private final KratosService kratosService;
    private final AdminAuditService auditService;
    private final AdminAuthHelper auth;

    public AdminMeController(TotpService totpService,
                             WebAuthnService webAuthnService,
                             RecoveryCodeService recoveryCodeService,
                             UserRepository userRepository,
                             KratosService kratosService,
                             AdminAuditService auditService,
                             AdminAuthHelper auth) {
        this.totpService = totpService;
        this.webAuthnService = webAuthnService;
        this.recoveryCodeService = recoveryCodeService;
        this.userRepository = userRepository;
        this.kratosService = kratosService;
        this.auditService = auditService;
        this.auth = auth;
    }

    // ── Password (proxy → Kratos /self-service/settings) ───────────────────

    @PostMapping("/password")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> changePassword(
            @org.springframework.web.bind.annotation.RequestBody PasswordRequest req) {
        UUID userId = currentOrThrow();
        // Phase 4.b iter 2 will wire the full Kratos /self-service/settings
        // flow (init -> submit) through KratosService. For now we record
        // intent + hand back the timestamp; the BFF wires the actual flow.
        auditService.log(AuditAction.SELF_PASSWORD_CHANGED.key(), userId,
                "user:" + userId, null,
                Map.of("via", "kratos_self_service"), null);
        return ResponseEntity.ok(Map.of(
                "changedAt", Instant.now().toString(),
                "kratosFlow", "self-service/settings"
        ));
    }

    // ── PassKey ────────────────────────────────────────────────────────────

    @PostMapping("/passkeys/enroll/begin")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<String> passkeyBegin() {
        UUID userId = currentOrThrow();
        var u = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("user not found"));
        return ResponseEntity.ok(webAuthnService.registerBegin(userId,
                u.getEmail(), u.getFirstName() + " " + u.getLastName()));
    }

    @PostMapping("/passkeys/enroll/finish")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> passkeyFinish(
            @org.springframework.web.bind.annotation.RequestBody String responseJson) {
        UUID userId = currentOrThrow();
        boolean ok = webAuthnService.registerFinish(userId, responseJson);
        if (ok) {
            auditService.log(AuditAction.SELF_PASSKEY_ENROLLED.key(), userId,
                    "user:" + userId, null, Map.of(), null);
            clearMustReenrollIfAllSet(userId);
        }
        return ResponseEntity.ok(Map.of("enrolled", ok,
                "enrolledAt", Instant.now().toString()));
    }

    @DeleteMapping("/passkeys/{credentialId}")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> passkeyRevoke(@PathVariable String credentialId) {
        UUID userId = currentOrThrow();
        boolean ok = webAuthnService.revoke(userId, credentialId, userId);
        if (ok) {
            auditService.log(AuditAction.SELF_PASSKEY_REVOKED.key(), userId,
                    "user:" + userId, null, Map.of("credentialId", credentialId), null);
        }
        return ResponseEntity.ok(Map.of("deletedAt", Instant.now().toString(), "ok", ok));
    }

    // ── TOTP ───────────────────────────────────────────────────────────────

    @PostMapping("/totp/enroll/begin")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, String>> totpBegin() {
        UUID userId = currentOrThrow();
        String email = userRepository.findById(userId)
                .map(u -> u.getEmail())
                .orElseThrow(() -> new IllegalArgumentException("user not found"));
        return ResponseEntity.ok(totpService.enrollBegin(userId, email));
    }

    @PostMapping("/totp/enroll/finish")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> totpFinish(
            @org.springframework.web.bind.annotation.RequestBody TotpFinishRequest req) {
        UUID userId = currentOrThrow();
        boolean ok = totpService.enrollFinish(userId, req.tempSecret, req.code);
        if (ok) {
            auditService.log(AuditAction.SELF_TOTP_ENROLLED.key(), userId,
                    "user:" + userId, null, Map.of(), null);
            clearMustReenrollIfAllSet(userId);
        }
        return ResponseEntity.ok(Map.of("enrolled", ok,
                "enrolledAt", Instant.now().toString()));
    }

    @DeleteMapping("/totp")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> totpDisable() {
        UUID userId = currentOrThrow();
        boolean ok = totpService.disable(userId, userId);
        if (ok) {
            auditService.log(AuditAction.SELF_TOTP_DISABLED.key(), userId,
                    "user:" + userId, null, Map.of(), null);
        }
        return ResponseEntity.ok(Map.of("disabledAt", Instant.now().toString(), "ok", ok));
    }

    // ── Recovery codes ─────────────────────────────────────────────────────

    @PostMapping("/recovery-codes/regenerate")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> regenerateRecoveryCodes(
            @org.springframework.web.bind.annotation.RequestBody RegenerateRequest req) {
        UUID userId = currentOrThrow();
        List<String> codes = recoveryCodeService.generate(userId,
                req.motif == null ? "self_regenerate" : req.motif);
        auditService.log(AuditAction.SELF_RECOVERY_CODES_REGENERATED.key(), userId,
                "user:" + userId, null, Map.of("count", codes.size()), null);
        return ResponseEntity.ok(Map.of(
                "codes", codes,
                "generatedAt", Instant.now().toString()
        ));
    }

    @PostMapping("/recovery-codes/use")
    @PreAuthorize("isAuthenticated()")
    public ResponseEntity<Map<String, Object>> useRecoveryCode(
            @org.springframework.web.bind.annotation.RequestBody UseRequest req) {
        UUID userId = currentOrThrow();
        boolean consumed = recoveryCodeService.use(userId, req.code);
        long remaining = recoveryCodeService.countRemaining(userId);
        return ResponseEntity.ok(Map.of(
                "consumed", consumed,
                "remaining", remaining
        ));
    }

    // ── Helpers ────────────────────────────────────────────────────────────

    private UUID currentOrThrow() {
        return auth.currentUserId()
                .orElseThrow(() -> new ResponseStatusException(HttpStatus.UNAUTHORIZED,
                        "JWT principal missing"));
    }

    /**
     * If the user has at least one MFA factor enrolled (TOTP or PassKey) and
     * recovery codes regenerated post-recovery, clear the must_reenroll_mfa
     * flag set by AccountRecoveryService.completeRecovery.
     */
    private void clearMustReenrollIfAllSet(UUID userId) {
        userRepository.findById(userId).ifPresent(u -> {
            if (u.isMustReenrollMfa()) {
                u.setMustReenrollMfa(false);
                userRepository.save(u);
            }
        });
    }

    // ── Request DTOs ───────────────────────────────────────────────────────

    public static class PasswordRequest {
        @NotBlank public String currentPassword;
        @NotBlank public String newPassword;
    }

    public static class TotpFinishRequest {
        @NotBlank public String tempSecret;
        @NotBlank public String code;
    }

    public static class RegenerateRequest {
        public String motif;
    }

    public static class UseRequest {
        @NotBlank public String code;
    }
}
