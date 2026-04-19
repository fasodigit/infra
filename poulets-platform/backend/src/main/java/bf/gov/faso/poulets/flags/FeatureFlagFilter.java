// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.poulets.flags;

import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.core.Ordered;
import org.springframework.core.annotation.Order;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/**
 * Servlet filter qui injecte l'en-tête {@code X-Faso-Features: feat1,feat2,…}
 * dans les réponses HTTP sortantes, pour que le frontend Angular (et
 * ARMAGEDDON en mode downstream) sache quels flags sont actifs pour le
 * porteur du JWT courant.
 *
 * <p>Ordre d'exécution : après {@code JwtAuthenticationFilter} (Spring Security
 * a déjà peuplé l'{@code Authentication} quand on passe ici).</p>
 */
@Component
@Order(Ordered.LOWEST_PRECEDENCE - 10)
public class FeatureFlagFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(FeatureFlagFilter.class);
    private static final String HEADER = "X-Faso-Features";

    private final FeatureFlagsService flags;

    public FeatureFlagFilter(FeatureFlagsService flags) {
        this.flags = flags;
    }

    @Override
    protected void doFilterInternal(HttpServletRequest req,
                                    HttpServletResponse res,
                                    FilterChain chain) throws ServletException, IOException {
        try {
            Map<String, Object> attrs = extractAttributes(req);
            String header = flags.activeFlagsHeader(attrs);
            if (header != null && !header.isEmpty()) {
                res.setHeader(HEADER, header);
            }
        } catch (Exception e) {
            // Fail-open : un flag system down ne doit jamais bloquer une requête métier.
            log.debug("FeatureFlagFilter skipped: {}", e.getMessage());
        }
        chain.doFilter(req, res);
    }

    private Map<String, Object> extractAttributes(HttpServletRequest req) {
        Map<String, Object> attrs = new HashMap<>();
        String userId = req.getHeader("X-User-Id");
        if (userId != null && !userId.isEmpty()) {
            attrs.put("user_id", userId);
        }
        String role = req.getHeader("X-User-Role");
        if (role != null && !role.isEmpty()) {
            attrs.put("role", role);
        }
        String region = req.getHeader("X-User-Region");
        if (region != null && !region.isEmpty()) {
            attrs.put("region", region);
        }
        return attrs;
    }

    /** Ne pas filtrer les healthchecks / métriques. */
    @Override
    protected boolean shouldNotFilter(HttpServletRequest request) {
        String p = request.getRequestURI();
        return p.startsWith("/actuator") || p.startsWith("/health") || p.startsWith("/metrics");
    }
}
