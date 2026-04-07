package bf.gov.faso.auth.graphql;

import bf.gov.faso.auth.model.JwtSigningKey;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.*;
import com.netflix.graphql.dgs.DgsComponent;
import com.netflix.graphql.dgs.DgsMutation;
import com.netflix.graphql.dgs.InputArgument;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * DGS mutation data fetcher for auth-ms.
 * Handles all write operations: user CRUD, role assignment, key rotation, token blacklisting.
 */
@DgsComponent
public class AuthMutationDataFetcher {

    private static final Logger log = LoggerFactory.getLogger(AuthMutationDataFetcher.class);

    private final UserRepository userRepository;
    private final PermissionGrantService permissionGrantService;
    private final JwtService jwtService;
    private final JtiBlacklistService blacklistService;
    private final BruteForceService bruteForceService;
    private final KratosService kratosService;
    private final KetoService ketoService;
    private final SessionLimitService sessionLimitService;

    public AuthMutationDataFetcher(UserRepository userRepository,
                                   PermissionGrantService permissionGrantService,
                                   JwtService jwtService,
                                   JtiBlacklistService blacklistService,
                                   BruteForceService bruteForceService,
                                   KratosService kratosService,
                                   KetoService ketoService,
                                   SessionLimitService sessionLimitService) {
        this.userRepository = userRepository;
        this.permissionGrantService = permissionGrantService;
        this.jwtService = jwtService;
        this.blacklistService = blacklistService;
        this.bruteForceService = bruteForceService;
        this.kratosService = kratosService;
        this.ketoService = ketoService;
        this.sessionLimitService = sessionLimitService;
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    @Transactional
    public User createUser(@InputArgument Map<String, Object> input) {
        String email = (String) input.get("email");
        String firstName = (String) input.get("firstName");
        String lastName = (String) input.get("lastName");
        String department = (String) input.get("department");
        String phoneNumber = (String) input.get("phoneNumber");

        if (userRepository.existsByEmail(email)) {
            throw new IllegalArgumentException("A user with email '" + email + "' already exists");
        }

        User user = new User();
        user.setEmail(email);
        user.setFirstName(firstName);
        user.setLastName(lastName);
        user.setDepartment(department);
        user.setPhoneNumber(phoneNumber);
        user.setPasswordExpiresAt(Instant.now().plus(90, ChronoUnit.DAYS));

        User saved = userRepository.save(user);

        // Create corresponding identity in Kratos
        kratosService.createIdentity(saved).ifPresent(kratosId -> {
            saved.setKratosIdentityId(kratosId);
            userRepository.save(saved);
        });

        log.info("Created user: id={} email={}", saved.getId(), email);
        return saved;
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    @Transactional
    public User updateUser(@InputArgument String id, @InputArgument Map<String, Object> input) {
        UUID userId = UUID.fromString(id);
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + id));

        if (input.containsKey("firstName")) {
            user.setFirstName((String) input.get("firstName"));
        }
        if (input.containsKey("lastName")) {
            user.setLastName((String) input.get("lastName"));
        }
        if (input.containsKey("department")) {
            String oldDept = user.getDepartment();
            String newDept = (String) input.get("department");
            user.setDepartment(newDept);

            // Sync department change to Keto
            if (oldDept != null && !oldDept.equals(newDept)) {
                ketoService.deleteRelationTuple("departments", oldDept.toLowerCase(),
                        "member", userId.toString());
            }
            if (newDept != null && !newDept.isBlank()) {
                ketoService.writeRelationTuple("departments", newDept.toLowerCase(),
                        "member", userId.toString());
            }
        }
        if (input.containsKey("phoneNumber")) {
            user.setPhoneNumber((String) input.get("phoneNumber"));
        }
        if (input.containsKey("active")) {
            user.setActive((Boolean) input.get("active"));
        }

        User saved = userRepository.save(user);

        // Sync changes to Kratos
        if (user.getKratosIdentityId() != null) {
            kratosService.updateIdentityTraits(user.getKratosIdentityId(), saved);
        }

        log.info("Updated user: id={}", id);
        return saved;
    }

    @DgsMutation
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    @Transactional
    public boolean deleteUser(@InputArgument String id) {
        UUID userId = UUID.fromString(id);
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + id));

        // Soft delete: deactivate
        user.setActive(false);
        userRepository.save(user);

        // Invalidate all sessions
        sessionLimitService.invalidateAllSessions(userId);

        // Deactivate in Kratos
        if (user.getKratosIdentityId() != null) {
            kratosService.deactivateIdentity(user.getKratosIdentityId());
        }

        // Remove all Keto relation tuples
        for (var role : user.getRoles()) {
            ketoService.deleteRelationTuple("auth", "roles",
                    role.getName().toLowerCase(), userId.toString());
        }

        log.info("Soft-deleted user: id={}", id);
        return true;
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    public User assignRole(@InputArgument String userId, @InputArgument String roleId) {
        return permissionGrantService.assignRole(UUID.fromString(userId), UUID.fromString(roleId));
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    public User revokeRole(@InputArgument String userId, @InputArgument String roleId) {
        return permissionGrantService.revokeRole(UUID.fromString(userId), UUID.fromString(roleId));
    }

    @DgsMutation
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public Map<String, Object> rotateJwtKeys() {
        JwtSigningKey newKey = jwtService.rotateKeys();

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("kid", newKey.getKid());
        result.put("algorithm", newKey.getAlgorithm());
        result.put("rotatedAt", newKey.getCreatedAt().toString());
        result.put("nextRotationAt", newKey.getExpiresAt().toString());

        log.info("JWT keys rotated: new kid={}", newKey.getKid());
        return result;
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    public boolean blacklistToken(@InputArgument String jti, @InputArgument String reason) {
        blacklistService.blacklist(jti, reason);
        log.info("Token blacklisted: jti={} reason={}", jti, reason);
        return true;
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    public boolean unlockAccount(@InputArgument String userId) {
        return bruteForceService.unlockAccount(UUID.fromString(userId));
    }

    @DgsMutation
    @PreAuthorize("hasAnyRole('SUPER_ADMIN', 'ADMIN')")
    @Transactional
    public boolean forcePasswordReset(@InputArgument String userId) {
        UUID uid = UUID.fromString(userId);
        User user = userRepository.findById(uid)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + userId));

        // Force password to expired state
        user.setPasswordExpiresAt(Instant.now().minusSeconds(1));
        userRepository.save(user);

        // Invalidate all sessions
        sessionLimitService.invalidateAllSessions(uid);

        // Create recovery link in Kratos
        if (user.getKratosIdentityId() != null) {
            kratosService.createRecoveryLink(user.getKratosIdentityId());
        }

        log.info("Forced password reset for userId={}", userId);
        return true;
    }
}
