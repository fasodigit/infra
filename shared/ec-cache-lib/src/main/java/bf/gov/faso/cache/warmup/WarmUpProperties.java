package bf.gov.faso.cache.warmup;

import org.springframework.boot.context.properties.ConfigurationProperties;

import java.time.Duration;

/**
 * Configuration properties for cache warm-up on application startup.
 * <p>
 * Prefix: {@code ec.cache.warmup}
 * <p>
 * Example YAML:
 * <pre>
 * ec:
 *   cache:
 *     warmup:
 *       enabled: true
 *       days: 30
 *       batch-size: 1000
 *       ttl: 30d
 * </pre>
 */
@ConfigurationProperties(prefix = "ec.cache.warmup")
public class WarmUpProperties {

    /** Whether cache warm-up is enabled on startup. */
    private boolean enabled = false;

    /** Number of days of recent data to warm up. */
    private int days = 30;

    /** Batch size for loading data from the database. */
    private int batchSize = 1000;

    /** TTL for warmed-up cache entries. */
    private Duration ttl = Duration.ofDays(30);

    public boolean isEnabled() { return enabled; }
    public void setEnabled(boolean enabled) { this.enabled = enabled; }

    public int getDays() { return days; }
    public void setDays(int days) { this.days = days; }

    public int getBatchSize() { return batchSize; }
    public void setBatchSize(int batchSize) { this.batchSize = batchSize; }

    public Duration getTtl() { return ttl; }
    public void setTtl(Duration ttl) { this.ttl = ttl; }
}
