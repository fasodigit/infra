package bf.gov.faso.impression.cache;

import bf.gov.faso.cache.dragonfly.DragonflyDBCacheService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.time.Duration;
import java.util.Optional;
import java.util.Set;

/**
 * DragonflyDB cache service for print jobs — write-behind pattern support.
 *
 * Keys (prefix ec: auto-added by DragonflyDBCacheService):
 * - impression:data:{printJobId}    -> JSON (PrintJobCacheDTO)
 * - impression:wb:pending           -> Set (dirty print job IDs awaiting flush)
 * - impression:search:{tenantId}:status:{status} -> Set (print job IDs by status)
 */
@Service
public class PrintJobSearchCacheService {

    private static final Logger log = LoggerFactory.getLogger(PrintJobSearchCacheService.class);

    private static final Duration TTL_21_DAYS = Duration.ofDays(21);
    private static final String DATA_PREFIX = "impression:data:";
    private static final String PENDING_FLUSH_KEY = "impression:wb:pending";
    private static final String SEARCH_PREFIX = "impression:search:";

    private final DragonflyDBCacheService cacheService;

    public PrintJobSearchCacheService(DragonflyDBCacheService cacheService) {
        this.cacheService = cacheService;
    }

    /**
     * Replaces the full cache entry with an updated DTO (all workflow fields included).
     * Used by write-behind pattern when print operations update multiple fields.
     */
    public void updateFullState(PrintJobCacheDTO dto) {
        try {
            cacheService.put(DATA_PREFIX + dto.id(), dto, TTL_21_DAYS);

            // Update status inverted index
            if (dto.tenantId() != null && dto.status() != null && !dto.status().isEmpty()) {
                cacheService.sAdd(
                    SEARCH_PREFIX + dto.tenantId() + ":status:" + dto.status(),
                    dto.id()
                );
            }

            log.debug("Cache updated for print job {}", dto.id());
        } catch (Exception e) {
            log.warn("Failed to update cache for print job {}: {}", dto.id(), e.getMessage());
        }
    }

    /**
     * Get a cached print job by ID.
     */
    public Optional<PrintJobCacheDTO> getFromCache(String id) {
        return cacheService.get(DATA_PREFIX + id, PrintJobCacheDTO.class);
    }

    /**
     * Adds a print job ID to the pending flush set (write-behind dirty tracking).
     */
    public void addToPendingFlush(String printJobId) {
        try {
            cacheService.sAdd(PENDING_FLUSH_KEY, printJobId);
        } catch (Exception e) {
            log.warn("Failed to add {} to pending flush: {}", printJobId, e.getMessage());
        }
    }

    /**
     * Returns all print job IDs pending flush to PostgreSQL.
     */
    public Set<String> getPendingFlushIds() {
        try {
            return cacheService.sMembers(PENDING_FLUSH_KEY);
        } catch (Exception e) {
            log.warn("Failed to read pending flush set: {}", e.getMessage());
            return Set.of();
        }
    }

    /**
     * Removes flushed IDs from the pending set.
     * Rebuilds the set without the flushed IDs since sRemove is not available.
     */
    public void removePendingFlush(Set<String> flushedIds) {
        if (flushedIds == null || flushedIds.isEmpty()) return;
        try {
            Set<String> remaining = cacheService.sMembers(PENDING_FLUSH_KEY);
            if (remaining == null || remaining.isEmpty()) return;

            cacheService.delete(PENDING_FLUSH_KEY);
            remaining.stream()
                .filter(id -> !flushedIds.contains(id))
                .forEach(id -> cacheService.sAdd(PENDING_FLUSH_KEY, id));
        } catch (Exception e) {
            log.warn("Failed to remove flushed IDs from pending set: {}", e.getMessage());
        }
    }

    /**
     * Evicts a print job from cache.
     */
    public void evict(String printJobId) {
        try {
            cacheService.delete(DATA_PREFIX + printJobId);
            log.debug("Evicted print job {} from cache", printJobId);
        } catch (Exception e) {
            log.warn("Failed to evict print job {} from cache: {}", printJobId, e.getMessage());
        }
    }
}
