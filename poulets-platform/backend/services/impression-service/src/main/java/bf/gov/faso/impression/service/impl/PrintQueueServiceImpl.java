package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.service.PrintQueueService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.ZSetOperations;
import org.springframework.stereotype.Service;

import java.util.List;
import java.util.Set;
import java.util.UUID;
import java.util.stream.Collectors;

/**
 * Implementation of print queue service using DragonflyDB (Redis-compatible).
 *
 * Uses sorted sets for priority-based ordering:
 * - Score = (priority * 1_000_000_000) + timestamp_millis
 * - Lower score = higher priority and earlier time = first in queue
 */
@Service
public class PrintQueueServiceImpl implements PrintQueueService {

    private static final Logger log = LoggerFactory.getLogger(PrintQueueServiceImpl.class);

    private static final String QUEUE_PREFIX = "print:queue:";
    private static final long PRIORITY_MULTIPLIER = 1_000_000_000L;

    private final StringRedisTemplate redisTemplate;
    private final ZSetOperations<String, String> zSetOps;

    public PrintQueueServiceImpl(StringRedisTemplate redisTemplate) {
        this.redisTemplate = redisTemplate;
        this.zSetOps = redisTemplate.opsForZSet();
    }

    @Override
    public void addToQueue(UUID printJobId, String tenantId, int priority) {
        String queueKey = getQueueKey(tenantId);
        double score = calculateScore(priority);

        zSetOps.add(queueKey, printJobId.toString(), score);
        log.debug("Added print job {} to queue {} with priority {} (score: {})",
            printJobId, tenantId, priority, score);
    }

    @Override
    public boolean removeFromQueue(UUID printJobId, String tenantId) {
        String queueKey = getQueueKey(tenantId);
        Long removed = zSetOps.remove(queueKey, printJobId.toString());
        boolean wasRemoved = removed != null && removed > 0;

        if (wasRemoved) {
            log.debug("Removed print job {} from queue {}", printJobId, tenantId);
        }

        return wasRemoved;
    }

    @Override
    public List<UUID> getNextBatch(String tenantId, int batchSize) {
        String queueKey = getQueueKey(tenantId);

        Set<String> batch = zSetOps.rangeByScore(queueKey, 0, Double.MAX_VALUE, 0, batchSize);

        if (batch == null || batch.isEmpty()) {
            return List.of();
        }

        return batch.stream()
            .map(UUID::fromString)
            .collect(Collectors.toList());
    }

    @Override
    public long getQueuePosition(UUID printJobId, String tenantId) {
        String queueKey = getQueueKey(tenantId);
        Long rank = zSetOps.rank(queueKey, printJobId.toString());
        return rank != null ? rank : -1;
    }

    @Override
    public long getQueueSize(String tenantId) {
        String queueKey = getQueueKey(tenantId);
        Long size = zSetOps.size(queueKey);
        return size != null ? size : 0;
    }

    @Override
    public boolean updatePriority(UUID printJobId, String tenantId, int newPriority) {
        String queueKey = getQueueKey(tenantId);
        String jobIdStr = printJobId.toString();

        // Check if job exists in queue
        Double currentScore = zSetOps.score(queueKey, jobIdStr);
        if (currentScore == null) {
            return false;
        }

        // Calculate new score preserving timestamp component
        long timestampComponent = (long) (currentScore % PRIORITY_MULTIPLIER);
        double newScore = (newPriority * PRIORITY_MULTIPLIER) + timestampComponent;

        zSetOps.add(queueKey, jobIdStr, newScore);
        log.debug("Updated priority for job {} to {} (new score: {})", printJobId, newPriority, newScore);

        return true;
    }

    @Override
    public void moveToFront(UUID printJobId, String tenantId) {
        String queueKey = getQueueKey(tenantId);
        String jobIdStr = printJobId.toString();

        // Remove and re-add with lowest possible score (priority 0, timestamp 0)
        zSetOps.remove(queueKey, jobIdStr);
        zSetOps.add(queueKey, jobIdStr, 0.0);

        log.info("Moved print job {} to front of queue {}", printJobId, tenantId);
    }

    @Override
    public long clearQueue(String tenantId) {
        String queueKey = getQueueKey(tenantId);
        Long size = zSetOps.size(queueKey);

        if (size != null && size > 0) {
            redisTemplate.delete(queueKey);
            log.warn("Cleared print queue for tenant {}. {} jobs removed.", tenantId, size);
            return size;
        }

        return 0;
    }

    @Override
    public boolean isInQueue(UUID printJobId, String tenantId) {
        String queueKey = getQueueKey(tenantId);
        Double score = zSetOps.score(queueKey, printJobId.toString());
        return score != null;
    }

    @Override
    public List<UUID> getAllInQueue(String tenantId) {
        String queueKey = getQueueKey(tenantId);
        Set<String> all = zSetOps.rangeByScore(queueKey, 0, Double.MAX_VALUE);

        if (all == null || all.isEmpty()) {
            return List.of();
        }

        return all.stream()
            .map(UUID::fromString)
            .collect(Collectors.toList());
    }

    private String getQueueKey(String tenantId) {
        return QUEUE_PREFIX + tenantId;
    }

    private double calculateScore(int priority) {
        // Score = priority * 1B + current timestamp
        // Lower priority number = lower score = higher priority
        return (priority * PRIORITY_MULTIPLIER) + System.currentTimeMillis();
    }
}
