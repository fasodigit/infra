package bf.gov.faso.cache.warmup;

import bf.gov.faso.cache.bloom.BloomFilterService;
import bf.gov.faso.cache.dragonfly.DragonflyDBCacheService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.context.event.ApplicationReadyEvent;
import org.springframework.context.event.EventListener;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;

/**
 * Service that warms up DragonflyDB cache and Bloom filters on application startup.
 * <p>
 * Activated only when {@code ec.cache.warmup.enabled=true}.
 * Uses {@link WarmUpDataProvider} implementations discovered in the application context
 * to load recent data into cache and Bloom filters.
 * <p>
 * Warm-up runs asynchronously after {@link ApplicationReadyEvent} to avoid
 * blocking the application startup sequence.
 */
public class CacheWarmUpService {

    private static final Logger log = LoggerFactory.getLogger(CacheWarmUpService.class);

    private final DragonflyDBCacheService cacheService;
    private final BloomFilterService bloomFilterService;
    private final WarmUpProperties properties;
    private final List<WarmUpDataProvider<?>> dataProviders;

    public CacheWarmUpService(DragonflyDBCacheService cacheService,
                              BloomFilterService bloomFilterService,
                              WarmUpProperties properties,
                              List<WarmUpDataProvider<?>> dataProviders) {
        this.cacheService = cacheService;
        this.bloomFilterService = bloomFilterService;
        this.properties = properties;
        this.dataProviders = dataProviders;
    }

    @EventListener(ApplicationReadyEvent.class)
    public void warmUp() {
        if (dataProviders.isEmpty()) {
            log.info("Cache warm-up: no WarmUpDataProvider beans found, skipping");
            return;
        }

        log.info("Starting cache warm-up for last {} days with {} provider(s)",
                properties.getDays(), dataProviders.size());

        Instant to = Instant.now();
        Instant from = to.minus(properties.getDays(), ChronoUnit.DAYS);

        for (WarmUpDataProvider<?> provider : dataProviders) {
            warmUpProvider(provider, from, to);
        }

        log.info("Cache warm-up completed");
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private void warmUpProvider(WarmUpDataProvider provider, Instant from, Instant to) {
        String bloomFilter = provider.bloomFilterName();
        int batchSize = properties.getBatchSize();
        int offset = 0;
        long totalLoaded = 0;

        try {
            while (true) {
                List<?> batch = provider.loadBatch(from, to, offset, batchSize);
                if (batch.isEmpty()) break;

                for (Object entity : batch) {
                    String cacheKey = provider.cacheKey(entity);
                    cacheService.put(cacheKey, entity, properties.getTtl());
                    bloomFilterService.add(bloomFilter, provider.bloomFilterKey(entity));
                }

                totalLoaded += batch.size();
                offset += batchSize;

                if (batch.size() < batchSize) break;
            }

            log.info("Warmed up {} entries for bloom filter '{}'", totalLoaded, bloomFilter);
        } catch (Exception e) {
            log.warn("Cache warm-up failed for provider [bloom={}] after {} entries: {}",
                    bloomFilter, totalLoaded, e.getMessage());
        }
    }
}
