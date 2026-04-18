package bf.gov.faso.cache.stream;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.connection.stream.RecordId;
import org.springframework.data.redis.core.StringRedisTemplate;

import java.time.Duration;
import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * High-level service for managing workflow event streams in DragonflyDB.
 * <p>
 * Each demande gets its own stream: {@code ec:workflow:{tenantId}:{demandeId}}.
 * Entries are immutable (append-only) and contain only workflow-essential fields:
 * step, status, operator reference, timestamps — no PII, no form data.
 * <p>
 * Streams are trimmed via XTRIM MINID to enforce time-based retention (default 7 days).
 * <p>
 * Consumer groups enable downstream services (traitement, validation, impression)
 * to process workflow events independently without Kafka overhead for internal state.
 *
 * @see WorkflowStepEntry
 * @see StreamOutboxService
 */
public class WorkflowStreamService {

    private static final Logger log = LoggerFactory.getLogger(WorkflowStreamService.class);

    private static final String STREAM_KEY_PREFIX = "ec:workflow:";

    private final StreamOutboxService streamOutboxService;
    private final StreamConsumerProperties properties;
    private final StringRedisTemplate redisTemplate;

    public WorkflowStreamService(StreamOutboxService streamOutboxService,
                                 StreamConsumerProperties properties,
                                 StringRedisTemplate redisTemplate) {
        this.streamOutboxService = streamOutboxService;
        this.properties = properties;
        this.redisTemplate = redisTemplate;
    }

    // ── Publishing ──────────────────────────────────────────────────

    /**
     * Publishes a workflow step to the demande's stream.
     *
     * @param tenantId  the tenant identifier
     * @param demandeId the demande UUID (as string)
     * @param entry     the workflow step entry
     * @return the DragonflyDB record ID, or null on failure
     */
    public RecordId publishStep(String tenantId, String demandeId, WorkflowStepEntry entry) {
        String streamKey = buildStreamKey(tenantId, demandeId);
        try {
            RecordId id = streamOutboxService.publish(streamKey, entry.toMap());
            log.debug("Published workflow step [stream={}, step={}, operator={}]",
                    streamKey, entry.step(), entry.operatorId());
            return id;
        } catch (Exception e) {
            log.warn("Failed to publish workflow step [stream={}, step={}]: {}",
                    streamKey, entry.step(), e.getMessage());
            return null;
        }
    }

    // ── Reading ─────────────────────────────────────────────────────

    /**
     * Reads the full workflow history for a demande (XRANGE - +).
     *
     * @param tenantId  the tenant identifier
     * @param demandeId the demande UUID (as string)
     * @return ordered list of workflow steps (oldest first), or empty list
     */
    public List<WorkflowStepEntry> readHistory(String tenantId, String demandeId) {
        String streamKey = buildStreamKey(tenantId, demandeId);
        try {
            var records = streamOutboxService.range(streamKey, "-", "+");
            return records.stream()
                    .map(r -> WorkflowStepEntry.fromMap(r.getValue()))
                    .toList();
        } catch (Exception e) {
            log.warn("Failed to read workflow history [stream={}]: {}", streamKey, e.getMessage());
            return List.of();
        }
    }

    /**
     * Reads the latest workflow step for a demande.
     *
     * @param tenantId  the tenant identifier
     * @param demandeId the demande UUID (as string)
     * @return the latest step, or null if stream is empty
     */
    public WorkflowStepEntry readLatest(String tenantId, String demandeId) {
        String streamKey = buildStreamKey(tenantId, demandeId);
        try {
            // XREVRANGE with count 1 — read last entry
            var range = org.springframework.data.domain.Range.closed("-", "+");
            var raw = redisTemplate.opsForStream()
                    .reverseRange(streamKey, range, org.springframework.data.redis.connection.Limit.limit().count(1));
            if (raw != null && !raw.isEmpty()) {
                var record = raw.getFirst();
                Map<String, String> stringMap = new java.util.LinkedHashMap<>();
                record.getValue().forEach((k, v) ->
                        stringMap.put(String.valueOf(k), v != null ? String.valueOf(v) : ""));
                return WorkflowStepEntry.fromMap(stringMap);
            }
            return null;
        } catch (Exception e) {
            log.warn("Failed to read latest workflow step [stream={}]: {}", streamKey, e.getMessage());
            return null;
        }
    }

