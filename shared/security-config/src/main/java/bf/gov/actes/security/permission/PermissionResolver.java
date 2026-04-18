package bf.gov.actes.security.permission;

import java.util.Collections;
import java.util.EnumMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

import static bf.gov.actes.security.permission.PermissionConstants.*;

/**
 * Resolves the set of UI-level permissions for a given role.
 *
 * <p>The permission hierarchy is strictly ordered:</p>
 * <ol>
 *   <li><strong>SUPER_ADMIN</strong> — ALL permissions (full platform control)</li>
 *   <li><strong>ADMIN</strong> — All except system:*, security:emergency-lockdown,
 *       security:role-manage, finance:manage-tariffs, user:delete, tenant:create, tenant:delete</li>
 *   <li><strong>MANAGER</strong> — Operator management, analytics, finance (view-only),
 *       document oversight, monitoring health, audit view, notifications</li>
 *   <li><strong>VIEWER</strong> — analytics:basic, analytics:detailed, document:statistics,
 *       monitoring:health (strictly read-only)</li>
 * </ol>
 *
 * <p>This class is thread-safe. The permission sets are computed once at class load
 * and stored in an immutable map.</p>
 */
public final class PermissionResolver {

    /**
     * Internal enum mirroring the admin-tier roles for type-safe mapping.
     * We use a local enum rather than depending on opa-core's Role enum
     * to keep security-config independent of opa-core.
     */
    private enum AdminRole {
        SUPER_ADMIN, ADMIN, MANAGER, VIEWER
    }

    /** All defined permissions (SUPER_ADMIN gets all of these). */
    private static final Set<String> ALL_PERMISSIONS;

    /** Pre-computed immutable permission sets per role. */
    private static final Map<AdminRole, Set<String>> ROLE_PERMISSIONS;

