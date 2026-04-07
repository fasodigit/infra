package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.User;
import com.fasterxml.jackson.databind.JsonNode;
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
}
