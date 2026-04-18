package bf.gov.faso.impression.entity;

import io.hypersistence.utils.hibernate.type.json.JsonBinaryType;
import jakarta.persistence.*;
import org.hibernate.annotations.Type;
import org.springframework.data.annotation.CreatedDate;
import org.springframework.data.annotation.LastModifiedDate;
import org.springframework.data.jpa.domain.support.AuditingEntityListener;

import java.time.Instant;
import java.util.*;

/**
 * Entity representing a print job in the system.
 *
 * A print job is created when a validated document is ready for printing.
 * It tracks the entire lifecycle from queue to delivery, including WORM
 * storage and blockchain audit trail.
 */
@Entity
@Table(
    name = "print_jobs",
    indexes = {
        @Index(name = "idx_print_jobs_document_id", columnList = "document_id"),
        @Index(name = "idx_print_jobs_demande_id", columnList = "demande_id"),
        @Index(name = "idx_print_jobs_tenant_id", columnList = "tenant_id"),
        @Index(name = "idx_print_jobs_status", columnList = "status"),
        @Index(name = "idx_print_jobs_operator_id", columnList = "operator_id"),
        @Index(name = "idx_print_jobs_created_at", columnList = "created_at"),
        @Index(name = "idx_print_jobs_priority", columnList = "priority")
    }
)
@EntityListeners(AuditingEntityListener.class)
public class PrintJob {

    @Id
    @Column(name = "id", updatable = false, nullable = false)
    private UUID id;

    @Column(name = "document_id", nullable = false)
    private UUID documentId;

    @Column(name = "demande_id", nullable = false)
    private UUID demandeId;

    @Column(name = "tenant_id", nullable = false)
    private String tenantId;

