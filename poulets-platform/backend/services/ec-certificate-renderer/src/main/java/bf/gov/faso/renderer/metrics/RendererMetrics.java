package bf.gov.faso.renderer.metrics;

import bf.gov.faso.renderer.service.PdfCacheService;
import bf.gov.faso.renderer.service.PlaywrightMultiBrowserPool;
import bf.gov.faso.renderer.util.RenderSemaphore;
import io.micrometer.core.instrument.Gauge;
import io.micrometer.core.instrument.MeterRegistry;
import jakarta.annotation.PostConstruct;
import org.springframework.stereotype.Component;

@Component
public class RendererMetrics {

    private final PlaywrightMultiBrowserPool pool;
    private final RenderSemaphore           semaphore;
    private final PdfCacheService           cacheService;
    private final MeterRegistry             registry;

    public RendererMetrics(
            PlaywrightMultiBrowserPool pool,
            RenderSemaphore semaphore,
            PdfCacheService cacheService,
            MeterRegistry registry) {
        this.pool         = pool;
        this.semaphore    = semaphore;
        this.cacheService = cacheService;
        this.registry     = registry;
    }

    @PostConstruct
    public void registerAll() {

        Gauge.builder("renderer.pool.available", pool, PlaywrightMultiBrowserPool::available)
             .description("Available Playwright pages in pool")
             .register(registry);

        Gauge.builder("renderer.pool.total", pool, PlaywrightMultiBrowserPool::total)
             .description("Total Playwright pool capacity")
             .register(registry);

        Gauge.builder("renderer.pool.in_use", pool, PlaywrightMultiBrowserPool::inUse)
             .description("Pages currently rendering")
             .register(registry);

        Gauge.builder("renderer.pool.saturation", pool, PlaywrightMultiBrowserPool::saturation)
             .description("Pool saturation (0.0=free, 1.0=full) — HPA metric")
             .register(registry);

        Gauge.builder("renderer.pool.browsers", pool, PlaywrightMultiBrowserPool::getBrowserCount)
             .description("Number of active Chromium processes")
             .register(registry);

        Gauge.builder("renderer.semaphore.active", semaphore, RenderSemaphore::getActiveTasks)
             .description("Active PDF renders")
             .register(registry);

        Gauge.builder("renderer.semaphore.available", semaphore, RenderSemaphore::getAvailableSlots)
             .description("Available render slots (semaphore)")
             .register(registry);

        Gauge.builder("renderer.cache.size", cacheService, PdfCacheService::estimatedSize)
             .description("PDF cache size")
             .register(registry);

        Gauge.builder("renderer.cache.hit_rate", cacheService,
                      s -> s.isEnabled() ? s.stats().hitRate() : 0.0)
             .description("PDF cache hit rate (0.0-1.0)")
             .register(registry);

        Gauge.builder("renderer.cache.eviction_count", cacheService,
                      s -> s.isEnabled() ? (double) s.stats().evictionCount() : 0.0)
             .description("Cache eviction count since startup")
             .register(registry);
    }
}
