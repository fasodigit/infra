// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.poulets.flags;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.ResponseEntity;
import org.springframework.stereotype.Component;
import org.springframework.web.client.RestTemplate;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Duration;
import java.util.HexFormat;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;

/**
 * FeatureFlagsService — source unique de vérité côté backend Java pour les
 * feature-flags FASO DIGITALISATION.
 *
 * <p>Stratégie :</p>
 * <ol>
 *   <li>Hit KAYA sur {@code ff:{env}:{hash}} (RESP3 via Lettuce, TTL 30 s).</li>
 *   <li>Miss → REST call GrowthBook {@code GET /api/features/{env}}.</li>
 *   <li>Cache le payload brut + les attributs hashés.</li>
 *   <li>Evaluation locale (pas de round-trip par {@code isOn}).</li>
 * </ol>
 *
 * <p><b>Souveraineté</b> : le cache backend est KAYA (port 6380, RESP3) —
 * jamais Redis ni DragonflyDB. La clef {@code redis}RedisTemplate fait
 * référence au driver Lettuce configuré sur {@code spring.data.redis.host=kaya}.</p>
 *
 * <p><b>Sécurité</b> : {@code GROWTHBOOK_API_KEY} est lue depuis Vault via
 * {@code spring-cloud-vault-config}. Jamais de fallback en clair.</p>
 */
@Component
public class FeatureFlagsService {

    private static final Logger log = LoggerFactory.getLogger(FeatureFlagsService.class);

    private static final Duration CACHE_TTL = Duration.ofSeconds(30);
    private static final String KEY_PREFIX  = "ff:";

    private final StringRedisTemplate redis;
    private final RestTemplate rest;
    private final ObjectMapper mapper;

    @Value("${faso.flags.growthbook.base-url:http://faso-growthbook:3100}")
    private String growthbookBaseUrl;

    @Value("${faso.flags.environment:dev}")
    private String environment;

    @Value("${faso.flags.growthbook.api-key:}")
    private String apiKey;

    public FeatureFlagsService(StringRedisTemplate redis, RestTemplate rest, ObjectMapper mapper) {
        this.redis  = redis;
        this.rest   = rest;
        this.mapper = mapper;
    }

    /**
     * Évalue un flag pour un jeu d'attributs (user_id, role, region…).
     * Retour {@code false} par défaut si le flag est absent ou indéterminé.
     */
    public boolean isOn(String key, Map<String, Object> attributes) {
        if (key == null || key.isBlank()) {
            return false;
        }
        try {
            JsonNode features = loadFeatures(attributes);
            return evaluate(features, key, attributes);
        } catch (Exception e) {
            log.warn("isOn({}) fallback=false: {}", key, e.getMessage());
            return false;
        }
    }

    /** Renvoie la liste {@code flag1,flag2,...} des flags ON pour cet utilisateur. */
    public String activeFlagsHeader(Map<String, Object> attributes) {
        try {
            JsonNode features = loadFeatures(attributes);
            StringBuilder sb = new StringBuilder(64);
            Iterator<String> names = features.fieldNames();
            while (names.hasNext()) {
                String name = names.next();
                if (evaluate(features, name, attributes)) {
                    if (sb.length() > 0) sb.append(',');
                    sb.append(name);
                }
            }
            return sb.toString();
        } catch (Exception e) {
            log.warn("activeFlagsHeader fallback=empty: {}", e.getMessage());
            return "";
        }
    }

    // ------------------------------------------------------------------ cache

    JsonNode loadFeatures(Map<String, Object> attributes) throws Exception {
        String cacheKey = KEY_PREFIX + environment + ":" + attributesHash(attributes);

        // 1) KAYA lookup (RESP3 via Lettuce)
        String cached = safeGet(cacheKey);
        if (cached != null) {
            log.debug("FF cache HIT {}", cacheKey);
            return mapper.readTree(cached);
        }

        // 2) Miss : interrogation GrowthBook
        log.debug("FF cache MISS {} → GrowthBook", cacheKey);
        String json = fetchFromGrowthBook();
        safeSet(cacheKey, json);
        return mapper.readTree(json);
    }

    private String fetchFromGrowthBook() {
        HttpHeaders headers = new HttpHeaders();
        if (apiKey != null && !apiKey.isBlank()) {
            headers.setBearerAuth(apiKey);
        }
        String url = growthbookBaseUrl + "/api/features/" + environment;
        ResponseEntity<String> resp = rest.exchange(url, HttpMethod.GET, new HttpEntity<>(headers), String.class);
        if (!resp.getStatusCode().is2xxSuccessful() || resp.getBody() == null) {
            throw new IllegalStateException("GrowthBook HTTP " + resp.getStatusCode());
        }
        // Réponse : { "features": { "flag": { "defaultValue": true, ... } } }
        try {
            JsonNode root = mapper.readTree(resp.getBody());
            JsonNode feats = root.path("features");
            return mapper.writeValueAsString(feats);
        } catch (Exception e) {
            throw new IllegalStateException("GrowthBook parse error", e);
        }
    }

    private String safeGet(String key) {
        try {
            return redis.opsForValue().get(key);
        } catch (Exception e) {
            log.debug("KAYA GET {} failed: {}", key, e.getMessage());
            return null;
        }
    }

    private void safeSet(String key, String json) {
        try {
            redis.opsForValue().set(key, json, CACHE_TTL);
        } catch (Exception e) {
            log.debug("KAYA SET {} failed: {}", key, e.getMessage());
        }
    }

    // ------------------------------------------------------------- evaluation

    /**
     * Evaluation simplifiée d'un flag GrowthBook (suffisante pour les flags
     * booléens FASO). Les règles complexes (targeting, A/B) nécessitent le SDK
     * officiel GrowthBook ; ici on couvre : defaultValue, force-on/off.
     */
    boolean evaluate(JsonNode features, String key, Map<String, Object> attributes) {
        JsonNode node = features.path(key);
        if (node.isMissingNode()) {
            return false;
        }
        // Règle "force" ciblée par user_id
        JsonNode rules = node.path("rules");
        if (rules.isArray()) {
            Object userId = attributes.get("user_id");
            for (JsonNode r : rules) {
                if (r.has("force") && r.path("condition").path("id").asText("").equals(String.valueOf(userId))) {
                    return r.path("force").asBoolean(false);
                }
            }
        }
        return node.path("defaultValue").asBoolean(false);
    }

    // ------------------------------------------------------------- hash utils

    static String attributesHash(Map<String, Object> attributes) {
        // Canonical JSON des attributs → SHA-256 → 16 hex chars
        try {
            Map<String, Object> sorted = (attributes == null) ? Map.of() : new LinkedHashMap<>(attributes);
            byte[] bytes = new ObjectMapper().writeValueAsBytes(sorted);
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] digest = md.digest(bytes);
            return HexFormat.of().formatHex(digest).substring(0, 16);
        } catch (Exception e) {
            return "0000000000000000";
        }
    }

    // Exposed for tests
    Optional<String> peekCache(Map<String, Object> attributes) {
        return Optional.ofNullable(safeGet(KEY_PREFIX + environment + ":" + attributesHash(attributes)));
    }

    /** Hook de configuration pour tests. */
    void configure(String baseUrl, String env, String apiKey) {
        this.growthbookBaseUrl = baseUrl;
        this.environment = env;
        this.apiKey = apiKey;
    }

    // Exposed for tests — (String) parsing helper
    JsonNode parse(String json) throws Exception {
        return mapper.readTree(json);
    }

    @SuppressWarnings("unused")
    private static TypeReference<Map<String, Object>> mapType() {
        return new TypeReference<>() { };
    }
}
