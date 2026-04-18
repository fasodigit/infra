package bf.gov.faso.cache.dragonfly;

import bf.gov.faso.cache.CacheProperties;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.ZSetOperations;

import java.time.Duration;
import java.util.*;
import java.util.stream.Collectors;

/**
 * Typed cache service backed by DragonflyDB (Redis-compatible).
 * <p>
 * All keys are automatically prefixed with {@code ec.cache.key-prefix}.
 * <p>
 * Provides:
 * <ul>
 *     <li><b>String operations</b>: typed get/put/delete with JSON serialization</li>
 *     <li><b>Sorted sets</b>: leaderboards, ranked queries via ZADD/ZRANGE</li>
 *     <li><b>Counters</b>: atomic increment/decrement with TTL (KPI, rate limiting)</li>
 *     <li><b>Pattern delete</b>: wildcard key deletion for cache invalidation</li>
 * </ul>
 * <p>
 * All operations are non-blocking on failure (log + return empty/default).
 */
public class DragonflyDBCacheService {

    private static final Logger log = LoggerFactory.getLogger(DragonflyDBCacheService.class);

    private final StringRedisTemplate redis;
    private final ObjectMapper objectMapper;
    private final CacheProperties properties;

    public DragonflyDBCacheService(StringRedisTemplate redis,
                                    ObjectMapper objectMapper,
                                    CacheProperties properties) {
        this.redis = redis;
        this.objectMapper = objectMapper;
        this.properties = properties;
    }

    // ── String operations (typed JSON) ──────────────────────────────

