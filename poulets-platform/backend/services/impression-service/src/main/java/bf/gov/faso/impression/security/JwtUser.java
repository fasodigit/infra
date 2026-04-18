package bf.gov.faso.impression.security;

import org.springframework.security.core.GrantedAuthority;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.oauth2.jwt.Jwt;

import java.util.*;

/**
 * Custom user principal extracted from JWT token.
 */
public class JwtUser {

    private final UUID userId;
    private final String tenantId;
    private final String email;
    private final String fullName;
    private final Set<String> roles;
    private final Jwt jwt;

    public JwtUser(Jwt jwt) {
        this.jwt = jwt;
        this.userId = extractUserId(jwt);
        this.tenantId = extractTenantId(jwt);
        this.email = jwt.getClaimAsString("email");
        this.fullName = jwt.getClaimAsString("name");
        this.roles = extractRoles(jwt);
    }

    /**
     * Dev constructor for creating a fake user without JWT (dev mode only).
     */
    public JwtUser(UUID userId, String tenantId, String email, String fullName, Set<String> roles) {
        this.jwt = null;
        this.userId = userId;
        this.tenantId = tenantId;
        this.email = email;
        this.fullName = fullName;
        this.roles = roles;
    }

    private UUID extractUserId(Jwt jwt) {
        String sub = jwt.getSubject();
        try {
            return UUID.fromString(sub);
        } catch (IllegalArgumentException e) {
            // If subject is not a UUID, generate one from the subject string
            return UUID.nameUUIDFromBytes(sub.getBytes());
        }
    }

    private String extractTenantId(Jwt jwt) {
        // Try multiple claim names for tenant ID
        String tenantId = jwt.getClaimAsString("tenant_id");
        if (tenantId == null) {
            tenantId = jwt.getClaimAsString("tenantId");
        }
        if (tenantId == null) {
            tenantId = jwt.getClaimAsString("org_id");
        }
        return tenantId != null ? tenantId : "default";
    }

    @SuppressWarnings("unchecked")
    private Set<String> extractRoles(Jwt jwt) {
        Set<String> extractedRoles = new HashSet<>();

        // Try realm_access (Keycloak format)
        Map<String, Object> realmAccess = jwt.getClaimAsMap("realm_access");
        if (realmAccess != null && realmAccess.containsKey("roles")) {
            extractedRoles.addAll((Collection<String>) realmAccess.get("roles"));
        }

        // Try roles claim directly
        List<String> rolesClaim = jwt.getClaimAsStringList("roles");
        if (rolesClaim != null) {
            extractedRoles.addAll(rolesClaim);
        }

        // Try groups claim (Ory format)
        List<String> groups = jwt.getClaimAsStringList("groups");
        if (groups != null) {
            extractedRoles.addAll(groups);
        }

        // Try scope claim
        String scope = jwt.getClaimAsString("scope");
        if (scope != null) {
            Arrays.stream(scope.split(" "))
                .filter(s -> s.startsWith("ROLE_") || s.startsWith("role_"))
                .forEach(extractedRoles::add);
        }

        return extractedRoles;
    }

    public UUID getUserId() {
        return userId;
    }

    public String getTenantId() {
        return tenantId;
    }

    public String getEmail() {
        return email;
    }

    public String getFullName() {
        return fullName;
    }

    public Set<String> getRoles() {
        return Collections.unmodifiableSet(roles);
    }

    public boolean hasRole(String role) {
        return roles.contains(role) ||
               roles.contains("ROLE_" + role) ||
               roles.contains(role.replace("ROLE_", ""));
    }

    public Collection<GrantedAuthority> getAuthorities() {
        List<GrantedAuthority> authorities = new ArrayList<>();
        for (String role : roles) {
            String normalizedRole = role.startsWith("ROLE_") ? role : "ROLE_" + role;
            authorities.add(new SimpleGrantedAuthority(normalizedRole));
        }
        return authorities;
    }

    public Jwt getJwt() {
        return jwt;
    }

    @Override
    public String toString() {
        return "JwtUser{" +
                "userId=" + userId +
                ", tenantId='" + tenantId + '\'' +
                ", email='" + email + '\'' +
                ", roles=" + roles +
                '}';
    }
}
