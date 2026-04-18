package bf.gov.faso.impression.repository;

import bf.gov.faso.impression.entity.BlockchainAction;
import bf.gov.faso.impression.entity.BlockchainEntry;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * Repository for BlockchainEntry entities.
 */
@Repository
public interface BlockchainRepository extends JpaRepository<BlockchainEntry, UUID> {

    /**
     * Find the last entry in the chain for a tenant.
     */
    Optional<BlockchainEntry> findTopByTenantIdOrderByTimestampDesc(String tenantId);

    /**
     * Find the last entry in the chain for a tenant by block number.
     */
    Optional<BlockchainEntry> findTopByTenantIdOrderByBlockNumberDesc(String tenantId);

    /**
     * Find an entry by block hash.
     */
    Optional<BlockchainEntry> findByBlockHashAndTenantId(String blockHash, String tenantId);

    /**
     * Find all entries for a document.
     */
    List<BlockchainEntry> findByDocumentIdAndTenantIdOrderByTimestampAsc(UUID documentId, String tenantId);

    /**
     * Find all entries for a print job.
     */
    List<BlockchainEntry> findByPrintJobIdAndTenantIdOrderByTimestampAsc(UUID printJobId, String tenantId);

    /**
     * Find all entries for a tenant ordered by timestamp (for chain verification).
     */
    List<BlockchainEntry> findByTenantIdOrderByTimestampAsc(String tenantId);

    /**
     * Find all entries for a tenant ordered by block number.
     */
    List<BlockchainEntry> findByTenantIdOrderByBlockNumberAsc(String tenantId);

    /**
     * Find entries by action.
     */
    Page<BlockchainEntry> findByActionAndTenantIdOrderByTimestampDesc(
        BlockchainAction action, String tenantId, Pageable pageable);

    /**
     * Find entries by operator.
     */
    Page<BlockchainEntry> findByOperatorIdAndTenantIdOrderByTimestampDesc(
        UUID operatorId, String tenantId, Pageable pageable);

    /**
     * Find entries in a time range.
     */
    Page<BlockchainEntry> findByTenantIdAndTimestampBetweenOrderByTimestampDesc(
        String tenantId, Instant startTime, Instant endTime, Pageable pageable);

    /**
     * Check if genesis block exists for tenant.
     */
    boolean existsByTenantIdAndAction(String tenantId, BlockchainAction action);

    /**
     * Count entries by tenant.
     */
    long countByTenantId(String tenantId);

    /**
     * Get the next block number for a tenant.
     */
    @Query("SELECT COALESCE(MAX(b.blockNumber), 0) + 1 FROM BlockchainEntry b WHERE b.tenantId = :tenantId")
    Long getNextBlockNumber(@Param("tenantId") String tenantId);

    /**
     * Find entries not yet synced to audit log service.
     */
    List<BlockchainEntry> findBySyncedToAuditLogFalseOrderByTimestampAsc();

    /**
     * Find entries not yet synced for a tenant.
     */
    List<BlockchainEntry> findByTenantIdAndSyncedToAuditLogFalseOrderByTimestampAsc(String tenantId);

    /**
     * Find entry by previous block hash (for chain navigation).
     */
    Optional<BlockchainEntry> findByPreviousBlockHashAndTenantId(String previousBlockHash, String tenantId);

    /**
     * Verify no gaps in block numbers for a tenant.
     */
    @Query("SELECT COUNT(b) FROM BlockchainEntry b WHERE b.tenantId = :tenantId " +
           "AND b.blockNumber > 0 AND NOT EXISTS (" +
           "  SELECT 1 FROM BlockchainEntry b2 WHERE b2.tenantId = :tenantId " +
           "  AND b2.blockNumber = b.blockNumber - 1)")
    long countBlockNumberGaps(@Param("tenantId") String tenantId);
}
