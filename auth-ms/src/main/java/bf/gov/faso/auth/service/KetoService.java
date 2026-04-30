package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.Permission;
import bf.gov.faso.auth.model.Role;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
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

import java.util.*;

/**
 * Synchronization service for Ory Keto (Zanzibar-based authorization).
 * <p>
 * Responsible for writing relation tuples to Keto when:
 * - A role is assigned/revoked from a user
 * - A user's department changes
 * - A full sync is triggered (administrative action)
 * <p>
 * Keto relation tuple format:
 *   namespace:object#relation@subject_id
 * Example:
 *   auth:users#admin@user-uuid
 *   departments:finance#member@user-uuid
 */
@Service
public class KetoService {

    private static final Logger log = LoggerFactory.getLogger(KetoService.class);

    private final WebClient readClient;
    private final WebClient writeClient;
    private final UserRepository userRepository;

    public KetoService(
            @Qualifier("ketoReadClient") WebClient readClient,
            @Qualifier("ketoWriteClient") WebClient writeClient,
            UserRepository userRepository) {
        this.readClient = readClient;
        this.writeClient = writeClient;
        this.userRepository = userRepository;
    }

    /**
     * Write a single relation tuple to Keto.
     */
    @CircuitBreaker(name = "keto", fallbackMethod = "writeRelationTupleFallback")
    @Retry(name = "default")
    public boolean writeRelationTuple(String namespace, String object, String relation, String subjectId) {
        try {
            Map<String, Object> tuple = buildTuplePayload(namespace, object, relation, subjectId);

            writeClient.put()
                    .uri("/admin/relation-tuples")
                    .bodyValue(tuple)
                    .retrieve()
                    .onStatus(HttpStatusCode::isError, resp ->
                            resp.bodyToMono(String.class)
                                    .flatMap(errBody -> Mono.error(
                                            new RuntimeException("Keto write failed: " + errBody))))
                    .bodyToMono(Void.class)
                    .block();

            log.info("Wrote relation tuple: {}:{}#{}@{}", namespace, object, relation, subjectId);
            return true;
        } catch (Exception e) {
            log.error("Failed to write relation tuple {}:{}#{}@{}: {}",
                    namespace, object, relation, subjectId, e.getMessage());
            return false;
        }
    }

    /**
     * Delete a relation tuple from Keto.
     */
    @CircuitBreaker(name = "keto", fallbackMethod = "deleteRelationTupleFallback")
    @Retry(name = "default")
    public boolean deleteRelationTuple(String namespace, String object, String relation, String subjectId) {
        try {
            writeClient.delete()
                    .uri(uriBuilder -> uriBuilder
                            .path("/admin/relation-tuples")
                            .queryParam("namespace", namespace)
                            .queryParam("object", object)
                            .queryParam("relation", relation)
                            .queryParam("subject_id", subjectId)
                            .build())
                    .retrieve()
                    .onStatus(HttpStatusCode::isError, resp -> Mono.empty())
                    .bodyToMono(Void.class)
                    .block();

            log.info("Deleted relation tuple: {}:{}#{}@{}", namespace, object, relation, subjectId);
            return true;
        } catch (Exception e) {
            log.error("Failed to delete relation tuple: {}", e.getMessage());
            return false;
        }
    }

    /**
     * Sync all role assignments for a user to Keto.
     */
    public int syncUserRoles(User user) {
        int synced = 0;
        String userId = user.getId().toString();

        for (Role role : user.getRoles()) {
            // Write the role assignment tuple
            if (writeRelationTuple("auth", "roles", role.getName().toLowerCase(), userId)) {
                synced++;
            }

            // Also write individual permission tuples
            for (Permission perm : role.getPermissions()) {
                if (writeRelationTuple(perm.getNamespace(), perm.getObject(), perm.getRelation(), userId)) {
                    synced++;
                }
            }
        }

        // Sync department membership if set
        if (user.getDepartment() != null && !user.getDepartment().isBlank()) {
            if (writeRelationTuple("departments", user.getDepartment().toLowerCase(), "member", userId)) {
                synced++;
            }
        }

        log.info("Synced {} relation tuples for userId={}", synced, userId);
        return synced;
    }

