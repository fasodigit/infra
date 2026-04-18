package bf.gov.faso.renderer.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

import java.util.List;

/**
 * Propriétés externalisées — préfixe {@code renderer}.
 *
 * <p>Toutes les valeurs sont surchargeable par variables d'environnement :
 * {@code RENDERER_BROWSER_COUNT}, {@code RENDERER_PAGES_PER_BROWSER}, etc.
 */
@ConfigurationProperties(prefix = "renderer")
public record RendererProperties(

        int browserCount,
        int pagesPerBrowser,
        long pageAcquireTimeoutMs,
        int maxConcurrent,
        boolean cacheEnabled,
        long cacheTtlSeconds,
        long cacheMaxSize,
        String chromiumPath,
        List<String> chromiumArgs,
        String hmacSecret,
        long hmacTimestampDriftMs,
        int rateLimitWindowSeconds,
        int rateLimitMaxRequests

) {

    /** Constructeur compact — applique les valeurs par défaut. */
    public RendererProperties {
        if (browserCount < 0)           browserCount = 0;
        if (pagesPerBrowser <= 0)       pagesPerBrowser = 3;
        if (pageAcquireTimeoutMs <= 0)  pageAcquireTimeoutMs = 8_000L;
        if (maxConcurrent < 0)          maxConcurrent = 0;
        if (cacheTtlSeconds <= 0)       cacheTtlSeconds = 3_600L;
        if (cacheMaxSize <= 0)          cacheMaxSize = 500L;
        if (chromiumPath == null)       chromiumPath = "";
        if (chromiumArgs == null)       chromiumArgs = List.of();
        if (hmacSecret == null)         hmacSecret = "dev-renderer-secret";
        if (hmacTimestampDriftMs <= 0)  hmacTimestampDriftMs = 30_000L;
        if (rateLimitWindowSeconds <= 0) rateLimitWindowSeconds = 60;
        if (rateLimitMaxRequests <= 0)   rateLimitMaxRequests = 200;
    }

    public int effectiveBrowserCount() {
        if (browserCount > 0) return browserCount;
        return Math.max(2, Runtime.getRuntime().availableProcessors() / 2);
    }

    public int effectiveMaxConcurrent(int effectiveBrowserCount) {
        if (maxConcurrent > 0) return maxConcurrent;
        return effectiveBrowserCount * pagesPerBrowser;
    }
}
