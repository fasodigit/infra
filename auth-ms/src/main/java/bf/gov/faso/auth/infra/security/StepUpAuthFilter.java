// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.infra.security;

import bf.gov.faso.auth.controller.admin.RequiresStepUp;
import bf.gov.faso.auth.repository.AdminSettingRepository;
import bf.gov.faso.auth.security.JwtAuthenticatedPrincipal;
import bf.gov.faso.auth.security.StepUpMethod;
import bf.gov.faso.auth.service.JwtService;
import bf.gov.faso.auth.service.admin.StepUpAuthService;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.nimbusds.jwt.JWTClaimsSet;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;
import org.springframework.web.method.HandlerMethod;
import org.springframework.web.servlet.HandlerExecutionChain;
import org.springframework.web.servlet.HandlerMapping;

import java.io.IOException;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;

/**
 * Phase 4.b.7 — Step-up auth filter.
 *
 * <p>Inspects each request: if the matched controller method bears
 * {@link RequiresStepUp} and the JWT claim {@code last_step_up_at} is
 * absent or older than {@code maxAgeSeconds()}, the filter responds with
 * HTTP 401 + JSON body
 * <pre>
 *   {
 *     "error": "step_up_required",
 *     "methods_available": ["passkey","push-approval","totp","otp"],
 *     "step_up_session_id": "uuid",
 *     "expires_at": "iso8601"
 *   }
 * </pre>
 *
 * <p>The filter MUST run AFTER {@link bf.gov.faso.auth.security.JwtAuthenticationFilter}
 * (which populates {@link SecurityContextHolder}) but BEFORE the controller
 * dispatch. We schedule it after JWT auth in {@code SecurityConfig}.
 *
 * <p>{@code AdminSettingsController.update} is sensitive ONLY when the touched
 * key belongs to a sensitive category (audit / mfa / grant / break_glass);
 * the filter resolves the key from the path variable {@code key} and
 * suppresses the step-up for non-sensitive categories.
 */