    /**
     * Returns the number of workflow steps recorded for a demande.
     *
     * @param tenantId  the tenant identifier
     * @param demandeId the demande UUID (as string)
     * @return step count, or 0 on error
     */
    public long stepCount(String tenantId, String demandeId) {
        return streamOutboxService.streamLength(buildStreamKey(tenantId, demandeId));
    }

    // ── Consumer Groups ─────────────────────────────────────────────

    /**
     * Creates a consumer group for a service on a specific demande stream.
     * Idempotent — safe to call on every service startup.
     *
     * @param tenantId    the tenant identifier
     * @param demandeId   the demande UUID (as string)
     * @param groupName   the consumer group name (e.g. "traitement-cg")
     */
    public void ensureConsumerGroup(String tenantId, String demandeId, String groupName) {
        streamOutboxService.createConsumerGroup(buildStreamKey(tenantId, demandeId), groupName);
    }

    /**
     * Reads unacknowledged messages for a consumer group.
     *
     * @param tenantId     the tenant identifier
     * @param demandeId    the demande UUID (as string)
     * @param groupName    the consumer group name
     * @param consumerName the consumer instance name
     * @param count        max records to read
     * @return list of workflow step entries
     */
    public List<WorkflowStepEntry> readGroup(String tenantId, String demandeId,
                                              String groupName, String consumerName, int count) {
        String streamKey = buildStreamKey(tenantId, demandeId);
        var records = streamOutboxService.readGroup(streamKey, groupName, consumerName, count);
        return records.stream()
                .map(r -> WorkflowStepEntry.fromMap(r.getValue()))
                .toList();
    }

    /**
     * Acknowledges processed records in a consumer group.
     */
    public void acknowledge(String tenantId, String demandeId, String groupName, RecordId... ids) {
        streamOutboxService.acknowledge(buildStreamKey(tenantId, demandeId), groupName, ids);
    }

    // ── Trimming / Retention ────────────────────────────────────────

    /**
     * Trims all workflow streams older than the configured retention period.
     * <p>
     * Scans for keys matching {@code ec:workflow:*} and applies XTRIM MINID
     * to remove entries older than {@code workflowRetentionDays}.
     * <p>
     * Designed to be called by a scheduled task (e.g. every 6 hours).
     *
     * @return number of streams trimmed
     */
    public int trimExpiredStreams() {
        int retentionDays = properties.getWorkflowRetentionDays();
        long minTimestamp = Instant.now().minus(Duration.ofDays(retentionDays)).toEpochMilli();
        String minId = minTimestamp + "-0";

        int trimmed = 0;
        try {
            Set<String> keys = redisTemplate.keys(STREAM_KEY_PREFIX + "*");
            if (keys == null || keys.isEmpty()) {
                log.debug("No workflow streams found for trimming");
                return 0;
            }

            for (String key : keys) {
                try {
                    streamOutboxService.trimByMinId(key, minId);
                    trimmed++;
                } catch (Exception e) {
                    log.warn("Failed to trim stream {}: {}", key, e.getMessage());
                }
            }

            log.info("Trimmed {} workflow streams (retention={}d, minId={})", trimmed, retentionDays, minId);
        } catch (Exception e) {
            log.warn("Failed to scan workflow streams for trimming: {}", e.getMessage());
        }
        return trimmed;
    }

    /**
     * Deletes the workflow stream for a demande.
     * Use only for cleanup of fully completed/archived demandes.
     *
     * @param tenantId  the tenant identifier
     * @param demandeId the demande UUID (as string)
     */
    public void deleteStream(String tenantId, String demandeId) {
        String streamKey = buildStreamKey(tenantId, demandeId);
        try {
            redisTemplate.delete(streamKey);
            log.debug("Deleted workflow stream: {}", streamKey);
        } catch (Exception e) {
            log.warn("Failed to delete workflow stream {}: {}", streamKey, e.getMessage());
        }
    }

    // ── Internal ────────────────────────────────────────────────────

    /**
     * Builds the DragonflyDB stream key for a demande.
     * Pattern: {@code ec:workflow:{tenantId}:{demandeId}}
     */
    public static String buildStreamKey(String tenantId, String demandeId) {
        return STREAM_KEY_PREFIX + tenantId + ":" + demandeId;
    }
}
