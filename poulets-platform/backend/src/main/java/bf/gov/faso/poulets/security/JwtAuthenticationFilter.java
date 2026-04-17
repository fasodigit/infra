package bf.gov.faso.poulets.security;

import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import com.nimbusds.jose.JWSAlgorithm;
import com.nimbusds.jose.jwk.source.RemoteJWKSet;
import com.nimbusds.jose.proc.JWSVerificationKeySelector;
import com.nimbusds.jose.proc.SecurityContext;
import com.nimbusds.jwt.JWTClaimsSet;
import com.nimbusds.jwt.proc.ConfigurableJWTProcessor;
import com.nimbusds.jwt.proc.DefaultJWTProcessor;

import java.io.IOException;
import java.net.URL;
import java.util.Collections;
import java.util.List;
import java.util.stream.Collectors;

/**
 * JWT authentication filter for poulets-api.
 * <p>
 * Validates Bearer tokens issued by auth-ms using the JWKS endpoint.
 * Populates the SecurityContext so method-level {@code @PreAuthorize} works.
 */
@Component
public class JwtAuthenticationFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(JwtAuthenticationFilter.class);
    private static final String BEARER_PREFIX = "Bearer ";

    private final ConfigurableJWTProcessor<SecurityContext> jwtProcessor;

    public JwtAuthenticationFilter(
            @Value("${auth.jwks-uri}") String jwksUri) throws Exception {
        this.jwtProcessor = new DefaultJWTProcessor<>();
        RemoteJWKSet<SecurityContext> jwkSet = new RemoteJWKSet<>(new URL(jwksUri));
        JWSVerificationKeySelector<SecurityContext> keySelector =
                new JWSVerificationKeySelector<>(JWSAlgorithm.ES384, jwkSet);
        this.jwtProcessor.setJWSKeySelector(keySelector);
    }

    @Override
    protected void doFilterInternal(HttpServletRequest request,
                                    HttpServletResponse response,
                                    FilterChain filterChain) throws ServletException, IOException {
        String authHeader = request.getHeader("Authorization");

        if (authHeader == null || !authHeader.startsWith(BEARER_PREFIX)) {
            filterChain.doFilter(request, response);
            return;
        }

        String token = authHeader.substring(BEARER_PREFIX.length()).trim();

        try {
            JWTClaimsSet claims = jwtProcessor.process(token, null);
            List<String> roles = extractRoles(claims);
            List<SimpleGrantedAuthority> authorities = roles.stream()
                    .map(r -> new SimpleGrantedAuthority("ROLE_" + r))
                    .collect(Collectors.toList());

            UsernamePasswordAuthenticationToken auth =
                    new UsernamePasswordAuthenticationToken(claims.getSubject(), null, authorities);
            SecurityContextHolder.getContext().setAuthentication(auth);
            log.debug("Authenticated sub={} roles={}", claims.getSubject(), roles);
        } catch (Exception e) {
            log.debug("JWT validation failed: {}", e.getMessage());
            // Let the request proceed; authorization rules will reject it if needed
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
            log.warn("Could not extract roles from JWT: {}", e.getMessage());
        }
        return Collections.emptyList();
    }
}