    @Column(name = "client_id", nullable = false)
    private UUID clientId;

    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false, length = 30)
    private PrintStatus status = PrintStatus.EN_ATTENTE;

    @Column(name = "priority", nullable = false)
    private int priority = 5; // 1 = highest, 10 = lowest

    @Column(name = "document_type", nullable = false, length = 50)
    private String documentType;

    @Column(name = "document_reference", length = 100)
    private String documentReference;

    @Column(name = "operator_id")
    private UUID operatorId;

    @Column(name = "printed_at")
    private Instant printedAt;

    @Column(name = "delivered_at")
    private Instant deliveredAt;

    @Column(name = "delivered_to", length = 255)
    private String deliveredTo;

    @Column(name = "delivery_method", length = 50)
    private String deliveryMethod;

    @Column(name = "recipient_signature", length = 500)
    private String recipientSignature;

    @Column(name = "copies_count", nullable = false)
    private int copiesCount = 1;

    @Column(name = "copies_printed")
    private int copiesPrinted = 0;

    // WORM Storage fields
    @Column(name = "worm_bucket", length = 100)
    private String wormBucket;

    @Column(name = "worm_object_key", length = 500)
    private String wormObjectKey;

    @Column(name = "worm_locked_at")
    private Instant wormLockedAt;

    @Column(name = "worm_retention_until")
    private Instant wormRetentionUntil;

    @Column(name = "is_worm_locked", nullable = false)
    private boolean wormLocked = false;

    // Hash and integrity fields
    @Column(name = "document_hash", length = 128)
    private String documentHash;

    @Column(name = "blockchain_hash", length = 128)
    private String blockchainHash;

    @Column(name = "pdf_storage_path", length = 500)
    private String pdfStoragePath;

    /** Code de verification HMAC-signe provenant de validation-acte-service */
    @Column(name = "qr_verification_code", length = 500)
    private String qrVerificationCode;

    /** URL complete de verification publique du QR code */
    @Column(name = "verification_url", length = 500)
    private String verificationUrl;

    // Reprint tracking
    @Column(name = "reprint_count", nullable = false)
    private int reprintCount = 0;

    @Column(name = "reprint_reason", length = 500)
    private String reprintReason;

    @Column(name = "reprint_authorized_by")
    private UUID reprintAuthorizedBy;

    @Column(name = "original_print_job_id")
    private UUID originalPrintJobId;

    @Type(JsonBinaryType.class)
    @Column(name = "metadata", columnDefinition = "jsonb")
    private Map<String, Object> metadata = new HashMap<>();

    @Column(name = "error_message", length = 1000)
    private String errorMessage;

    @Column(name = "notes", length = 2000)
    private String notes;

    @CreatedDate
    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;

    @LastModifiedDate
    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt;

    @Version
    @Column(name = "version")
    private Long version;

    // Constructors
    public PrintJob() {
        this.id = UUID.randomUUID();
    }

    public PrintJob(UUID documentId, UUID demandeId, String tenantId, UUID clientId, String documentType) {
        this();
        this.documentId = documentId;
        this.demandeId = demandeId;
        this.tenantId = tenantId;
        this.clientId = clientId;
        this.documentType = documentType;
    }

    // Business methods

    /**
     * Checks if the document can be printed (not WORM locked or already delivered).
     */
    public boolean canPrint() {
        return !wormLocked && status != PrintStatus.DELIVRE && status != PrintStatus.ANNULE;
    }

    /**
     * Checks if the document can be reprinted (requires special authorization if WORM locked).
     */
    public boolean canReprint() {
        if (status == PrintStatus.ANNULE) {
            return false;
        }
        return !wormLocked || reprintAuthorizedBy != null;
    }

    /**
     * Marks the job as printed and applies WORM lock.
     */
    public void markAsPrinted(UUID operatorId, String documentHash, String blockchainHash) {
        this.operatorId = operatorId;
        this.printedAt = Instant.now();
        this.documentHash = documentHash;
        this.blockchainHash = blockchainHash;
        this.copiesPrinted++;
        this.status = PrintStatus.IMPRIME;
    }

    /**
     * Locks the document in WORM storage.
     */
    public void applyWormLock(String bucket, String objectKey, Instant retentionUntil) {
        this.wormBucket = bucket;
        this.wormObjectKey = objectKey;
        this.wormLockedAt = Instant.now();
        this.wormRetentionUntil = retentionUntil;
        this.wormLocked = true;
        this.status = PrintStatus.VERROUILLE_WORM;
    }

    /**
     * Marks the document as delivered.
     */
    public void markAsDelivered(String deliveredTo, String deliveryMethod, String signature) {
        this.deliveredAt = Instant.now();
        this.deliveredTo = deliveredTo;
        this.deliveryMethod = deliveryMethod;
        this.recipientSignature = signature;
        this.status = PrintStatus.DELIVRE;
    }

    /**
     * Requests a reprint with justification.
     */
    public void requestReprint(String reason) {
        this.reprintReason = reason;
        this.status = PrintStatus.REPRINT_DEMANDE;
    }

    /**
     * Authorizes a reprint (required for WORM-locked documents).
     */
    public void authorizeReprint(UUID authorizedBy) {
        this.reprintAuthorizedBy = authorizedBy;
        this.reprintCount++;
    }

    /**
     * Marks the job as failed.
     */
    public void markAsFailed(String errorMessage) {
        this.errorMessage = errorMessage;
        this.status = PrintStatus.ERREUR;
    }

    /**
     * Cancels the print job.
     */
    public void cancel() {
        this.status = PrintStatus.ANNULE;
    }

    // Getters and Setters
    public UUID getId() {
        return id;
    }

    public void setId(UUID id) {
        this.id = id;
    }

    public UUID getDocumentId() {
        return documentId;
    }

    public void setDocumentId(UUID documentId) {
        this.documentId = documentId;
    }

    public UUID getDemandeId() {
        return demandeId;
    }

    public void setDemandeId(UUID demandeId) {
        this.demandeId = demandeId;
    }

    public String getTenantId() {
        return tenantId;
    }

    public void setTenantId(String tenantId) {
        this.tenantId = tenantId;
    }

    public UUID getClientId() {
        return clientId;
    }

    public void setClientId(UUID clientId) {
        this.clientId = clientId;
    }

    public PrintStatus getStatus() {
        return status;
    }

    public void setStatus(PrintStatus status) {
        this.status = status;
    }

    public int getPriority() {
        return priority;
    }

    public void setPriority(int priority) {
        this.priority = priority;
    }

    public String getDocumentType() {
        return documentType;
    }

    public void setDocumentType(String documentType) {
        this.documentType = documentType;
    }

    public String getDocumentReference() {
        return documentReference;
    }

    public void setDocumentReference(String documentReference) {
        this.documentReference = documentReference;
    }

    public UUID getOperatorId() {
        return operatorId;
    }

    public void setOperatorId(UUID operatorId) {
        this.operatorId = operatorId;
    }

    public Instant getPrintedAt() {
        return printedAt;
    }

    public void setPrintedAt(Instant printedAt) {
        this.printedAt = printedAt;
    }

    public Instant getDeliveredAt() {
        return deliveredAt;
    }

    public void setDeliveredAt(Instant deliveredAt) {
        this.deliveredAt = deliveredAt;
    }

    public String getDeliveredTo() {
        return deliveredTo;
    }

    public void setDeliveredTo(String deliveredTo) {
        this.deliveredTo = deliveredTo;
    }

    public String getDeliveryMethod() {
        return deliveryMethod;
    }

    public void setDeliveryMethod(String deliveryMethod) {
        this.deliveryMethod = deliveryMethod;
    }

    public String getRecipientSignature() {
        return recipientSignature;
    }

    public void setRecipientSignature(String recipientSignature) {
        this.recipientSignature = recipientSignature;
    }

    public int getCopiesCount() {
        return copiesCount;
    }

    public void setCopiesCount(int copiesCount) {
        this.copiesCount = copiesCount;
    }

    public int getCopiesPrinted() {
        return copiesPrinted;
    }

    public void setCopiesPrinted(int copiesPrinted) {
        this.copiesPrinted = copiesPrinted;
    }

    public String getWormBucket() {
        return wormBucket;
    }

    public void setWormBucket(String wormBucket) {
        this.wormBucket = wormBucket;
    }

    public String getWormObjectKey() {
        return wormObjectKey;
    }

    public void setWormObjectKey(String wormObjectKey) {
        this.wormObjectKey = wormObjectKey;
    }

    public Instant getWormLockedAt() {
        return wormLockedAt;
    }

    public void setWormLockedAt(Instant wormLockedAt) {
        this.wormLockedAt = wormLockedAt;
    }

    public Instant getWormRetentionUntil() {
        return wormRetentionUntil;
    }

    public void setWormRetentionUntil(Instant wormRetentionUntil) {
        this.wormRetentionUntil = wormRetentionUntil;
    }

    public boolean isWormLocked() {
        return wormLocked;
    }

    public void setWormLocked(boolean wormLocked) {
        this.wormLocked = wormLocked;
    }

    public String getDocumentHash() {
        return documentHash;
    }

    public void setDocumentHash(String documentHash) {
        this.documentHash = documentHash;
    }

    public String getBlockchainHash() {
        return blockchainHash;
    }

    public void setBlockchainHash(String blockchainHash) {
        this.blockchainHash = blockchainHash;
    }

    public String getPdfStoragePath() {
        return pdfStoragePath;
    }

    public void setPdfStoragePath(String pdfStoragePath) {
        this.pdfStoragePath = pdfStoragePath;
    }

    public String getQrVerificationCode() {
        return qrVerificationCode;
    }

    public void setQrVerificationCode(String qrVerificationCode) {
        this.qrVerificationCode = qrVerificationCode;
    }

    public String getVerificationUrl() {
        return verificationUrl;
    }

    public void setVerificationUrl(String verificationUrl) {
        this.verificationUrl = verificationUrl;
    }

    public int getReprintCount() {
        return reprintCount;
    }

    public void setReprintCount(int reprintCount) {
        this.reprintCount = reprintCount;
    }

    public String getReprintReason() {
        return reprintReason;
    }

    public void setReprintReason(String reprintReason) {
        this.reprintReason = reprintReason;
    }

    public UUID getReprintAuthorizedBy() {
        return reprintAuthorizedBy;
    }

    public void setReprintAuthorizedBy(UUID reprintAuthorizedBy) {
        this.reprintAuthorizedBy = reprintAuthorizedBy;
    }

    public UUID getOriginalPrintJobId() {
        return originalPrintJobId;
    }

    public void setOriginalPrintJobId(UUID originalPrintJobId) {
        this.originalPrintJobId = originalPrintJobId;
    }

    public Map<String, Object> getMetadata() {
        return metadata;
    }

    public void setMetadata(Map<String, Object> metadata) {
        this.metadata = metadata;
    }

    public String getErrorMessage() {
        return errorMessage;
    }

    public void setErrorMessage(String errorMessage) {
        this.errorMessage = errorMessage;
    }

    public String getNotes() {
        return notes;
    }

    public void setNotes(String notes) {
        this.notes = notes;
    }

    public Instant getCreatedAt() {
        return createdAt;
    }

    public void setCreatedAt(Instant createdAt) {
        this.createdAt = createdAt;
    }

    public Instant getUpdatedAt() {
        return updatedAt;
    }

    public void setUpdatedAt(Instant updatedAt) {
        this.updatedAt = updatedAt;
    }

    public Long getVersion() {
        return version;
    }

    public void setVersion(Long version) {
        this.version = version;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        PrintJob printJob = (PrintJob) o;
        return Objects.equals(id, printJob.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }

    @Override
    public String toString() {
        return "PrintJob{" +
                "id=" + id +
                ", documentId=" + documentId +
                ", demandeId=" + demandeId +
                ", tenantId='" + tenantId + '\'' +
                ", status=" + status +
                ", priority=" + priority +
                ", wormLocked=" + wormLocked +
                ", createdAt=" + createdAt +
                '}';
    }
}
