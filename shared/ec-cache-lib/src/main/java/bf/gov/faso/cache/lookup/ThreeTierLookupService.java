package bf.gov.faso.cache.lookup;

import bf.gov.faso.cache.bloom.BloomFilterService;
import bf.gov.faso.cache.dragonfly.DragonflyDBCacheService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Duration;
import java.util.Optional;
import java.util.function.Supplier;

/**
 * Three-tier lookup service implementing the Bloom -> Cache -> DB pattern.
 * <p>
 * Lookup flow:
 * <ol>
 *     <li><b>Bloom Filter</b> (O(1)): {@code mightContain()} -> false = DEFINITELY NOT present -> empty</li>
 *     <li><b>DragonflyDB</b> (sub-ms): cache hit -> return immediately</li>
 *     <li><b>Database</b> (B+Tree on UUIDv7): query, populate cache, return</li>
 * </ol>
 * <p>
 * This eliminates unnecessary DB queries for non-existent keys and leverages
 * DragonflyDB's sub-millisecond latency for hot data.
 */
public class ThreeTierLookupService {

    private static final Logger log = LoggerFactory.getLogger(ThreeTierLookupService.class);

    private static final Duration DEFAULT_CACHE_TTL = Duration.ofHours(1);

    private final BloomFilterService bloomFilterService;
    private final DragonflyDBCacheService cacheService;

    public ThreeTierLookupService(BloomFilterService bloomFilterService,
                                   DragonflyDBCacheService cacheService) {
        this.bloomFilterService = bloomFilterService;
        this.cacheService = cacheService;
    }

    /**
     * Performs a three-tier lookup: Bloom filter -> DragonflyDB cache -> database fallback.
     *
     * @param bloomFilterName the Bloom filter key to check
     * @param key             the cache key and Bloom filter item key
     * @param type            the expected return type
     * @param dbFallback      supplier that queries the database if cache misses
     * @param <T>             the entity type
     * @return the entity if found, empty if definitely absent
     */
    public <T> Optional<T> lookup(String bloomFilterName, String key, Class<T> type,
                                   Supplier<Optional<T>> dbFallback) {
        return lookup(bloomFilterName, key, type, dbFallback, DEFAULT_CACHE_TTL);
    }

    /**
     * Performs a three-tier lookup with explicit cache TTL.
     *
     * @param bloomFilterName the Bloom filter key to check
     * @param key             the cache key and Bloom filter item key
     * @param type            the expected return type
     * @param dbFallback      supplier that queries the database if cache misses
     * @param cacheTtl        TTL for the cache entry when populated from DB
     * @param <T>             the entity type
     * @return the entity if found, empty if definitely absent
     */
    public <T> Optional<T> lookup(String bloomFilterName, String key, Class<T> type,
                                   Supplier<Optional<T>> dbFallback, Duration cacheTtl) {
        // 1. Bloom filter check — false means DEFINITELY NOT present
        if (!bloomFilterService.mightContain(bloomFilterName, key)) {
            log.debug("Bloom filter negative for key={} in filter={}", key, bloomFilterName);
            return Optional.empty();
        }

        // 2. DragonflyDB cache lookup — sub-ms latency
        Optional<T> cached = cacheService.get(key, type);
        if (cached.isPresent()) {
            log.debug("Cache hit for key={}", key);
            return cached;
        }

        // 3. Database fallback — B+Tree on UUIDv7 primary key
        Optional<T> result = dbFallback.get();
        result.ifPresent(value -> {
            cacheService.put(key, value, cacheTtl);
            log.debug("Cache populated from DB for key={}", key);
        });

        return result;
    }

    /**
     * Performs a two-tier lookup without Bloom filter: Cache -> DB.
     * <p>
     * Use this when the entity is known to exist (e.g. after creation)
     * or when no Bloom filter is configured for this entity type.
     *
     * @param key        the cache key
     * @param type       the expected return type
     * @param dbFallback supplier that queries the database if cache misses
     * @param <T>        the entity type
     * @return the entity if found, empty otherwise
     */
    public <T> Optional<T> lookupWithoutBloom(String key, Class<T> type,
                                               Supplier<Optional<T>> dbFallback) {
        return lookupWithoutBloom(key, type, dbFallback, DEFAULT_CACHE_TTL);
    }

    /**
     * Performs a two-tier lookup without Bloom filter with explicit cache TTL.
     *
     * @param key        the cache key
     * @param type       the expected return type
     * @param dbFallback supplier that queries the database if cache misses
     * @param cacheTtl   TTL for the cache entry when populated from DB
     * @param <T>        the entity type
     * @return the entity if found, empty otherwise
     */
    public <T> Optional<T> lookupWithoutBloom(String key, Class<T> type,
                                               Supplier<Optional<T>> dbFallback,
                                               Duration cacheTtl) {
        Optional<T> cached = cacheService.get(key, type);
        if (cached.isPresent()) {
            return cached;
        }

        Optional<T> result = dbFallback.get();
        result.ifPresent(value -> cacheService.put(key, value, cacheTtl));
        return result;
    }
}
