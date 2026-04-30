// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AccountCapabilityGrant;
import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.AccountCapabilityGrantRepository;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.KetoService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.stream.Collectors;

/**
 * Per-user fine-grained capability management — delta amendment 2026-04-30.
 *
 * <p>Two ADMIN (or two MANAGER) accounts MUST NOT share exactly the same
 * active capability set. Enforcement is soft (UI warning + force-override by
 * SUPER_ADMIN) — see {@link #checkUniqueness}. SUPER_ADMIN accounts always
 * receive the full set and the rule does not apply.
 *
 * <p>Each grant is mirrored to Keto as a tuple
 * {@code Capability:<key>#granted@<userId>} so that ARMAGEDDON/permission
 * checks resolve fast (Zanzibar read).
 */
@Service
public class CapabilityService {

    private static final Logger log = LoggerFactory.getLogger(CapabilityService.class);

    /** Keto namespace for capability tuples. */
    public static final String KETO_NAMESPACE = "Capability";
    /** Keto relation linking a subject to a capability object. */
    public static final String KETO_RELATION = "granted";

    public enum AdminLevel { SUPER_ADMIN, ADMIN, MANAGER }

    private final AccountCapabilityGrantRepository grantRepo;
    private final UserRepository userRepo;
    private final KetoService ketoService;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;

    public CapabilityService(AccountCapabilityGrantRepository grantRepo,
                             UserRepository userRepo,
                             KetoService ketoService,
                             AdminAuditService auditService,
                             AdminEventProducer eventProducer) {
        this.grantRepo = grantRepo;
        this.userRepo = userRepo;
        this.ketoService = ketoService;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
    }

    /** Active capability keys held by a user. */
    public Set<String> getCapabilitiesForUser(UUID userId) {
        return grantRepo.findActiveByUserId(userId).stream()
                .map(AccountCapabilityGrant::getCapabilityKey)
                .collect(Collectors.toCollection(HashSet::new));
    }

    @Transactional
    public List<AccountCapabilityGrant> grantCapabilities(UUID userId,
                                                          Set<String> caps,
                                                          AdminLevel forRole,
                                                          UUID grantorId,
                                                          String motif) {
        if (caps == null || caps.isEmpty()) return List.of();
        Set<String> existing = getCapabilitiesForUser(userId);
        List<AccountCapabilityGrant> created = new ArrayList<>();
        for (String key : caps) {
            if (existing.contains(key)) continue;
            AccountCapabilityGrant g = new AccountCapabilityGrant();
            g.setUserId(userId);
            g.setCapabilityKey(key);
            g.setGrantedBy(grantorId);
            g.setGrantedAt(Instant.now());
            g.setGrantedForRole(forRole == null ? null : forRole.name());
            g.setMotif(motif);
            created.add(grantRepo.save(g));

            // Mirror to Keto (best-effort — circuit-breaker handles outages).
            ketoService.writeRelationTuple(KETO_NAMESPACE, key, KETO_RELATION,
                    userId.toString());

            auditService.log(AuditAction.CAPABILITY_GRANTED.key(), grantorId,
                    "user:" + userId, null,
                    Map.of("capability", key,
                            "forRole", forRole == null ? "" : forRole.name(),
                            "motif", motif == null ? "" : motif),
                    null);
            eventProducer.publishCapabilityGranted(userId, key,
                    forRole == null ? null : forRole.name(), grantorId, motif);
        }
        log.info("Granted {} capabilities to user={} forRole={} by={}",
                created.size(), userId, forRole, grantorId);
        return created;
    }

    @Transactional
    public int revokeCapabilities(UUID userId, Set<String> caps, UUID actorId, String motif) {
        if (caps == null || caps.isEmpty()) return 0;
        int revoked = 0;
        Instant now = Instant.now();
        for (String key : caps) {
            for (AccountCapabilityGrant g : grantRepo.findActiveByUserAndKey(userId, key)) {
                g.setRevokedAt(now);
                g.setRevokedBy(actorId);
                grantRepo.save(g);
                ketoService.deleteRelationTuple(KETO_NAMESPACE, key, KETO_RELATION,
                        userId.toString());
                auditService.log(AuditAction.CAPABILITY_REVOKED.key(), actorId,
                        "user:" + userId, null,
                        Map.of("capability", key,
                                "motif", motif == null ? "" : motif), null);
                eventProducer.publishCapabilityRevoked(userId, key, actorId, motif);
                revoked++;
            }
        }
        log.info("Revoked {} capability records for user={} by={}", revoked, userId, actorId);
        return revoked;
    }

    /**
     * Soft uniqueness check used by the BFF before commit. If another user
     * holding role {@code role} has exactly the same active capability set,
     * surface that user so the UI can warn the SUPER_ADMIN. SUPER_ADMIN role
     * is exempt — duplicate sets are expected and allowed for SAs.
     */
    public UniquenessReport checkUniqueness(Set<String> caps, AdminLevel role) {
        UniquenessReport rpt = new UniquenessReport();
        if (caps == null || caps.isEmpty()) return rpt;
        if (role == AdminLevel.SUPER_ADMIN) return rpt; // exempt — see delta §1.

        // Pick the smallest cap to bound the candidate set quickly.
        String pivot = caps.iterator().next();
        List<UUID> candidates = grantRepo.findUsersWithActiveCapability(pivot);
        for (UUID candidate : candidates) {
            Set<String> theirs = getCapabilitiesForUser(candidate);
            if (theirs.equals(caps)) {
                userRepo.findById(candidate).ifPresent(u -> rpt.duplicates.add(toDuplicate(u)));
            }
        }
        return rpt;
    }

    private static Map<String, String> toDuplicate(User u) {
        Map<String, String> m = new LinkedHashMap<>();
        m.put("userId", u.getId().toString());
        m.put("email", u.getEmail());
        m.put("firstName", u.getFirstName() == null ? "" : u.getFirstName());
        m.put("lastName", u.getLastName() == null ? "" : u.getLastName());
        return m;
    }

    public static class UniquenessReport {
        public final List<Map<String, String>> duplicates = new ArrayList<>();
        public boolean hasDuplicates() { return !duplicates.isEmpty(); }
    }
}
