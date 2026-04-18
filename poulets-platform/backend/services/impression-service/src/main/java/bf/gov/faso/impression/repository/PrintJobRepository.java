package bf.gov.faso.impression.repository;

import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.entity.PrintStatus;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * Repository for PrintJob entities.
 */
@Repository
public interface PrintJobRepository extends JpaRepository<PrintJob, UUID>, JpaSpecificationExecutor<PrintJob> {

    /**
     * Find a print job by ID and tenant ID.
     */
    Optional<PrintJob> findByIdAndTenantId(UUID id, String tenantId);

    /**
     * Find print jobs by document ID and tenant ID.
     */
    List<PrintJob> findByDocumentIdAndTenantId(UUID documentId, String tenantId);

    /**
     * Find print jobs by document ID (public verification - no tenant filter).
     */
    List<PrintJob> findByDocumentId(UUID documentId);

    /**
     * Find a print job by its HMAC-signed QR verification code.
     */
    Optional<PrintJob> findByQrVerificationCode(String qrVerificationCode);

    /**
     * Find print jobs by demande ID and tenant ID.
     */
    List<PrintJob> findByDemandeIdAndTenantId(UUID demandeId, String tenantId);

    /**
     * Find print jobs by status and tenant ID.
     */
    Page<PrintJob> findByStatusAndTenantIdOrderByPriorityAscCreatedAtAsc(
        PrintStatus status, String tenantId, Pageable pageable);

    /**
     * Find all pending print jobs in queue ordered by priority.
     */
    @Query("SELECT p FROM PrintJob p WHERE p.tenantId = :tenantId " +
           "AND p.status = 'EN_ATTENTE' " +
           "ORDER BY p.priority ASC, p.createdAt ASC")
    Page<PrintJob> findPendingQueue(@Param("tenantId") String tenantId, Pageable pageable);

    /**
     * Find the next job in queue (highest priority, oldest first).
     */
    @Query("SELECT p FROM PrintJob p WHERE p.tenantId = :tenantId " +
           "AND p.status = 'EN_ATTENTE' " +
           "ORDER BY p.priority ASC, p.createdAt ASC LIMIT 1")
    Optional<PrintJob> findNextInQueue(@Param("tenantId") String tenantId);

    /**
     * Find all jobs by operator ID.
     */
    Page<PrintJob> findByOperatorIdAndTenantId(UUID operatorId, String tenantId, Pageable pageable);

    /**
     * Find all jobs by client ID.
     */
    Page<PrintJob> findByClientIdAndTenantId(UUID clientId, String tenantId, Pageable pageable);

    /**
     * Find all WORM-locked jobs.
     */
    List<PrintJob> findByWormLockedTrueAndTenantId(String tenantId);

    /**
     * Find jobs pending delivery.
     */
    @Query("SELECT p FROM PrintJob p WHERE p.tenantId = :tenantId " +
           "AND p.status = 'IMPRIME' AND p.deliveredAt IS NULL " +
           "ORDER BY p.printedAt ASC")
    Page<PrintJob> findPendingDelivery(@Param("tenantId") String tenantId, Pageable pageable);

    /**
     * Count jobs by status and tenant.
     */
    long countByStatusAndTenantId(PrintStatus status, String tenantId);

    /**
     * Count total copies printed for a tenant.
     */
    @Query("SELECT COALESCE(SUM(p.copiesPrinted), 0) FROM PrintJob p WHERE p.tenantId = :tenantId")
    long sumCopiesPrintedByTenantId(@Param("tenantId") String tenantId);

    /**
     * Count jobs by document type.
     */
    @Query("SELECT p.documentType, COUNT(p) FROM PrintJob p " +
           "WHERE p.tenantId = :tenantId GROUP BY p.documentType")
    List<Object[]> countByDocumentTypeAndTenantId(@Param("tenantId") String tenantId);

    /**
     * Find jobs with reprint requests.
     */
    Page<PrintJob> findByStatusAndTenantIdAndReprintReasonIsNotNull(
        PrintStatus status, String tenantId, Pageable pageable);

    /**
     * Find original job for a reprint.
     */
    Optional<PrintJob> findByIdAndOriginalPrintJobIdIsNull(UUID id);

    /**
     * Find all reprints of an original job.
     */
    List<PrintJob> findByOriginalPrintJobIdAndTenantId(UUID originalId, String tenantId);

    /**
     * Check if document is already in queue or printed.
     */
    @Query("SELECT COUNT(p) > 0 FROM PrintJob p WHERE p.documentId = :documentId " +
           "AND p.tenantId = :tenantId AND p.status NOT IN ('ANNULE', 'ERREUR')")
    boolean existsActiveJobForDocument(@Param("documentId") UUID documentId, @Param("tenantId") String tenantId);

    /**
     * Find jobs created between dates.
     */
    Page<PrintJob> findByTenantIdAndCreatedAtBetween(
        String tenantId, Instant startDate, Instant endDate, Pageable pageable);

    /**
     * Update status in batch.
     */
    @Modifying
    @Query("UPDATE PrintJob p SET p.status = :newStatus WHERE p.status = :oldStatus " +
           "AND p.tenantId = :tenantId AND p.createdAt < :beforeDate")
    int updateStatusBatch(
        @Param("oldStatus") PrintStatus oldStatus,
        @Param("newStatus") PrintStatus newStatus,
        @Param("tenantId") String tenantId,
        @Param("beforeDate") Instant beforeDate);

    /**
     * Find jobs with errors.
     */
    Page<PrintJob> findByStatusAndTenantIdAndErrorMessageIsNotNull(
        PrintStatus status, String tenantId, Pageable pageable);

    /**
     * Calculate average queue time in minutes.
     * Uses native SQL since Hibernate 6.5+ doesn't support EXTRACT on interval results in JPQL.
     */
    @Query(value = "SELECT AVG(EXTRACT(EPOCH FROM (printed_at - created_at)) / 60) " +
           "FROM print_jobs WHERE tenant_id = :tenantId AND printed_at IS NOT NULL",
           nativeQuery = true)
    Double calculateAverageQueueTime(@Param("tenantId") String tenantId);

    /**
     * Calculate average print-to-delivery time in minutes.
     * Uses native SQL since Hibernate 6.5+ doesn't support EXTRACT on interval results in JPQL.
     */
    @Query(value = "SELECT AVG(EXTRACT(EPOCH FROM (delivered_at - printed_at)) / 60) " +
           "FROM print_jobs WHERE tenant_id = :tenantId AND delivered_at IS NOT NULL",
           nativeQuery = true)
    Double calculateAveragePrintToDeliveryTime(@Param("tenantId") String tenantId);
}
