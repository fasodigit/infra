package bf.gov.faso.cache;

import org.springframework.boot.context.properties.ConfigurationProperties;

import java.time.Duration;

/**
 * Configuration properties for ETAT-CIVIL cache infrastructure.
 * <p>
 * Prefix: {@code ec.cache}
 * <p>
 * Example YAML:
 * <pre>
 * ec:
 *   cache:
 *     key-prefix: "ec:demande:"
 *     default-ttl: 30m
 *     bloom:
 *       default-error-rate: 0.01
 *       default-capacity: 100000
 *     stream:
 *       enabled: true
 *       block-timeout-ms: 2000
 *       batch-size: 100
 *       max-stream-length: 10000
 *       consumer-group: ec-consumers
 *     warmup:
 *       enabled: true
 *       days: 30
 *       batch-size: 1000
 *       ttl: 30d
 * </pre>
 */
@ConfigurationProperties(prefix = "ec.cache")
public class CacheProperties {

    /** Key prefix for all DragonflyDB keys (e.g. "ec:demande:"). */
    private String keyPrefix = "ec:";

    /** Default TTL for cache entries. */
    private Duration defaultTtl = Duration.ofMinutes(30);

    /** Bloom filter configuration. */
    private BloomProperties bloom = new BloomProperties();

    /** Stream outbox configuration. */
    private StreamProperties stream = new StreamProperties();

    /** Cache warm-up configuration. */
    private WarmupProperties warmup = new WarmupProperties();

    public String getKeyPrefix() { return keyPrefix; }
    public void setKeyPrefix(String keyPrefix) { this.keyPrefix = keyPrefix; }

    public Duration getDefaultTtl() { return defaultTtl; }
    public void setDefaultTtl(Duration defaultTtl) { this.defaultTtl = defaultTtl; }

    public BloomProperties getBloom() { return bloom; }
    public void setBloom(BloomProperties bloom) { this.bloom = bloom; }

    public StreamProperties getStream() { return stream; }
    public void setStream(StreamProperties stream) { this.stream = stream; }

    public WarmupProperties getWarmup() { return warmup; }
    public void setWarmup(WarmupProperties warmup) { this.warmup = warmup; }

    public static class BloomProperties {

        /** Default false-positive error rate for Bloom filters. */
        private double defaultErrorRate = 0.01;

        /** Default capacity (expected number of elements). */
        private long defaultCapacity = 100_000;

        public double getDefaultErrorRate() { return defaultErrorRate; }
        public void setDefaultErrorRate(double defaultErrorRate) { this.defaultErrorRate = defaultErrorRate; }

        public long getDefaultCapacity() { return defaultCapacity; }
        public void setDefaultCapacity(long defaultCapacity) { this.defaultCapacity = defaultCapacity; }
    }

    public static class StreamProperties {

        /** Whether stream infrastructure is enabled. */
        private boolean enabled = false;

        /** Block timeout in milliseconds for XREADGROUP. */
        private long blockTimeoutMs = 2000;

        /** Maximum number of records to read per XREADGROUP call. */
        private int batchSize = 100;

        /** Maximum stream length before XTRIM. */
        private long maxStreamLength = 10_000;

        /** Default consumer group name. */
        private String consumerGroup = "ec-consumers";

        public boolean isEnabled() { return enabled; }
        public void setEnabled(boolean enabled) { this.enabled = enabled; }

        public long getBlockTimeoutMs() { return blockTimeoutMs; }
        public void setBlockTimeoutMs(long blockTimeoutMs) { this.blockTimeoutMs = blockTimeoutMs; }

        public int getBatchSize() { return batchSize; }
        public void setBatchSize(int batchSize) { this.batchSize = batchSize; }

        public long getMaxStreamLength() { return maxStreamLength; }
        public void setMaxStreamLength(long maxStreamLength) { this.maxStreamLength = maxStreamLength; }

        public String getConsumerGroup() { return consumerGroup; }
        public void setConsumerGroup(String consumerGroup) { this.consumerGroup = consumerGroup; }
    }

    public static class WarmupProperties {

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
}