    /**
     * Full sync: re-sync all users' roles and departments to Keto.
     */
    public int fullSync() {
        List<User> allUsers = userRepository.findAll();
        int totalSynced = 0;

        for (User user : allUsers) {
            if (user.isActive() && !user.isSuspended()) {
                totalSynced += syncUserRoles(user);
            }
        }

        log.info("Full Keto sync completed: {} tuples for {} users", totalSynced, allUsers.size());
        return totalSynced;
    }

    /**
     * Partial sync for specific user IDs.
     */
    public int syncUsers(List<String> userIds) {
        int totalSynced = 0;
        for (String id : userIds) {
            Optional<User> user = userRepository.findById(UUID.fromString(id));
            if (user.isPresent() && user.get().isActive()) {
                totalSynced += syncUserRoles(user.get());
            }
        }
        return totalSynced;
    }

    /**
     * Check a permission in Keto (read API).
     */
    @CircuitBreaker(name = "keto", fallbackMethod = "checkPermissionFallback")
    @Retry(name = "default")
    public boolean checkPermission(String namespace, String object, String relation, String subjectId) {
        try {
            JsonNode response = readClient.get()
                    .uri(uriBuilder -> uriBuilder
                            .path("/relation-tuples/check")
                            .queryParam("namespace", namespace)
                            .queryParam("object", object)
                            .queryParam("relation", relation)
                            .queryParam("subject_id", subjectId)
                            .build())
                    .retrieve()
                    .bodyToMono(JsonNode.class)
                    .block();

            if (response != null && response.has("allowed")) {
                return response.get("allowed").asBoolean(false);
            }
            return false;
        } catch (Exception e) {
            log.error("Keto permission check failed: {}", e.getMessage());
            return false;
        }
    }

    /**
     * Query all relation tuples for a subject (user).
     */
    public List<Map<String, String>> getUserRelationTuples(String userId, String namespace) {
        try {
            var uriSpec = readClient.get()
                    .uri(uriBuilder -> {
                        var b = uriBuilder.path("/relation-tuples")
                                .queryParam("subject_id", userId);
                        if (namespace != null && !namespace.isBlank()) {
                            b.queryParam("namespace", namespace);
                        }
                        return b.build();
                    });

            JsonNode response = uriSpec.retrieve()
                    .bodyToMono(JsonNode.class)
                    .block();

            List<Map<String, String>> tuples = new ArrayList<>();
            if (response != null && response.has("relation_tuples")) {
                for (JsonNode node : response.get("relation_tuples")) {
                    Map<String, String> tuple = new HashMap<>();
                    tuple.put("namespace", node.path("namespace").asText());
                    tuple.put("object", node.path("object").asText());
                    tuple.put("relation", node.path("relation").asText());
                    tuple.put("subject_id", node.path("subject_id").asText());
                    tuples.add(tuple);
                }
            }
            return tuples;
        } catch (Exception e) {
            log.error("Failed to query Keto tuples for userId={}: {}", userId, e.getMessage());
            return List.of();
        }
    }

    private Map<String, Object> buildTuplePayload(String namespace, String object, String relation, String subjectId) {
        Map<String, Object> tuple = new LinkedHashMap<>();
        tuple.put("namespace", namespace);
        tuple.put("object", object);
        tuple.put("relation", relation);
        tuple.put("subject_id", subjectId);
        return tuple;
    }

    // ── Circuit Breaker fallback methods ─────────────────────────────────────

    private boolean writeRelationTupleFallback(String namespace, String object, String relation, String subjectId, Exception e) {
        log.warn("CircuitBreaker fallback: Keto unavailable for writeRelationTuple {}:{}#{}@{}: {}",
                namespace, object, relation, subjectId, e.getMessage());
        return false;
    }

    private boolean deleteRelationTupleFallback(String namespace, String object, String relation, String subjectId, Exception e) {
        log.warn("CircuitBreaker fallback: Keto unavailable for deleteRelationTuple {}:{}#{}@{}: {}",
                namespace, object, relation, subjectId, e.getMessage());
        return false;
    }

    private boolean checkPermissionFallback(String namespace, String object, String relation, String subjectId, Exception e) {
        log.warn("CircuitBreaker fallback: Keto unavailable for checkPermission {}:{}#{}@{}: {}",
                namespace, object, relation, subjectId, e.getMessage());
        // Deny by default when authorization service is unavailable
        return false;
    }
}
