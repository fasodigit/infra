package bf.gov.faso.auth.security;

import bf.gov.faso.auth.service.KratosService;
import com.fasterxml.jackson.databind.JsonNode;
import jakarta.servlet.http.Cookie;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;
import org.springframework.web.servlet.HandlerInterceptor;

import java.util.Arrays;
import java.util.Optional;

/**
 * Interceptor that validates Kratos session tokens for admin users
 * who authenticate via Kratos login flow (browser-based admin UI).
 * <p>
 * This is an alternative authentication path to JWT -- admin users
 * who access auth-ms through the Kratos-backed admin login get a
 * Kratos session cookie. This interceptor validates that cookie and
 * populates request attributes with the identity info.
 * <p>
 * The JWT filter handles the primary auth path (service-to-service
 * and API-based access). This interceptor handles the secondary
 * browser-based admin path.
 */
@Component
public class KratosSessionHandshakeInterceptor implements HandlerInterceptor {

    private static final Logger log = LoggerFactory.getLogger(KratosSessionHandshakeInterceptor.class);
    private static final String KRATOS_SESSION_COOKIE = "ory_kratos_session";
    private static final String KRATOS_SESSION_HEADER = "X-Session-Token";

    private final KratosService kratosService;

    public KratosSessionHandshakeInterceptor(KratosService kratosService) {
        this.kratosService = kratosService;
    }

    @Override
    public boolean preHandle(HttpServletRequest request,
                             HttpServletResponse response,
                             Object handler) {

        // Skip if already authenticated via JWT
        if (request.getAttribute("kratosIdentity") != null) {
            return true;
        }

        // Try to extract session token from cookie or header
        String sessionToken = extractSessionToken(request);
        if (sessionToken == null) {
            return true; // No Kratos session, let the filter chain handle it
        }

        // Validate the session with Kratos
        Optional<JsonNode> sessionInfo = kratosService.validateSession(sessionToken);
        if (sessionInfo.isEmpty()) {
            log.debug("Invalid Kratos session token");
            return true;
        }

        JsonNode session = sessionInfo.get();
        JsonNode identity = session.path("identity");

        if (!identity.isMissingNode()) {
            String identityId = identity.path("id").asText();
            String email = identity.path("traits").path("email").asText();

            request.setAttribute("kratosIdentity", identityId);
            request.setAttribute("kratosEmail", email);
            request.setAttribute("kratosSession", session);

            log.debug("Kratos session validated for identity={} email={}", identityId, email);
        }

        return true;
    }

    private String extractSessionToken(HttpServletRequest request) {
        // First check header
        String headerToken = request.getHeader(KRATOS_SESSION_HEADER);
        if (headerToken != null && !headerToken.isBlank()) {
            return headerToken;
        }

        // Then check cookies
        Cookie[] cookies = request.getCookies();
        if (cookies != null) {
            return Arrays.stream(cookies)
                    .filter(c -> KRATOS_SESSION_COOKIE.equals(c.getName()))
                    .map(Cookie::getValue)
                    .findFirst()
                    .orElse(null);
        }

        return null;
    }
}
