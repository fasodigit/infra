package bf.gov.faso.poulets.controller;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.dao.DataAccessException;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.*;

/**
 * Public (no-auth) REST controller serving dashboard data from KAYA.
 * <p>
 * Reads annonces, besoins, aliments and stats from the poulets:* namespace
 * in KAYA (Redis-compatible on port 6380).
 * <p>
 * Phone numbers are masked for public consumption (only last 4 digits shown).
 */
@RestController
@RequestMapping("/api/public/dashboard")
public class PublicDashboardController {

    private static final Logger log = LoggerFactory.getLogger(PublicDashboardController.class);

    private static final TypeReference<Map<String, Object>> MAP_TYPE = new TypeReference<>() {};

    @Autowired
    private StringRedisTemplate redisTemplate;

    @Autowired
    private ObjectMapper objectMapper;

    // -------------------------------------------------------------------------
    // GET /api/public/dashboard/annonces
    // -------------------------------------------------------------------------
    @GetMapping("/annonces")
    public ResponseEntity<List<Map<String, Object>>> getAnnonces(
            @RequestParam(defaultValue = "10") int limit,
            @RequestParam(required = false) String region) {

        List<Map<String, Object>> results = fetchFromSortedSet(
                "poulets:annonces:index", "poulets:annonces:", limit, region, "phone");
        return ResponseEntity.ok(results);
    }

    // -------------------------------------------------------------------------
    // GET /api/public/dashboard/besoins
    // -------------------------------------------------------------------------
    @GetMapping("/besoins")
    public ResponseEntity<List<Map<String, Object>>> getBesoins(
            @RequestParam(defaultValue = "10") int limit,
            @RequestParam(required = false) String region) {

        List<Map<String, Object>> results = fetchFromSortedSet(
                "poulets:besoins:index", "poulets:besoins:", limit, region, null);
        return ResponseEntity.ok(results);
    }

    // -------------------------------------------------------------------------
    // GET /api/public/dashboard/aliments
    // -------------------------------------------------------------------------
    @GetMapping("/aliments")
    public ResponseEntity<List<Map<String, Object>>> getAliments(
            @RequestParam(defaultValue = "10") int limit,
            @RequestParam(required = false) String region) {

        List<Map<String, Object>> results = fetchFromSortedSet(
                "poulets:aliments:index", "poulets:aliments:", limit, region, "phone");
        return ResponseEntity.ok(results);
    }

    // -------------------------------------------------------------------------
    // GET /api/public/dashboard/poussins
    // -------------------------------------------------------------------------
    @GetMapping("/poussins")
    public ResponseEntity<List<Map<String, Object>>> getPoussins(
            @RequestParam(defaultValue = "20") int limit,
            @RequestParam(required = false) String region) {

        List<Map<String, Object>> results = fetchFromSortedSet(
                "poulets:poussins:index", "poulets:poussins:", limit, region, null);
        return ResponseEntity.ok(results);
    }

    // -------------------------------------------------------------------------
    // GET /api/public/dashboard/stats
    // -------------------------------------------------------------------------
    @GetMapping("/stats")
    public ResponseEntity<Map<String, Object>> getStats() {
        Map<String, Object> stats = new LinkedHashMap<>();

        String[] keys = {
            "poulets:stats:total_eleveurs",
            "poulets:stats:total_clients",
            "poulets:stats:total_transactions",
            "poulets:stats:total_regions",
            "poulets:stats:live_users",
            "poulets:stats:matchings_actifs"
        };

        for (String key : keys) {
            // Strip the namespace prefix for cleaner response keys
            String shortKey = key.substring("poulets:stats:".length());
            // TODO: retirer ce try/catch quand KAYA RESP3 complet — ticket INFRA/kaya#resp3-encoder
            // Graceful fallback: en dev, KAYA retourne "ERR protocol parse error"
            // sur certaines commandes (bug inbound frame parser). On dégrade à 0.
            String value;
            try {
                value = redisTemplate.opsForValue().get(key);
            } catch (DataAccessException e) {
                log.debug("KAYA GET {} failed: {}", key, e.getMessage());
                value = null;
            }
            if (value != null) {
                try {
                    stats.put(shortKey, Long.parseLong(value));
                } catch (NumberFormatException e) {
                    stats.put(shortKey, value);
                }
            } else {
                stats.put(shortKey, 0);
            }
        }

        return ResponseEntity.ok(stats);
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    /**
     * Fetches entries from a sorted set index (newest first), resolves each ID
     * to its JSON payload, optionally filters by region, masks phone fields,
     * and returns up to {@code limit} entries.
     */
    private List<Map<String, Object>> fetchFromSortedSet(
            String indexKey, String dataPrefix, int limit,
            String regionFilter, String phoneField) {

        // Fetch more than needed in case region filter removes some
        int fetchSize = (regionFilter != null) ? limit * 5 : limit;
        fetchSize = Math.max(fetchSize, 50);

        // ZREVRANGE: newest first (highest score = most recent timestamp)
        // TODO: retirer ce try/catch quand KAYA RESP3 complet — ticket INFRA/kaya#resp3-encoder
        // Graceful fallback: KAYA peut renvoyer "ERR protocol parse error" sur
        // ZREVRANGE en dev (bug inbound frame parser). On dégrade à liste vide.
        Set<String> ids;
        try {
            ids = redisTemplate.opsForZSet().reverseRange(indexKey, 0, fetchSize - 1);
        } catch (DataAccessException e) {
            log.debug("KAYA ZREVRANGE {} failed: {}", indexKey, e.getMessage());
            return Collections.emptyList();
        }
        if (ids == null || ids.isEmpty()) {
            return Collections.emptyList();
        }

        List<Map<String, Object>> results = new ArrayList<>();

        for (String id : ids) {
            if (results.size() >= limit) break;

            // TODO: retirer ce try/catch quand KAYA RESP3 complet — ticket INFRA/kaya#resp3-encoder
            String json;
            try {
                json = redisTemplate.opsForValue().get(dataPrefix + id);
            } catch (DataAccessException e) {
                log.debug("KAYA GET {}{} failed: {}", dataPrefix, id, e.getMessage());
                continue;
            }
            if (json == null) continue;

            try {
                Map<String, Object> entry = objectMapper.readValue(json, MAP_TYPE);

                // Region filter
                if (regionFilter != null && !regionFilter.isBlank()) {
                    Object entryRegion = entry.get("region");
                    if (entryRegion == null || !regionFilter.equalsIgnoreCase(entryRegion.toString())) {
                        continue;
                    }
                }

                // Mask phone number for public display
                if (phoneField != null) {
                    maskPhone(entry, phoneField);
                }

                results.add(entry);
            } catch (Exception e) {
                // Skip malformed entries
            }
        }

        return results;
    }

    /**
     * Masks a phone field to show only the last 4 digits.
     * Example: "+22670112233" becomes "****2233"
     */
    private void maskPhone(Map<String, Object> entry, String field) {
        Object phone = entry.get(field);
        if (phone instanceof String phoneStr && phoneStr.length() >= 4) {
            entry.put(field, "****" + phoneStr.substring(phoneStr.length() - 4));
        }
    }
}
