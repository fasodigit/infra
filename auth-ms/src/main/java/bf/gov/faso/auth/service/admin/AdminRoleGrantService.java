// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AdminRoleGrant;
import bf.gov.faso.auth.model.Role;
import bf.gov.faso.auth.repository.AdminRoleGrantRepository;
import bf.gov.faso.auth.repository.RoleRepository;
import bf.gov.faso.auth.service.PermissionGrantService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;

/**
 * Dual-control workflow on top of {@link PermissionGrantService}.
 * <p>
 * Sequence: a SUPER-ADMIN calls {@link #requestGrant} (status PENDING) ->
 * a DIFFERENT SUPER-ADMIN approves via {@link #approveGrant} (status
 * APPROVED, role assigned, Keto sync, role.granted event published).
 */
@Service
public class AdminRoleGrantService {

    private static final Logger log = LoggerFactory.getLogger(AdminRoleGrantService.class);

    private final AdminRoleGrantRepository grantRepo;
    private final RoleRepository roleRepo;
    private final PermissionGrantService permissionGrantService;
    private final AdminEventProducer eventProducer;
    private final AdminAuditService auditService;
    private final AdminSettingsService settingsService;

    /** Optional — wired only when the delta amendment 2026-04-30 services are present. */
    @Autowired(required = false)
    private CapabilityService capabilityService;

    @Autowired(required = false)
    private SuperAdminProtectionService superAdminProtection;

    public AdminRoleGrantService(AdminRoleGrantRepository grantRepo,
                                 RoleRepository roleRepo,
                                 PermissionGrantService permissionGrantService,
                                 AdminEventProducer eventProducer,
                                 AdminAuditService auditService,
                                 AdminSettingsService settingsService) {
        this.grantRepo = grantRepo;
        this.roleRepo = roleRepo;
        this.permissionGrantService = permissionGrantService;
        this.eventProducer = eventProducer;
        this.auditService = auditService;
        this.settingsService = settingsService;
    }

    @Transactional
    public AdminRoleGrant requestGrant(UUID grantorId, UUID granteeId, UUID roleId,
                                       String justification) {
        return requestGrant(grantorId, granteeId, roleId, justification, null);
    }

    /**
     * Delta amendment 2026-04-30 — accept a fine-grained {@code capabilities}
     * set that will be persisted at approval time (see
     * {@link #approveGrant(UUID, UUID, java.util.Set)}). Capabilities are
     * stored on the grant via the audit details (the column was not added to
     * keep the existing schema stable; the actual grants live in
     * {@code account_capability_grants}).
     */
    @Transactional
    public AdminRoleGrant requestGrant(UUID grantorId, UUID granteeId, UUID roleId,
                                       String justification, Set<String> capabilities) {
        if (justification == null || justification.isBlank()) {
            throw new IllegalArgumentException("justification required");
        }
        Role role = roleRepo.findById(roleId)
                .orElseThrow(() -> new IllegalArgumentException("role not found: " + roleId));

        AdminRoleGrant g = new AdminRoleGrant();
        g.setGrantorId(grantorId);
        g.setGranteeId(granteeId);
        g.setRoleId(roleId);
        g.setJustification(justification);
        g.setStatus(AdminRoleGrant.Status.PENDING);
        int defaultDays = settingsService.getInt("grant.expiry_default_days", 90);
        g.setExpiresAt(Instant.now().plus(defaultDays, ChronoUnit.DAYS));
        g.setCreatedAt(Instant.now());
        AdminRoleGrant saved = grantRepo.save(g);

        auditService.log("grant.requested", grantorId, "grant:" + saved.getId(), null,
                Map.of("granteeId", granteeId.toString(), "roleId", roleId.toString(),
                        "roleName", role.getName(), "justification", justification,
                        "capabilities", capabilities == null ? List.of() : List.copyOf(capabilities)),
                null);
        log.info("Grant requested id={} grantor={} grantee={} role={} caps={}",
                saved.getId(), grantorId, granteeId, role.getName(),
                capabilities == null ? 0 : capabilities.size());
        return saved;
    }

    @Transactional
    public AdminRoleGrant approveGrant(UUID grantId, UUID approverId) {
        return approveGrant(grantId, approverId, null);
    }

