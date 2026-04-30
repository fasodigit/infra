// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.BruteForceService;
import bf.gov.faso.auth.service.admin.AdminAuditService;
import bf.gov.faso.auth.service.admin.AdminMfaEnrollmentService;
import bf.gov.faso.auth.service.admin.SuperAdminProtectionService;
import org.springframework.dao.DataIntegrityViolationException;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * User CRUD endpoints. Iter 1 wires only the canonical actions used by the
 * admin UI (list, get, suspend, reactivate, mfa/reset, delete). The /invite
 * endpoint delegates to KratosService in iteration 2 (TODO).
 *
 * <p>Delta amendment 2026-04-30: every destructive operation goes through
 * {@link SuperAdminProtectionService} which enforces the "indeletable /
 * insuspendable / last-SA-protected" invariants.
 */
@RestController
@RequestMapping("/admin/users")
public class AdminUserController {

    private final UserRepository userRepository;
    private final BruteForceService bruteForceService;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;
    private final AdminMfaEnrollmentService mfaEnrollment;
    private final SuperAdminProtectionService superAdminProtection;
    private final AdminAuthHelper auth;

    public AdminUserController(UserRepository userRepository,
                               BruteForceService bruteForceService,
                               AdminAuditService auditService,
                               AdminEventProducer eventProducer,
                               AdminMfaEnrollmentService mfaEnrollment,
                               SuperAdminProtectionService superAdminProtection,
                               AdminAuthHelper auth) {
        this.userRepository = userRepository;
        this.bruteForceService = bruteForceService;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
        this.mfaEnrollment = mfaEnrollment;
        this.superAdminProtection = superAdminProtection;
        this.auth = auth;
    }

    @GetMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<List<User>> list() {
        return ResponseEntity.ok(userRepository.findAll());
    }

    @GetMapping("/{userId}")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<User> get(@PathVariable UUID userId) {
        return userRepository.findById(userId)
                .map(ResponseEntity::ok)
                .orElse(ResponseEntity.notFound().build());
    }

    @PostMapping("/{userId}/suspend")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<Map<String, Object>> suspend(
            @PathVariable UUID userId,
            @org.springframework.web.bind.annotation.RequestBody(required = false) SuspendRequest req) {
        UUID actor = auth.currentUserId().orElse(null);
        // Delta 2026-04-30: refuse to suspend a SUPER_ADMIN.
        superAdminProtection.assertNotSuperAdmin(userId, "suspend", actor);
        User u = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("user not found"));
        u.setSuspended(true);
        try {
            userRepository.save(u);
        } catch (DataIntegrityViolationException ex) {
            // The DB trigger is the backstop; if we reach here we missed an
            // edge case in the service guard. Log + audit + 403.
            auditService.log(AuditAction.SUPER_ADMIN_PROTECTION_TRIGGERED.key(),
                    actor, "user:" + userId, null,
                    Map.of("operation", "suspend", "trigger", "db"), null);
            throw new ResponseStatusException(HttpStatus.FORBIDDEN,
                    "SUPER_ADMIN_PROTECTION (db trigger): " + ex.getMostSpecificCause().getMessage());
        }
        String reason = req == null ? null : req.reason;
        auditService.log("user.suspended", actor, "user:" + userId, null,
                Map.of("reason", reason == null ? "" : reason), null);
        eventProducer.publishUserSuspended(userId, actor, reason);
        return ResponseEntity.ok(Map.of("suspended", true));
    }

    @DeleteMapping("/{userId}/suspend")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<Map<String, Object>> reactivate(@PathVariable UUID userId) {
        bruteForceService.unlockAccount(userId);
        UUID actor = auth.currentUserId().orElse(null);
        auditService.log("user.reactivated", actor, "user:" + userId, null, null, null);
        eventProducer.publishUserReactivated(userId, actor);
        return ResponseEntity.ok(Map.of("reactivated", true));
    }

    @DeleteMapping("/{userId}")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public ResponseEntity<Map<String, Object>> delete(@PathVariable UUID userId) {
        UUID actor = auth.currentUserId().orElse(null);
        // Delta 2026-04-30: refuse to delete a SUPER_ADMIN (DB trigger backstop).
        superAdminProtection.assertNotSuperAdmin(userId, "delete", actor);
        try {
            userRepository.deleteById(userId);
        } catch (DataIntegrityViolationException ex) {
            auditService.log(AuditAction.SUPER_ADMIN_PROTECTION_TRIGGERED.key(),
                    actor, "user:" + userId, null,
                    Map.of("operation", "delete", "trigger", "db"), null);
            throw new ResponseStatusException(HttpStatus.FORBIDDEN,
                    "SUPER_ADMIN_PROTECTION (db trigger): " + ex.getMostSpecificCause().getMessage());
        }
        auditService.log("user.deleted", actor, "user:" + userId, null, null, null);
        return ResponseEntity.ok(Map.of("deleted", true));
    }

    @PostMapping("/{userId}/mfa/reset")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<Map<String, Object>> resetMfa(@PathVariable UUID userId) {
        UUID actor = auth.currentUserId().orElse(null);
        // TODO Phase 4.b iter 2 — orchestrate full MFA reset (delete TOTP, revoke
        // passkeys, invalidate recovery codes) within one transaction.
        var status = mfaEnrollment.recomputeStatus(userId);
        auditService.log("user.mfa.reset", actor, "user:" + userId, null, null, null);
        return ResponseEntity.ok(Map.of("status", status));
    }

    public static class SuspendRequest {
        public String reason;
    }
}
