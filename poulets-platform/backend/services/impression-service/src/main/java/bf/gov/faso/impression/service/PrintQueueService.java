package bf.gov.faso.impression.service;

import java.util.List;
import java.util.UUID;

/**
 * Service interface for print queue operations using DragonflyDB (Redis-compatible).
 *
 * The queue uses sorted sets for priority-based ordering:
 * - Lower priority number = higher priority (1 is highest)
 * - FIFO within same priority level
 */
public interface PrintQueueService {

    /**
     * Add a print job to the queue.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @param priority    The priority (1-10, 1 = highest)
     */
    void addToQueue(UUID printJobId, String tenantId, int priority);

    /**
     * Remove a print job from the queue.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return True if removed, false if not in queue
     */
    boolean removeFromQueue(UUID printJobId, String tenantId);

    /**
     * Get the next batch of jobs from the queue.
     *
     * @param tenantId   The tenant ID
     * @param batchSize  Number of jobs to retrieve
     * @return List of print job IDs
     */
    List<UUID> getNextBatch(String tenantId, int batchSize);

    /**
     * Get the position of a job in the queue.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return The position (0-based), or -1 if not in queue
     */
    long getQueuePosition(UUID printJobId, String tenantId);

    /**
     * Get the current queue size.
     *
     * @param tenantId  The tenant ID
     * @return The number of jobs in queue
     */
    long getQueueSize(String tenantId);

    /**
     * Update the priority of a job in the queue.
     *
     * @param printJobId   The print job ID
     * @param tenantId     The tenant ID
     * @param newPriority  The new priority
     * @return True if updated, false if not in queue
     */
    boolean updatePriority(UUID printJobId, String tenantId, int newPriority);

    /**
     * Move a job to the front of the queue (emergency priority).
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     */
    void moveToFront(UUID printJobId, String tenantId);

    /**
     * Clear the entire queue for a tenant.
     *
     * @param tenantId  The tenant ID
     * @return Number of jobs cleared
     */
    long clearQueue(String tenantId);

    /**
     * Check if a job is in the queue.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return True if in queue
     */
    boolean isInQueue(UUID printJobId, String tenantId);

    /**
     * Get all job IDs in the queue (for debugging/admin).
     *
     * @param tenantId  The tenant ID
     * @return List of all print job IDs
     */
    List<UUID> getAllInQueue(String tenantId);
}
