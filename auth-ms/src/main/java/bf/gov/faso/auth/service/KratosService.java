package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.User;
import com.fasterxml.jackson.databind.JsonNode;
import io.github.resilience4j.circuitbreaker.annotation.CircuitBreaker;
import io.github.resilience4j.retry.annotation.Retry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.http.HttpStatusCode;
import org.springframework.stereotype.Service;
import org.springframework.web.reactive.function.client.WebClient;
import reactor.core.publisher.Mono;

import java.util.HashMap;
import java.util.Map;
import java.util.Optional;

/**
 * Client for Ory Kratos identity management API.
 * <p>
 * Handles:
 * - Creating identities in Kratos when users are created in auth-ms
 * - Fetching session/identity info
 * - Triggering recovery/verification flows
 * - Managing identity state (active/inactive)
 */
@Service
public class KratosService {

    private static final Logger log = LoggerFactory.getLogger(KratosService.class);

    private final WebClient publicClient;
    private final WebClient adminClient;

    public KratosService(
            @Qualifier("kratosPublicClient") WebClient publicClient,
            @Qualifier("kratosAdminClient") WebClient adminClient) {
        this.publicClient = publicClient;
        this.adminClient = adminClient;
    }

    /**
     * Create an identity in Kratos for a new user.
     * Uses the admin API to create the identity directly.
     *
     * @param user the user entity to create in Kratos
     * @return the Kratos identity ID
     */
    @CircuitBreaker(name = "kratos", fallbackMethod = "createIdentityFallback")
    @Retry(name = "kratos")
    public Optional<String> createIdentity(User user) {
        try {
            Map<String, Object> traits = new HashMap<>();
            traits.put("email", user.getEmail());
            traits.put("name", Map.of(
                    "first", user.getFirstName(),
                    "last", user.getLastName()
            ));
            if (user.getDepartment() != null) {
                traits.put("department", user.getDepartment());
            }

            Map<String, Object> body = new HashMap<>();
            body.put("schema_id", "default");
            body.put("traits", traits);
            body.put("state", "active");

            JsonNode response = adminClient.post()
                    .uri("/admin/identities")
                    .bodyValue(body)
                    .retrieve()
                    .onStatus(HttpStatusCode::isError, resp ->
                            resp.bodyToMono(String.class)
                                    .flatMap(errBody -> Mono.error(
                                            new RuntimeException("Kratos identity creation failed: " + errBody))))
                    .bodyToMono(JsonNode.class)
                    .block();

            if (response != null && response.has("id")) {
                String kratosId = response.get("id").asText();
                log.info("Created Kratos identity: {} for user email={}", kratosId, user.getEmail());
                return Optional.of(kratosId);
            }

            return Optional.empty();
        } catch (Exception e) {
            log.error("Failed to create Kratos identity for email={}: {}", user.getEmail(), e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Fetch an identity from Kratos by its ID.
     */
    @CircuitBreaker(name = "kratos", fallbackMethod = "getIdentityFallback")
    @Retry(name = "kratos")
    public Optional<JsonNode> getIdentity(String kratosIdentityId) {
        try {
            JsonNode response = adminClient.get()
                    .uri("/admin/identities/{id}", kratosIdentityId)
                    .retrieve()
                    .onStatus(HttpStatusCode::isError, resp -> Mono.empty())
                    .bodyToMono(JsonNode.class)
                    .block();
            return Optional.ofNullable(response);
        } catch (Exception e) {
            log.error("Failed to fetch Kratos identity {}: {}", kratosIdentityId, e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Update identity traits in Kratos.
     */
    @CircuitBreaker(name = "kratos", fallbackMethod = "updateIdentityTraitsFallback")
    @Retry(name = "kratos")
    public boolean updateIdentityTraits(String kratosIdentityId, User user) {
        try {
            Map<String, Object> traits = new HashMap<>();
            traits.put("email", user.getEmail());
            traits.put("name", Map.of(
                    "first", user.getFirstName(),
                    "last", user.getLastName()
            ));
            if (user.getDepartment() != null) {
                traits.put("department", user.getDepartment());
            }

            Map<String, Object> body = new HashMap<>();
            body.put("schema_id", "default");
            body.put("traits", traits);
            body.put("state", user.isActive() ? "active" : "inactive");

            adminClient.put()
                    .uri("/admin/identities/{id}", kratosIdentityId)
                    .bodyValue(body)
                    .retrieve()
                    .onStatus(HttpStatusCode::isError, resp ->
                            resp.bodyToMono(String.class)
                                    .flatMap(errBody -> Mono.error(
                                            new RuntimeException("Kratos update failed: " + errBody))))
                    .bodyToMono(JsonNode.class)
                    .block();

            log.info("Updated Kratos identity traits for {}", kratosIdentityId);
            return true;
        } catch (Exception e) {
            log.error("Failed to update Kratos identity {}: {}", kratosIdentityId, e.getMessage());
            return false;
        }
    }

    /**
     * Deactivate an identity in Kratos (set state to inactive).
     */
    @CircuitBreaker(name = "kratos", fallbackMethod = "deactivateIdentityFallback")
    @Retry(name = "kratos")
    public boolean deactivateIdentity(String kratosIdentityId) {
        try {
            Map<String, Object> body = Map.of("state", "inactive");

            adminClient.patch()
                    .uri("/admin/identities/{id}", kratosIdentityId)
                    .bodyValue(body)
                    .retrieve()
                    .bodyToMono(JsonNode.class)
                    .block();

            log.info("Deactivated Kratos identity: {}", kratosIdentityId);
            return true;
        } catch (Exception e) {
            log.error("Failed to deactivate Kratos identity {}: {}", kratosIdentityId, e.getMessage());
            return false;
        }
    }

    /**
     * Validate a Kratos session token (from cookie or header).
     * Used for the handshake interceptor when admin users come through Kratos login.
     */
    @CircuitBreaker(name = "kratos", fallbackMethod = "validateSessionFallback")
    @Retry(name = "kratos")
    public Optional<JsonNode> validateSession(String sessionToken) {
        try {
            JsonNode response = publicClient.get()
                    .uri("/sessions/whoami")
                    .header("X-Session-Token", sessionToken)
                    .retrieve()
                    .onStatus(HttpStatusCode::isError, resp -> Mono.empty())
                    .bodyToMono(JsonNode.class)
                    .block();
            return Optional.ofNullable(response);
        } catch (Exception e) {
            log.debug("Session validation failed: {}", e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Create a recovery link for a user (admin action).
     */
    @CircuitBreaker(name = "kratos", fallbackMethod = "createRecoveryLinkFallback")
    @Retry(name = "kratos")
    public Optional<String> createRecoveryLink(String kratosIdentityId) {
        try {
            Map<String, Object> body = Map.of(
                    "identity_id", kratosIdentityId,
                    "expires_in", "24h"
            );

            JsonNode response = adminClient.post()
                    .uri("/admin/recovery/link")
                    .bodyValue(body)
                    .retrieve()
                    .bodyToMono(JsonNode.class)
                    .block();

            if (response != null && response.has("recovery_link")) {
                return Optional.of(response.get("recovery_link").asText());
            }
            return Optional.empty();
        } catch (Exception e) {
            log.error("Failed to create recovery link for {}: {}", kratosIdentityId, e.getMessage());
            return Optional.empty();
        }
    }

    // ── Circuit Breaker fallback methods ─────────────────────────────────────

    private Optional<String> createIdentityFallback(User user, Exception e) {
        log.warn("CircuitBreaker fallback: Kratos unavailable for createIdentity email={}: {}", user.getEmail(), e.getMessage());
        return Optional.empty();
    }

    private Optional<JsonNode> getIdentityFallback(String kratosIdentityId, Exception e) {
        log.warn("CircuitBreaker fallback: Kratos unavailable for getIdentity id={}: {}", kratosIdentityId, e.getMessage());
        return Optional.empty();
    }

    private boolean updateIdentityTraitsFallback(String kratosIdentityId, User user, Exception e) {
        log.warn("CircuitBreaker fallback: Kratos unavailable for updateIdentityTraits id={}: {}", kratosIdentityId, e.getMessage());
        return false;
    }

    private boolean deactivateIdentityFallback(String kratosIdentityId, Exception e) {
        log.warn("CircuitBreaker fallback: Kratos unavailable for deactivateIdentity id={}: {}", kratosIdentityId, e.getMessage());
        return false;
    }

    private Optional<JsonNode> validateSessionFallback(String sessionToken, Exception e) {
        log.warn("CircuitBreaker fallback: Kratos unavailable for validateSession: {}", e.getMessage());
        return Optional.empty();
    }

    private Optional<String> createRecoveryLinkFallback(String kratosIdentityId, Exception e) {
        log.warn("CircuitBreaker fallback: Kratos unavailable for createRecoveryLink id={}: {}", kratosIdentityId, e.getMessage());
        return Optional.empty();
    }
}