@Component
public class StepUpAuthFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(StepUpAuthFilter.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** Settings categories that always require step-up. */
    private static final Set<String> SENSITIVE_CATEGORIES =
            Set.of("audit", "mfa", "grant", "break_glass");

    private final JwtService jwtService;
    private final StepUpAuthService stepUpService;
    private final AdminSettingRepository settingRepository;
    private final HandlerMapping handlerMapping;

    @Value("${admin.step-up.default-max-age-seconds:300}")
    private int defaultMaxAgeSeconds;

    public StepUpAuthFilter(JwtService jwtService,
                            StepUpAuthService stepUpService,
                            AdminSettingRepository settingRepository,
                            // Phase 4.b.7 stub — wired by the dispatch context once
                            // step-up dispatch resolution is finalised. The qualifier
                            // pins the @RequestMapping-based mapping; without it
                            // Spring sees 12 HandlerMapping beans (welcomePage,
                            // webSocket, resource, …) and refuses to autowire.
                            @org.springframework.beans.factory.annotation.Qualifier("requestMappingHandlerMapping")
                            HandlerMapping handlerMapping) {
        this.jwtService = jwtService;
        this.stepUpService = stepUpService;
        this.settingRepository = settingRepository;
        this.handlerMapping = handlerMapping;
    }

    @Override
    protected void doFilterInternal(HttpServletRequest request,
                                    HttpServletResponse response,
                                    FilterChain filterChain) throws ServletException, IOException {

        // Match request to controller method.
        HandlerMethod handlerMethod = resolveHandlerMethod(request);
        if (handlerMethod == null) {
            filterChain.doFilter(request, response);
            return;
        }

        RequiresStepUp annotation = handlerMethod.getMethodAnnotation(RequiresStepUp.class);
        if (annotation == null) {
            filterChain.doFilter(request, response);
            return;
        }

        // Conditional enforcement for AdminSettingsController.update — only
        // when the targeted key belongs to a sensitive category.
        if (annotation.settingsCategories().length > 0
                && !shouldEnforceForSetting(request, annotation.settingsCategories())) {
            filterChain.doFilter(request, response);
            return;
        }

        // Pull the JWT claim "last_step_up_at" from the SecurityContext-attached
        // principal (or by re-parsing the Bearer token if unavailable).
        Long lastStepUpAt = readLastStepUpAtClaim(request);
        long maxAgeMs = (annotation.maxAgeSeconds() > 0
                ? annotation.maxAgeSeconds()
                : defaultMaxAgeSeconds) * 1000L;
        long now = System.currentTimeMillis();

        if (lastStepUpAt != null && (now - lastStepUpAt) <= maxAgeMs) {
            // Fresh step-up — let the request through.
            filterChain.doFilter(request, response);
            return;
        }

        // Need step-up. Resolve current user (must be authenticated already).
        Optional<UUID> userIdOpt = currentUserId();
        if (userIdOpt.isEmpty()) {
            // No principal — let downstream auth produce a 401.
            filterChain.doFilter(request, response);
            return;
        }

        String requestedFor = request.getMethod() + " " + request.getRequestURI();
        List<StepUpMethod> allowed = Arrays.asList(annotation.allowedMethods());
        Map<String, Object> body = stepUpService.openSessionForFilter(
                userIdOpt.get(), requestedFor, allowed);

        response.setStatus(HttpServletResponse.SC_UNAUTHORIZED);
        response.setContentType("application/json");
        response.getWriter().write(MAPPER.writeValueAsString(body));
        log.info("Step-up enforced — user={} method={} uri={}",
                userIdOpt.get(), request.getMethod(), request.getRequestURI());
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    private HandlerMethod resolveHandlerMethod(HttpServletRequest request) {
        try {
            HandlerExecutionChain chain = handlerMapping.getHandler(request);
            if (chain == null) return null;
            Object handler = chain.getHandler();
            return handler instanceof HandlerMethod hm ? hm : null;
        } catch (Exception e) {
            log.debug("StepUpAuthFilter handler resolution failed: {}", e.getMessage());
            return null;
        }
    }

    private boolean shouldEnforceForSetting(HttpServletRequest request, String[] categories) {
        // Path: PUT /admin/settings/{key}
        String uri = request.getRequestURI();
        int idx = uri.indexOf("/admin/settings/");
        if (idx < 0) return true; // unknown shape — fail-closed
        String key = uri.substring(idx + "/admin/settings/".length());
        // Strip any sub-path (e.g. /history, /revert)
        int slash = key.indexOf('/');
        if (slash > 0) key = key.substring(0, slash);
        if (key.isBlank()) return true;
        try {
            String cat = settingRepository.findById(key)
                    .map(s -> s.getCategory())
                    .orElse(null);
            if (cat == null) return true;
            // Enforce if either the annotation declares the category OR the
            // category is in the global sensitive set.
            for (String c : categories) {
                if (c.equalsIgnoreCase(cat)) return true;
            }
            return SENSITIVE_CATEGORIES.contains(cat);
        } catch (Exception e) {
            log.warn("settings step-up enforcement check failed key={}: {}", key, e.getMessage());
            return true; // fail-closed
        }
    }

    private Long readLastStepUpAtClaim(HttpServletRequest request) {
        // Prefer the Bearer token re-parse — it carries the claim verbatim,
        // SecurityContext only exposes a few fields.
        String authHeader = request.getHeader("Authorization");
        if (authHeader == null || !authHeader.regionMatches(true, 0, "Bearer ", 0, 7)) {
            return null;
        }
        String token = authHeader.substring(7);
        Optional<JWTClaimsSet> claims = jwtService.verifyToken(token);
        if (claims.isEmpty()) return null;
        try {
            Object v = claims.get().getClaim("last_step_up_at");
            if (v instanceof Number n) return n.longValue();
            if (v instanceof String s) {
                try { return Long.parseLong(s); } catch (NumberFormatException ignored) {}
            }
        } catch (Exception ignored) {}
        return null;
    }

    private Optional<UUID> currentUserId() {
        Authentication auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null) return Optional.empty();
        Object principal = auth.getPrincipal();
        if (principal instanceof JwtAuthenticatedPrincipal p) {
            try { return Optional.of(UUID.fromString(p.getUserId())); }
            catch (Exception e) { return Optional.empty(); }
        }
        return Optional.empty();
    }
}