    /**
     * Delta amendment 2026-04-30 — accept the capability set selected at
     * request time (or supplemented at approval). After role assignment,
     * persist the capabilities via {@link CapabilityService} and write Keto
     * tuples in the {@code Capability} namespace.
     */
    @Transactional
    public AdminRoleGrant approveGrant(UUID grantId, UUID approverId, Set<String> capabilities) {
        AdminRoleGrant g = grantRepo.findById(grantId)
                .orElseThrow(() -> new IllegalArgumentException("grant not found: " + grantId));

        if (g.getStatus() != AdminRoleGrant.Status.PENDING) {
            throw new IllegalStateException("grant is not PENDING (status=" + g.getStatus() + ")");
        }
        if (settingsService.getBool("grant.dual_control_required", true) &&
                approverId.equals(g.getGrantorId())) {
            throw new IllegalArgumentException("approver must differ from grantor (dual-control)");
        }

        g.setStatus(AdminRoleGrant.Status.APPROVED);
        g.setApproverId(approverId);
        g.setApprovedAt(Instant.now());
        grantRepo.save(g);

        // Effective role assignment + Keto sync (delegate to existing service).
        permissionGrantService.assignRole(g.getGranteeId(), g.getRoleId());
        Role role = roleRepo.findById(g.getRoleId()).orElse(null);
        String roleName = role == null ? "<unknown>" : role.getName();

        // Delta 2026-04-30: persist fine-grained capabilities (best-effort —
        // optional service so legacy grants without caps still work).
        if (capabilityService != null && capabilities != null && !capabilities.isEmpty()) {
            CapabilityService.AdminLevel forRole = mapRoleNameToLevel(roleName);
            capabilityService.grantCapabilities(g.getGranteeId(), capabilities,
                    forRole, approverId, "grant:" + g.getId());
        }

        eventProducer.publishRoleGranted(g.getGranteeId(), roleName, approverId);
        auditService.log("grant.approved", approverId, "grant:" + g.getId(), null,
                Map.of("granteeId", g.getGranteeId().toString(),
                        "roleId", g.getRoleId().toString(),
                        "roleName", roleName,
                        "capabilities", capabilities == null ? List.of() : List.copyOf(capabilities)),
                null);
        log.info("Grant approved id={} approver={} role={} caps={}",
                g.getId(), approverId, roleName,
                capabilities == null ? 0 : capabilities.size());
        return g;
    }

    /**
     * Delta amendment 2026-04-30 — revoke an effective role from a user
     * with last-SA protection. Capabilities tied to the role context are NOT
     * automatically revoked here; callers must invoke
     * {@link CapabilityService#revokeCapabilities} explicitly when needed.
     */
    @Transactional
    public void revokeRole(UUID granteeId, UUID roleId, UUID actorId, String motif) {
        Role role = roleRepo.findById(roleId)
                .orElseThrow(() -> new IllegalArgumentException("role not found: " + roleId));
        if ("SUPER_ADMIN".equals(role.getName()) && superAdminProtection != null) {
            superAdminProtection.assertNotLastSuperAdmin(granteeId, actorId);
        }
        permissionGrantService.revokeRole(granteeId, roleId);
        eventProducer.publishRoleRevoked(granteeId, role.getName(), actorId);
        auditService.log("role.revoked", actorId, "user:" + granteeId, null,
                Map.of("roleName", role.getName(),
                        "motif", motif == null ? "" : motif), null);
        log.info("Role revoked grantee={} role={} actor={}", granteeId, role.getName(), actorId);
    }

    private static CapabilityService.AdminLevel mapRoleNameToLevel(String roleName) {
        if (roleName == null) return null;
        return switch (roleName) {
            case "SUPER_ADMIN" -> CapabilityService.AdminLevel.SUPER_ADMIN;
            case "ADMIN"       -> CapabilityService.AdminLevel.ADMIN;
            case "MANAGER"     -> CapabilityService.AdminLevel.MANAGER;
            default            -> null;
        };
    }

    @Transactional
    public AdminRoleGrant rejectGrant(UUID grantId, UUID approverId, String reason) {
        AdminRoleGrant g = grantRepo.findById(grantId)
                .orElseThrow(() -> new IllegalArgumentException("grant not found: " + grantId));
        if (g.getStatus() != AdminRoleGrant.Status.PENDING) {
            throw new IllegalStateException("grant is not PENDING");
        }
        g.setStatus(AdminRoleGrant.Status.REJECTED);
        g.setApproverId(approverId);
        g.setRejectedAt(Instant.now());
        g.setRejectionReason(reason);
        grantRepo.save(g);
        auditService.log("grant.rejected", approverId, "grant:" + g.getId(), null,
                Map.of("reason", reason == null ? "" : reason), null);
        return g;
    }

    public List<AdminRoleGrant> listPendingForApprover(UUID approverId) {
        return grantRepo.findPendingByApproverId(approverId);
    }

    public List<AdminRoleGrant> listForGrantee(UUID granteeId) {
        return grantRepo.findByGranteeIdOrderByCreatedAtDesc(granteeId);
    }
}
