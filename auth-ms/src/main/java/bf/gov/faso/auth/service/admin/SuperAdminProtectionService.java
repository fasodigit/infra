// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.web.server.ResponseStatusException;

import java.util.Map;
import java.util.UUID;

/**
 * Service-level guard for the SUPER_ADMIN protection invariants
 * (delta amendment 2026-04-30 §2). The DB trigger
 * {@code prevent_super_admin_delete} (V11) is the defense-in-depth backstop;
 * this service short-circuits the request earlier with a clean 403/409 +
 * {@code SUPER_ADMIN_PROTECTION_TRIGGERED} audit entry.
 */
@Service
public class SuperAdminProtectionService {

    private static final Logger log = LoggerFactory.getLogger(SuperAdminProtectionService.class);
    private static final String SUPER_ADMIN_ROLE = "SUPER_ADMIN";

    private final UserRepository userRepo;
    private final AdminAuditService auditService;

    public SuperAdminProtectionService(UserRepository userRepo,
                                       AdminAuditService auditService) {
        this.userRepo = userRepo;
        this.auditService = auditService;
    }

    /**
     * @throws ResponseStatusException 403 if {@code targetUserId} carries the
     *         {@code SUPER_ADMIN} role.
     */
    public void assertNotSuperAdmin(UUID targetUserId, String operation, UUID actorId) {
        if (targetUserId == null) return;
        User target = userRepo.findById(targetUserId).orElse(null);
        if (target == null) return;
        boolean isSuper = target.getRoles().stream()
                .anyMatch(r -> SUPER_ADMIN_ROLE.equals(r.getName()));
        if (!isSuper) return;

        auditService.log(AuditAction.SUPER_ADMIN_PROTECTION_TRIGGERED.key(),
                actorId, "user:" + targetUserId, null,
                Map.of("operation", operation == null ? "" : operation,
                        "reason", "target_is_super_admin"), null);
        log.warn("SUPER_ADMIN_PROTECTION blocked op={} target={} actor={}",
                operation, targetUserId, actorId);
        throw new ResponseStatusException(HttpStatus.FORBIDDEN,
                "SUPER_ADMIN_PROTECTION: cannot " + operation + " a SUPER_ADMIN account");
    }

    /**
     * @throws ResponseStatusException 409 if removing {@code targetUserId}
     *         would leave the platform with zero active SUPER_ADMINs.
     */
    public void assertNotLastSuperAdmin(UUID targetUserId, UUID actorId) {
        long activeSupers = userRepo.findByRoleName(SUPER_ADMIN_ROLE).stream()
                .filter(u -> !u.isSuspended())
                .count();
        if (activeSupers > 1) return;

        // 0 or 1 active SA — if target is the (or only) one, refuse.
        User target = userRepo.findById(targetUserId).orElse(null);
        boolean targetIsSuper = target != null && target.getRoles().stream()
                .anyMatch(r -> SUPER_ADMIN_ROLE.equals(r.getName()));
        if (!targetIsSuper) return;

        auditService.log(AuditAction.SUPER_ADMIN_PROTECTION_TRIGGERED.key(),
                actorId, "user:" + targetUserId, null,
                Map.of("operation", "demote_last_super_admin",
                        "activeSuperCount", activeSupers), null);
        log.warn("LAST_SUPER_ADMIN_PROTECTION blocked target={} (active SA count={})",
                targetUserId, activeSupers);
        throw new ResponseStatusException(HttpStatus.CONFLICT,
                "LAST_SUPER_ADMIN_PROTECTION: cannot remove the last active SUPER_ADMIN");
    }

    public boolean isSuperAdmin(UUID userId) {
        if (userId == null) return false;
        return userRepo.findById(userId).map(u -> u.getRoles().stream()
                .anyMatch(r -> SUPER_ADMIN_ROLE.equals(r.getName())))
                .orElse(false);
    }
}
