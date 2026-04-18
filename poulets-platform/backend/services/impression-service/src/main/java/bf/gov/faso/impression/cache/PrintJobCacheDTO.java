package bf.gov.faso.impression.cache;

import bf.gov.faso.impression.entity.PrintJob;

import java.time.Instant;
import java.util.HashMap;
import java.util.Map;

/**
 * DTO for DragonflyDB cache of print jobs.
 * Contains all mutable workflow fields so cache-only reads return complete state.
 * TTL 21 days, key pattern: ec:impression:data:{id}
 */
public record PrintJobCacheDTO(
    // Identity (immutable)
    String id,
    String documentId,
    String demandeId,
    String tenantId,
    String clientId,
    // Searchable
    String documentType,
    String documentReference,
    String status,
    String priority,
    // Workflow (mutable)
    String operatorId,
    String documentHash,
    String blockchainHash,
    String pdfStoragePath,
    String qrVerificationCode,
    String verificationUrl,
    // WORM
    String wormBucket,
    String wormObjectKey,
    String wormLocked,
    String wormLockedAt,
    String wormRetentionUntil,
    // Print tracking
    String copiesCount,
    String copiesPrinted,
    String printedAt,
    String deliveredAt,
    String deliveredTo,
    String deliveryMethod,
    String recipientSignature,
    // Reprint tracking
    String reprintCount,
    String reprintReason,
    String reprintAuthorizedBy,
    String originalPrintJobId,
    // Error / notes
    String errorMessage,
    String notes,
    // Metadata
    Map<String, String> metadata,
    Instant createdAt,
    Instant updatedAt
) {

    /**
     * Builds a PrintJobCacheDTO from a JPA entity.
     */
    public static PrintJobCacheDTO fromEntity(PrintJob job) {
        Map<String, String> meta = new HashMap<>();
        if (job.getMetadata() != null) {
            job.getMetadata().forEach((k, v) -> {
                if (v != null) meta.put(k, v.toString());
            });
        }

        return new PrintJobCacheDTO(
            job.getId().toString(),
            job.getDocumentId().toString(),
            job.getDemandeId().toString(),
            job.getTenantId(),
            job.getClientId() != null ? job.getClientId().toString() : "",
            job.getDocumentType(),
            job.getDocumentReference(),
            job.getStatus() != null ? job.getStatus().name() : "",
            String.valueOf(job.getPriority()),
            job.getOperatorId() != null ? job.getOperatorId().toString() : "",
            job.getDocumentHash(),
            job.getBlockchainHash(),
            job.getPdfStoragePath(),
            job.getQrVerificationCode(),
            job.getVerificationUrl(),
            job.getWormBucket(),
            job.getWormObjectKey(),
            String.valueOf(job.isWormLocked()),
            job.getWormLockedAt() != null ? job.getWormLockedAt().toString() : null,
            job.getWormRetentionUntil() != null ? job.getWormRetentionUntil().toString() : null,
            String.valueOf(job.getCopiesCount()),
            String.valueOf(job.getCopiesPrinted()),
            job.getPrintedAt() != null ? job.getPrintedAt().toString() : null,
            job.getDeliveredAt() != null ? job.getDeliveredAt().toString() : null,
            job.getDeliveredTo(),
            job.getDeliveryMethod(),
            job.getRecipientSignature(),
            String.valueOf(job.getReprintCount()),
            job.getReprintReason(),
            job.getReprintAuthorizedBy() != null ? job.getReprintAuthorizedBy().toString() : null,
            job.getOriginalPrintJobId() != null ? job.getOriginalPrintJobId().toString() : null,
            job.getErrorMessage(),
            job.getNotes(),
            meta,
            job.getCreatedAt(),
            job.getUpdatedAt()
        );
    }

    /**
     * Returns a new DTO with updated workflow fields after a print/deliver/status change.
     * Immutable record pattern — creates a new instance with updated fields.
     */
    public PrintJobCacheDTO withWorkflowUpdate(
            String newStatus,
            String newOperatorId,
            String newDocumentHash,
            String newBlockchainHash,
            String newPrintedAt,
            String newDeliveredAt,
            String newDeliveredTo,
            String newDeliveryMethod,
            String newErrorMessage,
            String newWormLocked,
            String newWormBucket,
            String newWormObjectKey,
            String newCopiesPrinted
    ) {
        return new PrintJobCacheDTO(
            id, documentId, demandeId, tenantId, clientId,
            documentType, documentReference,
            newStatus != null ? newStatus : status,
            priority,
            newOperatorId != null ? newOperatorId : operatorId,
            newDocumentHash != null ? newDocumentHash : documentHash,
            newBlockchainHash != null ? newBlockchainHash : blockchainHash,
            pdfStoragePath, qrVerificationCode, verificationUrl,
            newWormBucket != null ? newWormBucket : wormBucket,
            newWormObjectKey != null ? newWormObjectKey : wormObjectKey,
            newWormLocked != null ? newWormLocked : wormLocked,
            wormLockedAt, wormRetentionUntil,
            copiesCount,
            newCopiesPrinted != null ? newCopiesPrinted : copiesPrinted,
            newPrintedAt != null ? newPrintedAt : printedAt,
            newDeliveredAt != null ? newDeliveredAt : deliveredAt,
            newDeliveredTo != null ? newDeliveredTo : deliveredTo,
            newDeliveryMethod != null ? newDeliveryMethod : deliveryMethod,
            recipientSignature,
            reprintCount, reprintReason, reprintAuthorizedBy, originalPrintJobId,
            newErrorMessage != null ? newErrorMessage : errorMessage,
            notes,
            metadata, createdAt, Instant.now()
        );
    }
}
