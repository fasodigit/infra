package bf.gov.faso.auth.security;

import bf.gov.faso.auth.service.JtiBlacklistService;
import bf.gov.faso.auth.service.JwtService;
import com.nimbusds.jwt.JWTClaimsSet;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.Collections;
import java.util.List;
import java.util.Optional;
import java.util.stream.Collectors;

/**
 * JWT authentication filter for management-plane requests.
 * <p>
 * Extracts the Bearer token from the Authorization header, validates it
 * using JwtService, checks the JTI blacklist, and populates the
 * SecurityContext with the authenticated principal.
 * <p>
 * Note: In production, ARMAGEDDON handles JWT validation on the critical path.
 * This filter protects the admin GraphQL API and internal management endpoints.
 */
@Component
public class JwtAuthenticationFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(JwtAuthenticationFilter.class);
    private static final String AUTHORIZATION_HEADER = "Authorization";
    private static final String BEARER_PREFIX = "Bearer ";

    private final JwtService jwtService;
    private final JtiBlacklistService blacklistService;

    public JwtAuthenticationFilter(JwtService jwtService, JtiBlacklistService blacklistService) {
        this.jwtService = jwtService;
        this.blacklistService = blacklistService;
    }

    @Override
    protected void doFilterInternal(HttpServletRequest request,
                                    HttpServletResponse response,
                                    FilterChain filterChain) throws ServletException, IOException {

        String authHeader = request.getHeader(AUTHORIZATION_HEADER);

        if (authHeader == null || !authHeader.startsWith(BEARER_PREFIX)) {
            filterChain.doFilter(request, response);
            return;
        }

        String token = authHeader.substring(BEARER_PREFIX.length());

        try {
            Optional<JWTClaimsSet> claimsOpt = jwtService.verifyToken(token);

            if (claimsOpt.isEmpty()) {
                log.debug("JWT verification failed for request to {}", request.getRequestURI());
                filterChain.doFilter(request, response);
                return;
            }

            JWTClaimsSet claims = claimsOpt.get();

            // Check JTI blacklist
            String jti = claims.getJWTID();
            if (jti != null && blacklistService.isBlacklisted(jti)) {
                log.info("Rejected blacklisted JWT jti={} for sub={}", jti, claims.getSubject());
                response.setStatus(HttpServletResponse.SC_UNAUTHORIZED);
                response.getWriter().write("{\"error\":\"token_revoked\",\"message\":\"This token has been revoked\"}");
                response.setContentType("application/json");
                return;
            }

            // Extract roles from the claims
            List<String> roles = extractRoles(claims);
            List<SimpleGrantedAuthority> authorities = roles.stream()
                    .map(role -> new SimpleGrantedAuthority("ROLE_" + role))
                    .collect(Collectors.toList());

            // Build authentication token with user ID as principal
            JwtAuthenticatedPrincipal principal = new JwtAuthenticatedPrincipal(
                    claims.getSubject(),
                    claims.getStringClaim("email"),
                    roles,
                    jti
            );

            UsernamePasswordAuthenticationToken authToken =
                    new UsernamePasswordAuthenticationToken(principal, null, authorities);

            SecurityContextHolder.getContext().setAuthentication(authToken);

            log.debug("Authenticated user sub={} with roles={}", claims.getSubject(), roles);
        } catch (Exception e) {
            log.error("JWT authentication error: {}", e.getMessage());
        }

        filterChain.doFilter(request, response);
    }

    @SuppressWarnings("unchecked")
    private List<String> extractRoles(JWTClaimsSet claims) {
        try {
            Object rolesObj = claims.getClaim("roles");
            if (rolesObj instanceof List) {
                return (List<String>) rolesObj;
            }
        } catch (Exception e) {
            log.warn("Failed to extract roles from JWT claims: {}", e.getMessage());
        }
        return Collections.emptyList();
    }
}
