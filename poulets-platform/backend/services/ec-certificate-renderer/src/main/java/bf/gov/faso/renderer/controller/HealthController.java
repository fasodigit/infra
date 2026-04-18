package bf.gov.faso.renderer.controller;

import bf.gov.faso.renderer.service.AssetInliner;
import bf.gov.faso.renderer.service.PdfCacheService;
import bf.gov.faso.renderer.service.PlaywrightMultiBrowserPool;
import bf.gov.faso.renderer.service.TemplateService;
import bf.gov.faso.renderer.util.RenderSemaphore;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;
import reactor.core.publisher.Mono;

import java.lang.management.ManagementFactory;
import java.util.LinkedHashMap;
import java.util.Map;

@RestController
public class HealthController {

    private static final long START_TIME_MS = System.currentTimeMillis();

    private final PlaywrightMultiBrowserPool browserPool;
    private final TemplateService            templateService;
    private final RenderSemaphore            semaphore;
    private final PdfCacheService            cacheService;
    private final AssetInliner               assetInliner;

    public HealthController(
            PlaywrightMultiBrowserPool browserPool,
            TemplateService templateService,
            RenderSemaphore semaphore,
            PdfCacheService cacheService,
            AssetInliner assetInliner) {
        this.browserPool     = browserPool;
        this.templateService = templateService;
        this.semaphore       = semaphore;
        this.cacheService    = cacheService;
        this.assetInliner    = assetInliner;
    }

    @GetMapping("/health")
    public Mono<ResponseEntity<Map<String, Object>>> health() {
        boolean browserUp = browserPool.isHealthy();

        Map<String, Object> body = new LinkedHashMap<>();
        body.put("status",    browserUp ? "UP" : "DEGRADED");
        body.put("pool",      poolInfo());
        body.put("render",    renderInfo());
        body.put("cache",     cacheInfo());
        body.put("templates", templateService.availableTemplates());
        body.put("assets",    assetsInfo());
        body.put("uptime",    (System.currentTimeMillis() - START_TIME_MS) / 1000);
        body.put("jvm",       jvmInfo());

        return Mono.just(browserUp
                ? ResponseEntity.ok(body)
                : ResponseEntity.status(503).body(body));
    }

    private Map<String, Object> poolInfo() {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("browsers",       browserPool.getBrowserCount());
        m.put("pagesPerBrowser",browserPool.getPagesPerBrowser());
        m.put("total",          browserPool.total());
        m.put("available",      browserPool.available());
        m.put("inUse",          browserPool.inUse());
        m.put("saturation",     "%.1f%%".formatted(browserPool.saturation() * 100));
        m.put("healthy",        browserPool.isHealthy());
        return m;
    }

    private Map<String, Object> renderInfo() {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("activeTasks",    semaphore.getActiveTasks());
        m.put("maxConcurrent",  semaphore.getMaxConcurrent());
        m.put("availableSlots", semaphore.getAvailableSlots());
        return m;
    }

    private Map<String, Object> cacheInfo() {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("enabled", cacheService.isEnabled());
        if (cacheService.isEnabled()) {
            m.put("size",    cacheService.estimatedSize());
            m.put("hitRate", "%.1f%%".formatted(cacheService.stats().hitRate() * 100));
            m.put("evictions", cacheService.stats().evictionCount());
        }
        return m;
    }

    private Map<String, Object> assetsInfo() {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("common", assetInliner.getCommonAssets().size());
        m.put("total",  assetInliner.totalAssetCount());
        return m;
    }

    private static Map<String, Object> jvmInfo() {
        Runtime rt = Runtime.getRuntime();
        long heapUsedMb  = (rt.totalMemory() - rt.freeMemory()) / (1024 * 1024);
        long heapMaxMb   = rt.maxMemory() / (1024 * 1024);
        String gcName = ManagementFactory.getGarbageCollectorMXBeans()
                .stream()
                .map(java.lang.management.GarbageCollectorMXBean::getName)
                .reduce((a, b) -> a + ", " + b)
                .orElse("unknown");

        Map<String, Object> m = new LinkedHashMap<>();
        m.put("version",        Runtime.version().toString());
        m.put("virtualThreads", true);
        m.put("gc",             gcName);
        m.put("heapUsedMb",     heapUsedMb);
        m.put("heapMaxMb",      heapMaxMb);
        m.put("heapUsagePct",   "%.1f%%".formatted((double) heapUsedMb / heapMaxMb * 100));
        m.put("processors",     rt.availableProcessors());
        return m;
    }
}
