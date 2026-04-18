package bf.gov.actes.security.permission;

/**
 * UI-level permission constants used in JWT claims and frontend guards.
 *
 * <p>These are SEPARATE from OPA policy evaluations. OPA handles fine-grained
 * resource-level decisions (role + resource + action + context), while these
 * permissions control UI visibility and coarse-grained API access.</p>
 *
 * <p>Permissions are included in JWT claims as {@code "permissions": ["user:create", ...]}.
 * Frontend apps use these to show/hide UI elements. Backend services can use them
 * for quick pre-checks before delegating to OPA for the full policy evaluation.</p>
 *
 * <p>This class is shared across all projects (ETAT-CIVIL, SOGESY, FASO_KALAN, etc.)
 * via the security-config module.</p>
 */
public final class PermissionConstants {

    private PermissionConstants() {}

    // ═══════════════════════════════════════════════════════════════════
    // User Management
    // ═══════════════════════════════════════════════════════════════════
    public static final String USER_CREATE = "user:create";
    public static final String USER_READ = "user:read";
    public static final String USER_UPDATE = "user:update";
    public static final String USER_DEACTIVATE = "user:deactivate";
    public static final String USER_DELETE = "user:delete";
    public static final String USER_RESET_PASSWORD = "user:reset-password";
    public static final String USER_ASSIGN_ROLE = "user:assign-role";

    // ═══════════════════════════════════════════════════════════════════
    // Tenant Management
    // ═══════════════════════════════════════════════════════════════════
    public static final String TENANT_CREATE = "tenant:create";
    public static final String TENANT_READ = "tenant:read";
    public static final String TENANT_UPDATE = "tenant:update";
    public static final String TENANT_DELETE = "tenant:delete";
    public static final String TENANT_QUOTA_MANAGE = "tenant:quota-manage";

    // ═══════════════════════════════════════════════════════════════════
    // Operator Management
    // ═══════════════════════════════════════════════════════════════════
    public static final String OPERATOR_CREATE = "operator:create";
    public static final String OPERATOR_READ = "operator:read";
    public static final String OPERATOR_UPDATE = "operator:update";
    public static final String OPERATOR_DEACTIVATE = "operator:deactivate";
    public static final String OPERATOR_WORKLOAD_VIEW = "operator:workload-view";

    // ═══════════════════════════════════════════════════════════════════
    // Document Operations
    // ═══════════════════════════════════════════════════════════════════
    public static final String DOCUMENT_TYPE_MANAGE = "document:type-manage";
    public static final String DOCUMENT_OVERSIGHT = "document:oversight";
    public static final String DOCUMENT_SEARCH = "document:search";
    public static final String DOCUMENT_EXPORT = "document:export";
    public static final String DOCUMENT_STATISTICS = "document:statistics";

    // ═══════════════════════════════════════════════════════════════════
    // Analytics
    // ═══════════════════════════════════════════════════════════════════
    public static final String ANALYTICS_BASIC = "analytics:basic";
    public static final String ANALYTICS_DETAILED = "analytics:detailed";
    public static final String ANALYTICS_EXPORT = "analytics:export";
    public static final String ANALYTICS_REAL_TIME = "analytics:real-time";

    // ═══════════════════════════════════════════════════════════════════
    // Finance
    // ═══════════════════════════════════════════════════════════════════
    public static final String FINANCE_VIEW_REVENUE = "finance:view-revenue";
    public static final String FINANCE_VIEW_TARIFFS = "finance:view-tariffs";
    public static final String FINANCE_MANAGE_TARIFFS = "finance:manage-tariffs";
    public static final String FINANCE_EXPORT = "finance:export";

    // ═══════════════════════════════════════════════════════════════════
    // System
    // ═══════════════════════════════════════════════════════════════════
    public static final String SYSTEM_SETTINGS = "system:settings";
    public static final String SYSTEM_FEATURE_FLAGS = "system:feature-flags";

    // ═══════════════════════════════════════════════════════════════════
    // Security
    // ═══════════════════════════════════════════════════════════════════
    public static final String SECURITY_AUDIT_VIEW = "security:audit-view";
    public static final String SECURITY_AUDIT_EXPORT = "security:audit-export";
    public static final String SECURITY_EMERGENCY_LOCKDOWN = "security:emergency-lockdown";
    public static final String SECURITY_ROLE_MANAGE = "security:role-manage";

    // ═══════════════════════════════════════════════════════════════════
    // Monitoring
    // ═══════════════════════════════════════════════════════════════════
    public static final String MONITORING_HEALTH = "monitoring:health";
    public static final String MONITORING_CIRCUIT_BREAKERS = "monitoring:circuit-breakers";
    public static final String MONITORING_METRICS = "monitoring:metrics";
    public static final String MONITORING_LOGS = "monitoring:logs";

    // ═══════════════════════════════════════════════════════════════════
    // Notifications
    // ═══════════════════════════════════════════════════════════════════
    public static final String NOTIFICATION_VIEW = "notification:view";
    public static final String NOTIFICATION_MANAGE = "notification:manage";
}
