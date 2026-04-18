package bf.gov.faso.impression.repository;

import bf.gov.faso.impression.entity.DeliveryMethod;
import bf.gov.faso.impression.entity.DeliveryRecord;
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
 * Repository for DeliveryRecord entities.
 */
@Repository
public interface DeliveryRecordRepository extends JpaRepository<DeliveryRecord, UUID> {

    /**
     * Find delivery record by ID and tenant ID.
     */
    Optional<DeliveryRecord> findByIdAndTenantId(UUID id, String tenantId);

    /**
     * Find delivery record by print job ID.
     */
    Optional<DeliveryRecord> findByPrintJobIdAndTenantId(UUID printJobId, String tenantId);

    /**
     * Find all deliveries for a document.
     */
    List<DeliveryRecord> findByDocumentIdAndTenantIdOrderByDeliveredAtDesc(UUID documentId, String tenantId);

    /**
     * Find all deliveries for a client.
     */
    Page<DeliveryRecord> findByClientIdAndTenantIdOrderByDeliveredAtDesc(
        UUID clientId, String tenantId, Pageable pageable);

    /**
     * Find deliveries by operator.
     */
    Page<DeliveryRecord> findByOperatorIdAndTenantIdOrderByDeliveredAtDesc(
        UUID operatorId, String tenantId, Pageable pageable);

    /**
     * Find deliveries by method.
     */
    Page<DeliveryRecord> findByDeliveryMethodAndTenantIdOrderByDeliveredAtDesc(
        DeliveryMethod method, String tenantId, Pageable pageable);

    /**
     * Find deliveries in a time range.
     */
    Page<DeliveryRecord> findByTenantIdAndDeliveredAtBetweenOrderByDeliveredAtDesc(
        String tenantId, Instant startTime, Instant endTime, Pageable pageable);

    /**
     * Count deliveries by tenant.
     */
    long countByTenantId(String tenantId);

    /**
     * Count deliveries by method and tenant.
     */
    @Query("SELECT d.deliveryMethod, COUNT(d) FROM DeliveryRecord d " +
           "WHERE d.tenantId = :tenantId GROUP BY d.deliveryMethod")
    List<Object[]> countByDeliveryMethodAndTenantId(@Param("tenantId") String tenantId);

    /**
     * Search deliveries by recipient name.
     */
    Page<DeliveryRecord> findByTenantIdAndRecipientNameContainingIgnoreCaseOrderByDeliveredAtDesc(
        String tenantId, String recipientName, Pageable pageable);

    /**
     * Find deliveries by tracking number.
     */
    Optional<DeliveryRecord> findByTrackingNumberAndTenantId(String trackingNumber, String tenantId);
}
