package bf.gov.faso.impression.config;

import bf.gov.faso.impression.security.JwtUser;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.springframework.context.annotation.Profile;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.Set;
import java.util.UUID;

/**
 * Dev-only filter that injects a fake JwtUser into the SecurityContext.
 * This allows controller methods using @AuthenticationPrincipal to work
 * without an actual JWT token.
 */
@Component
@Profile({"dev", "local"})
public class DevAuthenticationFilter extends OncePerRequestFilter {

    private static final JwtUser DEV_USER = new JwtUser(
            UUID.fromString("00000000-0000-0000-0000-000000000001"),
            "default",
            "dev@actes.gov.bf",
            "Dev Operator",
            Set.of("OPERATEUR_IMPRESSION", "MANAGER", "ADMIN")
    );

    @Override
    protected void doFilterInternal(HttpServletRequest request, HttpServletResponse response,
                                     FilterChain filterChain) throws ServletException, IOException {
        if (SecurityContextHolder.getContext().getAuthentication() == null) {
            UsernamePasswordAuthenticationToken auth = new UsernamePasswordAuthenticationToken(
                    DEV_USER, null, DEV_USER.getAuthorities()
            );
            SecurityContextHolder.getContext().setAuthentication(auth);
        }
        filterChain.doFilter(request, response);
    }
}