    static {
        // ── Collect all permissions ──
        Set<String> all = new LinkedHashSet<>();
        // User Management
        all.add(USER_CREATE);
        all.add(USER_READ);
        all.add(USER_UPDATE);
        all.add(USER_DEACTIVATE);
        all.add(USER_DELETE);
        all.add(USER_RESET_PASSWORD);
        all.add(USER_ASSIGN_ROLE);
        // Tenant Management
        all.add(TENANT_CREATE);
        all.add(TENANT_READ);
        all.add(TENANT_UPDATE);
        all.add(TENANT_DELETE);
        all.add(TENANT_QUOTA_MANAGE);
        // Operator Management
        all.add(OPERATOR_CREATE);
        all.add(OPERATOR_READ);
        all.add(OPERATOR_UPDATE);
        all.add(OPERATOR_DEACTIVATE);
        all.add(OPERATOR_WORKLOAD_VIEW);
        // Document Operations
        all.add(DOCUMENT_TYPE_MANAGE);
        all.add(DOCUMENT_OVERSIGHT);
        all.add(DOCUMENT_SEARCH);
        all.add(DOCUMENT_EXPORT);
        all.add(DOCUMENT_STATISTICS);
        // Analytics
        all.add(ANALYTICS_BASIC);
        all.add(ANALYTICS_DETAILED);
        all.add(ANALYTICS_EXPORT);
        all.add(ANALYTICS_REAL_TIME);
        // Finance
        all.add(FINANCE_VIEW_REVENUE);
        all.add(FINANCE_VIEW_TARIFFS);
        all.add(FINANCE_MANAGE_TARIFFS);
        all.add(FINANCE_EXPORT);
        // System
        all.add(SYSTEM_SETTINGS);
        all.add(SYSTEM_FEATURE_FLAGS);
        // Security
        all.add(SECURITY_AUDIT_VIEW);
        all.add(SECURITY_AUDIT_EXPORT);
        all.add(SECURITY_EMERGENCY_LOCKDOWN);
        all.add(SECURITY_ROLE_MANAGE);
        // Monitoring
        all.add(MONITORING_HEALTH);
        all.add(MONITORING_CIRCUIT_BREAKERS);
        all.add(MONITORING_METRICS);
        all.add(MONITORING_LOGS);
        // Notifications
        all.add(NOTIFICATION_VIEW);
        all.add(NOTIFICATION_MANAGE);

        ALL_PERMISSIONS = Collections.unmodifiableSet(all);

        // ── Build per-role permission sets ──
        Map<AdminRole, Set<String>> map = new EnumMap<>(AdminRole.class);

        // SUPER_ADMIN: everything
        map.put(AdminRole.SUPER_ADMIN, ALL_PERMISSIONS);

        // ADMIN: everything except system:*, security:emergency-lockdown,
        // security:role-manage, finance:manage-tariffs, user:delete, tenant:create, tenant:delete
        Set<String> admin = new LinkedHashSet<>(ALL_PERMISSIONS);
        admin.remove(SYSTEM_SETTINGS);
        admin.remove(SYSTEM_FEATURE_FLAGS);
        admin.remove(SECURITY_EMERGENCY_LOCKDOWN);
        admin.remove(SECURITY_ROLE_MANAGE);
        admin.remove(FINANCE_MANAGE_TARIFFS);
        admin.remove(USER_DELETE);
        admin.remove(TENANT_CREATE);
        admin.remove(TENANT_DELETE);
        map.put(AdminRole.ADMIN, Collections.unmodifiableSet(admin));

        // MANAGER: operator management, analytics, finance (view), document oversight,
        // monitoring health, security audit view, notifications, user CRUD (operators only), tenant read
        Set<String> manager = new LinkedHashSet<>();
        // Operator management
        manager.add(OPERATOR_CREATE);
        manager.add(OPERATOR_READ);
        manager.add(OPERATOR_UPDATE);
        manager.add(OPERATOR_DEACTIVATE);
        manager.add(OPERATOR_WORKLOAD_VIEW);
        // User management (limited to operators)
        manager.add(USER_CREATE);
        manager.add(USER_READ);
        manager.add(USER_UPDATE);
        manager.add(USER_DEACTIVATE);
        manager.add(USER_RESET_PASSWORD);
        // Tenant (read-only)
        manager.add(TENANT_READ);
        // Analytics
        manager.add(ANALYTICS_BASIC);
        manager.add(ANALYTICS_DETAILED);
        manager.add(ANALYTICS_REAL_TIME);
        // Finance (view-only)
        manager.add(FINANCE_VIEW_REVENUE);
        manager.add(FINANCE_VIEW_TARIFFS);
        // Documents
        manager.add(DOCUMENT_OVERSIGHT);
        manager.add(DOCUMENT_SEARCH);
        manager.add(DOCUMENT_STATISTICS);
        // Monitoring
        manager.add(MONITORING_HEALTH);
        // Security
        manager.add(SECURITY_AUDIT_VIEW);
        // Notifications
        manager.add(NOTIFICATION_VIEW);
        map.put(AdminRole.MANAGER, Collections.unmodifiableSet(manager));

        // VIEWER: strictly read-only basics
        Set<String> viewer = new LinkedHashSet<>();
        viewer.add(ANALYTICS_BASIC);
        viewer.add(ANALYTICS_DETAILED);
        viewer.add(DOCUMENT_STATISTICS);
        viewer.add(MONITORING_HEALTH);
        map.put(AdminRole.VIEWER, Collections.unmodifiableSet(viewer));

        ROLE_PERMISSIONS = Collections.unmodifiableMap(map);
    }

    private PermissionResolver() {}

    /**
     * Resolves the set of UI-level permissions for the given role name.
     *
     * @param role the role name (case-insensitive, e.g. "SUPER_ADMIN", "admin", "Manager")
     * @return an unmodifiable set of permission strings, or an empty set if the role
     *         is not an admin-tier role (operational roles like AGENT_TRAITEMENT
     *         do not carry UI permissions — they rely on OPA for authorization)
     */
    public static Set<String> resolvePermissions(String role) {
        if (role == null || role.isBlank()) {
            return Collections.emptySet();
        }
        try {
            AdminRole adminRole = AdminRole.valueOf(role.toUpperCase().replace("-", "_").replace(" ", "_"));
            return ROLE_PERMISSIONS.getOrDefault(adminRole, Collections.emptySet());
        } catch (IllegalArgumentException e) {
            // Not an admin-tier role (e.g., AGENT_TRAITEMENT, CITOYEN) — no UI permissions
            return Collections.emptySet();
        }
    }

    /**
     * Returns the complete set of all defined permissions.
     * Useful for admin UIs that need to display the full permission catalog.
     *
     * @return an unmodifiable set of all permission strings
     */
    public static Set<String> allPermissions() {
        return ALL_PERMISSIONS;
    }

    /**
     * Checks whether a role has a specific permission.
     *
     * @param role       the role name
     * @param permission the permission string (e.g. "user:create")
     * @return true if the role has the permission
     */
    public static boolean hasPermission(String role, String permission) {
        return resolvePermissions(role).contains(permission);
    }
}
