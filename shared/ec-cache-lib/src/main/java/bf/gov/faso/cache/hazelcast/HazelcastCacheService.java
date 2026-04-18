package bf.gov.faso.cache.hazelcast;

import com.hazelcast.core.HazelcastInstance;
import com.hazelcast.map.IMap;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;

import java.util.Optional;
import java.util.concurrent.TimeUnit;

/**
 * L2 near-cache service backed by Hazelcast.
 * <p>
 * Provides ~0.1ms local reads (near-cache) vs ~1-5ms remote reads.
 * Ideal for frequently-read, rarely-written data (act types, configs).
 * <p>
 * Conditionally created only when Hazelcast is on the classpath.
 * If absent, services should fall back to DragonflyDB cache.
 */
public class HazelcastCacheService {

    private static final Logger log = LoggerFactory.getLogger(HazelcastCacheService.class);

    @Autowired(required = false)
    private HazelcastInstance hazelcastInstance;

    /**
     * Get a value from the Hazelcast distributed map (near-cache enabled).
     *
     * @param mapName the distributed map name
     * @param key     the entry key
     * @param type    the expected value type
     * @return the value, or empty if absent or Hazelcast unavailable
     */
    @SuppressWarnings("unchecked")
    public <T> Optional<T> get(String mapName, String key, Class<T> type) {
        if (hazelcastInstance == null) return Optional.empty();
        try {
            IMap<String, Object> map = hazelcastInstance.getMap(mapName);
            Object value = map.get(key);
            if (value == null) return Optional.empty();
            return Optional.of(type.cast(value));
        } catch (Exception e) {
            log.warn("Hazelcast get failed [map={}, key={}]: {}", mapName, key, e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Put a value in the distributed map with TTL.
     *
     * @param mapName    the distributed map name
     * @param key        the entry key
     * @param value      the value to store
     * @param ttlSeconds TTL in seconds (0 = no expiry)
     */
    public <T> void put(String mapName, String key, T value, long ttlSeconds) {
        if (hazelcastInstance == null) return;
        try {
            IMap<String, Object> map = hazelcastInstance.getMap(mapName);
            if (ttlSeconds > 0) {
                map.put(key, value, ttlSeconds, TimeUnit.SECONDS);
            } else {
                map.put(key, value);
            }
        } catch (Exception e) {
            log.warn("Hazelcast put failed [map={}, key={}]: {}", mapName, key, e.getMessage());
        }
    }

    /**
     * Evict a single entry from the distributed map.
     */
    public void evict(String mapName, String key) {
        if (hazelcastInstance == null) return;
        try {
            IMap<String, Object> map = hazelcastInstance.getMap(mapName);
            map.evict(key);
        } catch (Exception e) {
            log.warn("Hazelcast evict failed [map={}, key={}]: {}", mapName, key, e.getMessage());
        }
    }

    /**
     * Check if Hazelcast is available.
     */
    public boolean isAvailable() {
        return hazelcastInstance != null;
    }
}
