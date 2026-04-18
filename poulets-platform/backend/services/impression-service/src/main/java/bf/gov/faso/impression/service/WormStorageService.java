package bf.gov.faso.impression.service;

import java.time.Instant;
import java.util.UUID;

/**
 * Service interface for WORM (Write Once Read Many) storage operations.
 *
 * WORM storage ensures document immutability after printing:
 * - Documents cannot be modified or deleted
 * - MinIO Object Lock in COMPLIANCE mode
 * - Retention period of 10 years
 */
public interface WormStorageService {

    /**
     * Store a document in WORM storage.
     *
     * @param documentId  The document ID
     * @param pdfBytes    The PDF content
     * @param tenantId    The tenant ID
     * @return The storage result with hashes
     */
    WormStorageResult storeImmutable(UUID documentId, byte[] pdfBytes, String tenantId);

    /**
     * Retrieve a document from WORM storage.
     *
     * @param bucket     The bucket name
     * @param objectKey  The object key
     * @param tenantId   The tenant ID
     * @return The document content
     */
    byte[] retrieveDocument(String bucket, String objectKey, String tenantId);

    /**
     * Verify document integrity in WORM storage.
     *
     * @param bucket       The bucket name
     * @param objectKey    The object key
     * @param expectedHash The expected hash
     * @return True if integrity is valid
     */
    boolean verifyIntegrity(String bucket, String objectKey, String expectedHash);

    /**
     * Check if a document exists in WORM storage.
     *
     * @param bucket     The bucket name
     * @param objectKey  The object key
     * @return True if the document exists
     */
    boolean documentExists(String bucket, String objectKey);

    /**
     * Get the retention end date for a document.
     *
     * @param bucket     The bucket name
     * @param objectKey  The object key
     * @return The retention end date
     */
    Instant getRetentionEndDate(String bucket, String objectKey);

    /**
     * Result of WORM storage operation.
     */
    record WormStorageResult(
        UUID documentId,
        String bucket,
        String objectKey,
        String contentHash,
        String blockHash,
        Instant retentionUntil
    ) {}
}