    /**
     * Get a cached value, deserialized to the given type.
     *
     * @return Optional.empty() on cache miss or deserialization failure
     */
    public <T> Optional<T> get(String key, Class<T> type) {
        String prefixedKey = prefixKey(key);
        try {
            String json = redis.opsForValue().get(prefixedKey);
            if (json == null) return Optional.empty();
            return Optional.of(objectMapper.readValue(json, type));
        } catch (JsonProcessingException e) {
            log.warn("Cache deserialization failed [key={}]: {}", prefixedKey, e.getMessage());
            return Optional.empty();
        } catch (Exception e) {
            log.warn("Cache get failed [key={}]: {}", prefixedKey, e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Put a value in cache with explicit TTL.
     */
    public <T> void put(String key, T value, Duration ttl) {
        String prefixedKey = prefixKey(key);
        try {
            String json = objectMapper.writeValueAsString(value);
            redis.opsForValue().set(prefixedKey, json, ttl);
        } catch (JsonProcessingException e) {
            log.warn("Cache serialization failed [key={}]: {}", prefixedKey, e.getMessage());
        } catch (Exception e) {
            log.warn("Cache put failed [key={}]: {}", prefixedKey, e.getMessage());
        }
    }

    /**
     * Put a value in cache with the default TTL from properties.
     */
    public <T> void put(String key, T value) {
        put(key, value, properties.getDefaultTtl());
    }

    /**
     * Delete a single cache key.
     */
    public void delete(String key) {
        try {
            redis.delete(prefixKey(key));
        } catch (Exception e) {
            log.warn("Cache delete failed [key={}]: {}", key, e.getMessage());
        }
    }

    /**
     * Delete all keys matching a pattern (e.g. "demande:*").
     * Uses SCAN internally to avoid blocking.
     */
    public void deletePattern(String pattern) {
        String prefixedPattern = prefixKey(pattern);
        try {
            Set<String> keys = redis.keys(prefixedPattern);
            if (keys != null && !keys.isEmpty()) {
                redis.delete(keys);
                log.debug("Deleted {} keys matching pattern [{}]", keys.size(), prefixedPattern);
            }
        } catch (Exception e) {
            log.warn("Cache deletePattern failed [pattern={}]: {}", prefixedPattern, e.getMessage());
        }
    }

    /**
     * Check if a key exists in cache.
     */
    public boolean exists(String key) {
        try {
            return Boolean.TRUE.equals(redis.hasKey(prefixKey(key)));
        } catch (Exception e) {
            log.warn("Cache exists check failed [key={}]: {}", key, e.getMessage());
            return false;
        }
    }

    // ── Counters (atomic) ───────────────────────────────────────────

    /**
     * Atomically increment a counter and set TTL.
     * Useful for real-time KPI counters (e.g. daily demande count).
     *
     * @return the new value after increment
     */
    public Long increment(String key, Duration ttl) {
        String prefixedKey = prefixKey(key);
        try {
            Long result = redis.opsForValue().increment(prefixedKey);
            redis.expire(prefixedKey, ttl);
            return result;
        } catch (Exception e) {
            log.warn("Cache increment failed [key={}]: {}", prefixedKey, e.getMessage());
            return 0L;
        }
    }

    /**
     * Atomically decrement a counter.
     *
     * @return the new value after decrement
     */
    public Long decrement(String key) {
        String prefixedKey = prefixKey(key);
        try {
            return redis.opsForValue().decrement(prefixedKey);
        } catch (Exception e) {
            log.warn("Cache decrement failed [key={}]: {}", prefixedKey, e.getMessage());
            return 0L;
        }
    }

    // ── Sorted sets (leaderboards, rankings) ────────────────────────

    /**
     * Add a member to a sorted set with a score.
     * Useful for leaderboards, priority queues, time-ordered indexes.
     */
    public void zAdd(String key, String member, double score, Duration ttl) {
        String prefixedKey = prefixKey(key);
        try {
            redis.opsForZSet().add(prefixedKey, member, score);
            redis.expire(prefixedKey, ttl);
        } catch (Exception e) {
            log.warn("Cache zAdd failed [key={}, member={}]: {}", prefixedKey, member, e.getMessage());
        }
    }

    /**
     * Get top N members from a sorted set (highest scores first).
     * Returns a list of maps with "member" and "score" keys.
     */
    public List<Map<String, Object>> zTopN(String key, int n) {
        String prefixedKey = prefixKey(key);
        try {
            Set<ZSetOperations.TypedTuple<String>> tuples =
                    redis.opsForZSet().reverseRangeWithScores(prefixedKey, 0, (long) n - 1);
            if (tuples == null) return List.of();
            return tuples.stream()
                    .map(t -> {
                        Map<String, Object> entry = new LinkedHashMap<>();
                        entry.put("member", t.getValue());
                        entry.put("score", t.getScore());
                        return entry;
                    })
                    .collect(Collectors.toList());
        } catch (Exception e) {
            log.warn("Cache zTopN failed [key={}, n={}]: {}", prefixedKey, n, e.getMessage());
            return List.of();
        }
    }

    /**
     * Get members in a score range (ascending).
     */
    public Set<String> zRangeByScore(String key, double min, double max, int limit) {
        String prefixedKey = prefixKey(key);
        try {
            Set<String> result = redis.opsForZSet().rangeByScore(prefixedKey, min, max, 0, limit);
            return result != null ? result : Set.of();
        } catch (Exception e) {
            log.warn("Cache zRangeByScore failed [key={}]: {}", prefixedKey, e.getMessage());
            return Set.of();
        }
    }

    /**
     * Remove a member from a sorted set.
     */
    public void zRemove(String key, String member) {
        String prefixedKey = prefixKey(key);
        try {
            redis.opsForZSet().remove(prefixedKey, member);
        } catch (Exception e) {
            log.warn("Cache zRemove failed [key={}, member={}]: {}", prefixedKey, member, e.getMessage());
        }
    }

    /**
     * Get the rank of a member in a sorted set (0-based, ascending).
     */
    public Long zRank(String key, String member) {
        String prefixedKey = prefixKey(key);
        try {
            return redis.opsForZSet().rank(prefixedKey, member);
        } catch (Exception e) {
            log.warn("Cache zRank failed [key={}, member={}]: {}", prefixedKey, member, e.getMessage());
            return null;
        }
    }

    // ── Set operations ──────────────────────────────────────────────

    /**
     * Add members to a set.
     */
    public void sAdd(String key, String... members) {
        String prefixedKey = prefixKey(key);
        try {
            redis.opsForSet().add(prefixedKey, members);
        } catch (Exception e) {
            log.warn("Cache sAdd failed [key={}]: {}", prefixedKey, e.getMessage());
        }
    }

    /**
     * Check if a member is in a set.
     */
    public boolean sIsMember(String key, String member) {
        String prefixedKey = prefixKey(key);
        try {
            return Boolean.TRUE.equals(redis.opsForSet().isMember(prefixedKey, member));
        } catch (Exception e) {
            log.warn("Cache sIsMember failed [key={}]: {}", prefixedKey, e.getMessage());
            return false;
        }
    }

    /**
     * Get all members of a set.
     */
    public Set<String> sMembers(String key) {
        String prefixedKey = prefixKey(key);
        try {
            Set<String> result = redis.opsForSet().members(prefixedKey);
            return result != null ? result : Set.of();
        } catch (Exception e) {
            log.warn("Cache sMembers failed [key={}]: {}", prefixedKey, e.getMessage());
            return Set.of();
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    private String prefixKey(String key) {
        String prefix = properties.getKeyPrefix();
        if (key.startsWith(prefix)) return key;
        return prefix + key;
    }
}
