package bf.gov.faso.poulets.cache;

import bf.gov.faso.poulets.model.Poulet;
import bf.gov.faso.poulets.repository.PouletRepository;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;

import java.time.Duration;
import java.time.Instant;
import java.util.*;

/**
 * Write-Behind cache pattern for Poulet entities using KAYA (Redis-compatible).
 * <p>
 * Key patterns:
 * - poulets:poulet:data:{id}     -- cached poulet JSON (TTL 21 days)
 * - poulets:poulet:wb:pending    -- Set of dirty IDs pending flush
 * - poulets:persist:poulet       -- Stream for audit trail
 * <p>
 * On writes:
 * 1. Update the cache entry immediately
 * 2. Add the ID to the pending set
 * 3. A scheduled flush persists dirty entries and writes to the audit stream
 */
@Service
public class PouletCacheService {

    private static final Logger log = LoggerFactory.getLogger(PouletCacheService.class);

    private static final String DATA_KEY_PREFIX = "poulets:poulet:data:";
    private static final String PENDING_SET_KEY = "poulets:poulet:wb:pending";
    private static final String AUDIT_STREAM_KEY = "poulets:persist:poulet";

    private final StringRedisTemplate redisTemplate;
    private final ObjectMapper objectMapper;
    private final PouletRepository pouletRepository;

    @Value("${cache.poulet.ttl-days:21}")
    private int ttlDays;

    public PouletCacheService(StringRedisTemplate redisTemplate,
                              ObjectMapper objectMapper,
                              PouletRepository pouletRepository) {
        this.redisTemplate = redisTemplate;
        this.objectMapper = objectMapper;
        this.pouletRepository = pouletRepository;
    }

    /**
     * Get a cached poulet by ID.
     * Returns empty if not in cache.
     */
    public Optional<Poulet> getCached(UUID id) {
        try {
            String key = DATA_KEY_PREFIX + id;
            String json = redisTemplate.opsForValue().get(key);
            if (json == null) {
                return Optional.empty();
            }
            Poulet poulet = objectMapper.readValue(json, Poulet.class);
            log.debug("Cache HIT for poulet id={}", id);
            return Optional.of(poulet);
        } catch (Exception e) {
            log.warn("Cache read error for poulet id={}: {}", id, e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Cache a poulet loaded from DB (read-through).
     * Does NOT mark as dirty since data already matches the DB.
     */
    public void cacheFromDb(Poulet poulet) {
        try {
            String key = DATA_KEY_PREFIX + poulet.getId();
            String json = serializePoulet(poulet);
            redisTemplate.opsForValue().set(key, json, Duration.ofDays(ttlDays));
            log.debug("Cached poulet from DB: id={}", poulet.getId());
        } catch (Exception e) {
            log.warn("Cache write error for poulet id={}: {}", poulet.getId(), e.getMessage());
        }
    }

    /**
     * Cache a poulet and mark it as dirty (write-behind).
     * Used when the poulet is created or updated -- the data is in both
     * the DB (via JPA transaction) and cache, but we track the write
     * for audit trail purposes.
     */
    public void cacheAndMarkDirty(Poulet poulet) {
        try {
            String id = poulet.getId().toString();
            String key = DATA_KEY_PREFIX + id;
            String json = serializePoulet(poulet);

            redisTemplate.opsForValue().set(key, json, Duration.ofDays(ttlDays));
            redisTemplate.opsForSet().add(PENDING_SET_KEY, id);

            log.debug("Cached and marked dirty: poulet id={}", id);
        } catch (Exception e) {
            log.warn("Cache write-behind error for poulet id={}: {}", poulet.getId(), e.getMessage());
        }
    }

    /**
     * Invalidate a poulet from the cache.
     */
    public void invalidate(UUID id) {
        String key = DATA_KEY_PREFIX + id;
        redisTemplate.delete(key);
        redisTemplate.opsForSet().remove(PENDING_SET_KEY, id.toString());
        log.debug("Invalidated cache for poulet id={}", id);
    }

    /**
     * Scheduled flush of pending (dirty) poulet entries.
     * Writes audit trail entries to the KAYA stream.
     * Runs every 30 seconds.
     */
    @Scheduled(fixedDelayString = "${cache.poulet.wb-flush-interval-seconds:30}000")
    public void flushPendingWrites() {
        Set<String> pendingIds = redisTemplate.opsForSet().members(PENDING_SET_KEY);
        if (pendingIds == null || pendingIds.isEmpty()) {
            return;
        }

        log.debug("Write-behind flush: {} pending poulet entries", pendingIds.size());

        for (String idStr : pendingIds) {
            try {
                // Write audit trail entry to KAYA stream
                Map<String, String> auditEntry = new LinkedHashMap<>();
                auditEntry.put("pouletId", idStr);
                auditEntry.put("action", "PERSIST");
                auditEntry.put("timestamp", Instant.now().toString());

                String dataKey = DATA_KEY_PREFIX + idStr;
                String cachedJson = redisTemplate.opsForValue().get(dataKey);
                if (cachedJson != null) {
                    auditEntry.put("snapshot", cachedJson);
                }

                redisTemplate.opsForStream().add(AUDIT_STREAM_KEY, auditEntry);

                // Remove from pending set
                redisTemplate.opsForSet().remove(PENDING_SET_KEY, idStr);

                log.debug("Flushed write-behind for poulet id={}", idStr);
            } catch (Exception e) {
                log.error("Write-behind flush error for poulet id={}: {}", idStr, e.getMessage());
            }
        }
    }

    private String serializePoulet(Poulet poulet) throws JsonProcessingException {
        // Create a lightweight representation for caching to avoid circular refs
        Map<String, Object> data = new LinkedHashMap<>();
        data.put("id", poulet.getId().toString());
        if (poulet.getEleveurId() != null) {
            data.put("eleveurId", poulet.getEleveurId().toString());
        } else if (poulet.getEleveur() != null) {
            data.put("eleveurId", poulet.getEleveur().getId().toString());
        }
        data.put("race", poulet.getRace().name());
        data.put("weight", poulet.getWeight());
        data.put("price", poulet.getPrice());
        data.put("quantity", poulet.getQuantity());
        data.put("description", poulet.getDescription());
        data.put("available", poulet.isAvailable());
        if (poulet.getCategorieId() != null) {
            data.put("categorieId", poulet.getCategorieId().toString());
        }
        data.put("createdAt", poulet.getCreatedAt() != null ? poulet.getCreatedAt().toString() : null);
        data.put("updatedAt", poulet.getUpdatedAt() != null ? poulet.getUpdatedAt().toString() : null);
        return objectMapper.writeValueAsString(data);
    }
}
