package bf.gov.faso.auth.security;

import java.security.Principal;
import java.util.List;

/**
 * Principal object representing an authenticated JWT user in the SecurityContext.
 */
public class JwtAuthenticatedPrincipal implements Principal {

    private final String userId;
    private final String email;
    private final List<String> roles;
    private final String jti;

    public JwtAuthenticatedPrincipal(String userId, String email, List<String> roles, String jti) {
        this.userId = userId;
        this.email = email;
        this.roles = roles;
        this.jti = jti;
    }

    @Override
    public String getName() {
        return userId;
    }

    public String getUserId() { return userId; }
    public String getEmail() { return email; }
    public List<String> getRoles() { return roles; }
    public String getJti() { return jti; }
}
