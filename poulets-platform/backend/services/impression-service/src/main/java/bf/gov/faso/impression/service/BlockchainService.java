package bf.gov.faso.impression.service;

import bf.gov.faso.impression.dto.response.BlockchainVerificationResponse;
import bf.gov.faso.impression.entity.BlockchainAction;
import bf.gov.faso.impression.entity.BlockchainEntry;

import java.util.List;
import java.util.UUID;

/**
 * Service interface for blockchain audit trail operations.
 *
 * The blockchain provides an immutable audit trail of all print operations:
 * - Each entry is linked to the previous via hash chaining
 * - SHA-256 hashing for integrity
 * - Synchronized with external audit-log-ms (Rust) for additional security
 */
public interface BlockchainService {

    /**
     * Add a new entry to the blockchain.
     *
     * @param documentId    The document ID
     * @param printJobId    The print job ID (optional)
     * @param documentHash  The SHA-256 hash of the document
     * @param operatorId    The operator ID
     * @param tenantId      The tenant ID
     * @param action        The action being recorded
     * @return The created blockchain entry
     */
    BlockchainEntry addEntry(
        UUID documentId,
        UUID printJobId,
        String documentHash,
        UUID operatorId,
        String tenantId,
        BlockchainAction action
    );

    /**
     * Add an entry with additional details.
     *
     * @param documentId    The document ID
     * @param printJobId    The print job ID (optional)
     * @param documentHash  The SHA-256 hash of the document
     * @param operatorId    The operator ID
     * @param tenantId      The tenant ID
     * @param action        The action being recorded
     * @param details       Additional details
     * @param clientIp      The client IP address
     * @param userAgent     The user agent
     * @return The created blockchain entry
     */
    BlockchainEntry addEntry(
        UUID documentId,
        UUID printJobId,
        String documentHash,
        UUID operatorId,
        String tenantId,
        BlockchainAction action,
        String details,
        String clientIp,
        String userAgent
    );

    /**
     * Verify the integrity of a single block.
     *
     * @param blockHash  The block hash to verify
     * @param tenantId   The tenant ID
     * @return The verification result
     */
    BlockchainVerificationResponse verifyBlock(String blockHash, String tenantId);

    /**
     * Verify the entire blockchain for a tenant.
     *
     * @param tenantId  The tenant ID
     * @return True if the chain is valid
     */
    boolean verifyChainIntegrity(String tenantId);

    /**
     * Get all blockchain entries for a document.
     *
     * @param documentId  The document ID
     * @param tenantId    The tenant ID
     * @return List of blockchain entries
     */
    List<BlockchainEntry> getEntriesForDocument(UUID documentId, String tenantId);

    /**
     * Get all blockchain entries for a print job.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return List of blockchain entries
     */
    List<BlockchainEntry> getEntriesForPrintJob(UUID printJobId, String tenantId);

    /**
     * Get the latest blockchain entry for a tenant.
     *
     * @param tenantId  The tenant ID
     * @return The latest entry (or null if no entries)
     */
    BlockchainEntry getLatestEntry(String tenantId);

    /**
     * Initialize the blockchain for a new tenant (create genesis block).
     *
     * @param tenantId    The tenant ID
     * @param operatorId  The operator creating the genesis block
     * @return The genesis block
     */
    BlockchainEntry initializeChain(String tenantId, UUID operatorId);

    /**
     * Check if the blockchain has been initialized for a tenant.
     *
     * @param tenantId  The tenant ID
     * @return True if initialized
     */
    boolean isChainInitialized(String tenantId);

    /**
     * Get the total number of blocks for a tenant.
     *
     * @param tenantId  The tenant ID
     * @return The block count
     */
    long getBlockCount(String tenantId);

    /**
     * Sync pending entries to the external audit-log-ms service.
     *
     * @return Number of entries synced
     */
    int syncToAuditLogService();
}
