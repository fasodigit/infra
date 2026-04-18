package bf.gov.faso.cache.stream;

import org.springframework.boot.context.properties.ConfigurationProperties;

/**
 * Configuration properties for DragonflyDB Stream consumer infrastructure.
 * <p>
 * Prefix: {@code ec.cache.stream}
 * <p>
 * Example YAML:
 * <pre>
 * ec:
 *   cache:
 *     stream:
 *       enabled: true
 *       block-timeout-ms: 2000
 *       batch-size: 100
 *       max-stream-length: 10000
 *       consumer-group: ec-consumers
 * </pre>
 */
@ConfigurationProperties(prefix = "ec.cache.stream")
public class StreamConsumerProperties {

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

    /** Retention period in days for workflow streams (XTRIM MINID). */
    private int workflowRetentionDays = 7;

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

    public int getWorkflowRetentionDays() { return workflowRetentionDays; }
    public void setWorkflowRetentionDays(int workflowRetentionDays) { this.workflowRetentionDays = workflowRetentionDays; }
}
