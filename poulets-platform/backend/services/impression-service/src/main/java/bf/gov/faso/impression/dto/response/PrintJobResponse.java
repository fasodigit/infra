package bf.gov.faso.impression.dto.response;

import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.entity.PrintStatus;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * DTO Record for print job response.
 */
public record PrintJobResponse(
    UUID id,
    UUID documentId,
    UUID demandeId,
    String tenantId,
    UUID clientId,
    PrintStatus status,
    int priority,
    String documentType,
    String documentReference,
    UUID operatorId,
    Instant printedAt,
    Instant deliveredAt,
    String deliveredTo,
    String deliveryMethod,
    int copiesCount,
    int copiesPrinted,
    boolean wormLocked,
    String wormBucket,
    String wormObjectKey,
    Instant wormLockedAt,
    Instant wormRetentionUntil,
    String documentHash,
    String blockchainHash,
    String pdfStoragePath,
    int reprintCount,
    String reprintReason,
    UUID reprintAuthorizedBy,
    UUID originalPrintJobId,
    Map<String, Object> metadata,
    String errorMessage,
    String notes,
    Instant createdAt,
    Instant updatedAt
) {
    /**
     * Factory method to create a response from entity.
     */
    public static PrintJobResponse fromEntity(PrintJob job) {
        return new PrintJobResponse(
            job.getId(),
            job.getDocumentId(),
            job.getDemandeId(),
            job.getTenantId(),
            job.getClientId(),
            job.getStatus(),
            job.getPriority(),
            job.getDocumentType(),
            job.getDocumentReference(),
            job.getOperatorId(),
            job.getPrintedAt(),
            job.getDeliveredAt(),
            job.getDeliveredTo(),
            job.getDeliveryMethod(),
            job.getCopiesCount(),
            job.getCopiesPrinted(),
            job.isWormLocked(),
            job.getWormBucket(),
            job.getWormObjectKey(),
            job.getWormLockedAt(),
            job.getWormRetentionUntil(),
            job.getDocumentHash(),
            job.getBlockchainHash(),
            job.getPdfStoragePath(),
            job.getReprintCount(),
            job.getReprintReason(),
            job.getReprintAuthorizedBy(),
            job.getOriginalPrintJobId(),
            job.getMetadata(),
            job.getErrorMessage(),
            job.getNotes(),
            job.getCreatedAt(),
            job.getUpdatedAt()
        );
    }

    /**
     * Factory method for list views (without sensitive/large fields).
     */
    public static PrintJobResponse fromEntitySummary(PrintJob job) {
        return new PrintJobResponse(
            job.getId(),
            job.getDocumentId(),
            job.getDemandeId(),
            job.getTenantId(),
            job.getClientId(),
            job.getStatus(),
            job.getPriority(),
            job.getDocumentType(),
            job.getDocumentReference(),
            job.getOperatorId(),
            job.getPrintedAt(),
            job.getDeliveredAt(),
            job.getDeliveredTo(),
            job.getDeliveryMethod(),
            job.getCopiesCount(),
            job.getCopiesPrinted(),
            job.isWormLocked(),
            null, // Hide bucket
            null, // Hide object key
            job.getWormLockedAt(),
            job.getWormRetentionUntil(),
            job.getDocumentHash(),
            job.getBlockchainHash(),
            null, // Hide storage path
            job.getReprintCount(),
            null, // Hide reprint reason
            job.getReprintAuthorizedBy(),
            job.getOriginalPrintJobId(),
            Map.of(), // Empty metadata for performance
            job.getErrorMessage(),
            null, // Hide notes
            job.getCreatedAt(),
            job.getUpdatedAt()
        );
    }
}
