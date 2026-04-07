package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.Permission;
import bf.gov.faso.auth.model.Role;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.RoleRepository;
import bf.gov.faso.auth.repository.UserRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.*;
import java.util.stream.Collectors;

/**
 * Service for granting and revoking permissions through role assignment.
 * Orchestrates between local database changes and Keto synchronization.
 */
@Service
public class PermissionGrantService {

    private static final Logger log = LoggerFactory.getLogger(PermissionGrantService.class);

    private final UserRepository userRepository;
    private final RoleRepository roleRepository;
    private final KetoService ketoService;

    public PermissionGrantService(UserRepository userRepository,
                                  RoleRepository roleRepository,
                                  KetoService ketoService) {
        this.userRepository = userRepository;
        this.roleRepository = roleRepository;
        this.ketoService = ketoService;
    }

    /**
     * Assign a role to a user and sync the permissions to Keto.
     *
     * @param userId the user ID
     * @param roleId the role ID to assign
     * @return the updated user
     */
    @Transactional
    public User assignRole(UUID userId, UUID roleId) {
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + userId));
        Role role = roleRepository.findById(roleId)
                .orElseThrow(() -> new IllegalArgumentException("Role not found: " + roleId));

        if (user.getRoles().contains(role)) {
            log.info("User {} already has role {}", userId, role.getName());
            return user;
        }

        user.getRoles().add(role);
        User saved = userRepository.save(user);

        // Sync the new role assignment to Keto
        String userIdStr = userId.toString();
        ketoService.writeRelationTuple("auth", "roles", role.getName().toLowerCase(), userIdStr);

        // Also sync individual permissions from the role
        for (Permission perm : role.getPermissions()) {
            ketoService.writeRelationTuple(perm.getNamespace(), perm.getObject(), perm.getRelation(), userIdStr);
        }

        log.info("Assigned role '{}' to user {} and synced to Keto", role.getName(), userId);
        return saved;
    }

    /**
     * Revoke a role from a user and remove the permissions from Keto.
     *
     * @param userId the user ID
     * @param roleId the role ID to revoke
     * @return the updated user
     */
    @Transactional
    public User revokeRole(UUID userId, UUID roleId) {
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + userId));
        Role role = roleRepository.findById(roleId)
                .orElseThrow(() -> new IllegalArgumentException("Role not found: " + roleId));

        if (!user.getRoles().contains(role)) {
            log.info("User {} does not have role {}", userId, role.getName());
            return user;
        }

        user.getRoles().remove(role);
        User saved = userRepository.save(user);

        // Remove the role tuple from Keto
        String userIdStr = userId.toString();
        ketoService.deleteRelationTuple("auth", "roles", role.getName().toLowerCase(), userIdStr);

        // Remove permissions that are NOT granted by any other remaining role
        Set<Permission> remainingPerms = user.getRoles().stream()
                .flatMap(r -> r.getPermissions().stream())
                .collect(Collectors.toSet());

        for (Permission perm : role.getPermissions()) {
            if (!remainingPerms.contains(perm)) {
                ketoService.deleteRelationTuple(perm.getNamespace(), perm.getObject(),
                        perm.getRelation(), userIdStr);
            }
        }

        log.info("Revoked role '{}' from user {} and synced to Keto", role.getName(), userId);
        return saved;
    }

    /**
     * Get all effective permissions for a user across all their roles.
     */
    public Set<Permission> getEffectivePermissions(UUID userId) {
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + userId));

        return user.getRoles().stream()
                .flatMap(role -> role.getPermissions().stream())
                .collect(Collectors.toSet());
    }

    /**
     * Get all effective permissions as Zanzibar tuple strings.
     */
    public List<String> getEffectivePermissionTuples(UUID userId) {
        return getEffectivePermissions(userId).stream()
                .map(Permission::toTupleString)
                .sorted()
                .collect(Collectors.toList());
    }
}
