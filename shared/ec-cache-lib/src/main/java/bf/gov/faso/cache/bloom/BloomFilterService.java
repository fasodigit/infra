package bf.gov.faso.cache.bloom;

import bf.gov.faso.cache.CacheProperties;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

/**
 * Bloom Filter service backed by DragonflyDB BF.* commands.
 * <p>
 * Bloom filters provide O(1) probabilistic membership testing:
 * <ul>
 *     <li>{@code mightContain() == false} → item is DEFINITELY absent</li>
 *     <li>{@code mightContain() == true}  → item is PROBABLY present (false positive possible)</li>
 * </ul>
 * <p>
 * Use pattern: test Bloom filter first, then query DB only if {@code mightContain} returns true.
 */
public class BloomFilterService {

    private static final Logger log = LoggerFactory.getLogger(BloomFilterService.class);

    private final StringRedisTemplate redis;
    private final CacheProperties properties;

    public BloomFilterService(StringRedisTemplate redis, CacheProperties properties) {
        this.redis = redis;
        this.properties = properties;
    }

    /**
     * Create a Bloom filter with explicit error rate and capacity.
     *
     * @param key      filter key (auto-prefixed with {@code ec.cache.key-prefix})
     * @param errorRate false-positive probability (e.g. 0.01 for 1%)
     * @param capacity expected number of elements
     */
    public void createFilter(String key, double errorRate, long capacity) {
        String prefixedKey = prefixKey(key);
        try {
            redis.execute(connection -> {
                connection.execute("BF.RESERVE", prefixedKey.getBytes(),
                        String.valueOf(errorRate).getBytes(),
                        String.valueOf(capacity).getBytes());
                return null;
            }, true);
            log.debug("Created Bloom filter [key={}, errorRate={}, capacity={}]", prefixedKey, errorRate, capacity);
        } catch (Exception e) {
            log.warn("Failed to create Bloom filter [key={}]: {}", prefixedKey, e.getMessage());
        }
    }

    /**
     * Create a Bloom filter with default error rate and capacity from properties.
     */
    public void createFilter(String key) {
        createFilter(key,
                properties.getBloom().getDefaultErrorRate(),
                properties.getBloom().getDefaultCapacity());
    }

    /**
     * Add an item to the Bloom filter.
     *
     * @return true if the item was newly added, false if it was already present (or error)
     */
    public boolean add(String key, String item) {
        String prefixedKey = prefixKey(key);
        try {
            Object result = redis.execute(connection ->
                    connection.execute("BF.ADD", prefixedKey.getBytes(), item.getBytes()), true);
            return result != null && (Long) result == 1L;
        } catch (Exception e) {
            log.warn("Bloom filter add failed [key={}, item={}]: {}", prefixedKey, item, e.getMessage());
            return false;
        }
    }

    /**
     * Test if an item might be in the filter.
     * <p>
     * <b>IMPORTANT</b>: {@code false} means the item is DEFINITELY NOT in the set.
     * {@code true} means the item MIGHT be in the set (false positive possible).
     */
    public boolean mightContain(String key, String item) {
        String prefixedKey = prefixKey(key);
        try {
            Object result = redis.execute(connection ->
                    connection.execute("BF.EXISTS", prefixedKey.getBytes(), item.getBytes()), true);
            return result != null && (Long) result == 1L;
        } catch (Exception e) {
            log.warn("Bloom filter exists check failed [key={}, item={}]: {}", prefixedKey, item, e.getMessage());
            // On error, assume item might exist to avoid false negatives
            return true;
        }
    }

    /**
     * Add multiple items to the Bloom filter in a single call.
     */
    public void addAll(String key, Collection<String> items) {
        if (items.isEmpty()) return;
        String prefixedKey = prefixKey(key);
        try {
            List<byte[]> args = new ArrayList<>();
            args.add(prefixedKey.getBytes());
            items.forEach(item -> args.add(item.getBytes()));
            redis.execute(connection -> {
                connection.execute("BF.MADD", args.toArray(new byte[0][]));
                return null;
            }, true);
        } catch (Exception e) {
            log.warn("Bloom filter addAll failed [key={}, count={}]: {}", prefixedKey, items.size(), e.getMessage());
        }
    }

    /**
     * Convenience method for the Bloom → DB pattern.
     * <p>
     * Returns {@code true} if the item <em>might</em> exist in the database
     * (meaning you should query the DB). Returns {@code false} if the item
     * is DEFINITELY NOT in the database (skip the query).
     *
     * @param filterKey Bloom filter key
     * @param itemId    the item identifier to check
     * @return true if DB query is warranted, false if definitely absent
     */
    public boolean shouldQueryDatabase(String filterKey, String itemId) {
        return mightContain(filterKey, itemId);
    }

    private String prefixKey(String key) {
        String prefix = properties.getKeyPrefix();
        if (key.startsWith(prefix)) return key;
        return prefix + key;
    }
}
