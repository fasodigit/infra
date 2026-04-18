package bf.gov.faso.cache.warmup;

import java.time.Instant;
import java.util.List;

/**
 * SPI interface for providing data to the cache warm-up process.
 * <p>
 * Each service implements this interface for entity types that should be
 * pre-loaded into DragonflyDB and Bloom filters on application startup.
 * <p>
 * Example implementation for demande entities:
 * <pre>
 * {@literal @}Component
 * public class DemandeWarmUpProvider implements WarmUpDataProvider&lt;Demande&gt; {
 *
 *     {@literal @}Override
 *     public List&lt;Demande&gt; loadBatch(Instant from, Instant to, int offset, int limit) {
 *         return demandeRepository.findByCreatedAtBetween(from, to, PageRequest.of(offset / limit, limit));
 *     }
 *
 *     {@literal @}Override
 *     public String cacheKey(Demande entity) {
 *         return "demande:" + entity.getId();
 *     }
 *
 *     {@literal @}Override
 *     public String bloomFilterName() {
 *         return "bloom:demande";
 *     }
 *
 *     {@literal @}Override
 *     public String bloomFilterKey(Demande entity) {
 *         return entity.getId().toString();
 *     }
 * }
 * </pre>
 *
 * @param <T> the entity type to warm up
 */
public interface WarmUpDataProvider<T> {

    /**
     * Loads a batch of entities from the database within the given time range.
     *
     * @param from   start of the time range (inclusive)
     * @param to     end of the time range (exclusive)
     * @param offset the offset within the result set
     * @param limit  maximum number of results to return
     * @return list of entities, empty list if no more data
     */
    List<T> loadBatch(Instant from, Instant to, int offset, int limit);

    /**
     * Returns the cache key for the given entity.
     * The key will be used with {@link bf.gov.faso.cache.dragonfly.DragonflyDBCacheService}.
     *
     * @param entity the entity to derive a cache key for
     * @return the cache key (without prefix — prefix is applied automatically)
     */
    String cacheKey(T entity);

    /**
     * Returns the Bloom filter name for this entity type.
     *
     * @return the Bloom filter key (without prefix)
     */
    String bloomFilterName();

    /**
     * Returns the Bloom filter key for the given entity.
     *
     * @param entity the entity to derive a Bloom filter key for
     * @return the key to add to the Bloom filter
     */
    String bloomFilterKey(T entity);
}
