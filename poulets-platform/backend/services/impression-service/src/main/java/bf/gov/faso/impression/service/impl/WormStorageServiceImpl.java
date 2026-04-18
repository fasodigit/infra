package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.exception.WormViolationException;
import bf.gov.faso.impression.service.BlockchainService;
import bf.gov.faso.impression.service.WormStorageService;
import io.minio.*;
import io.minio.messages.Retention;
import io.minio.messages.RetentionMode;
import org.apache.commons.codec.digest.DigestUtils;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.time.Instant;
import java.time.ZonedDateTime;
import java.time.temporal.ChronoUnit;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Implementation of WORM storage service using MinIO with Object Lock.
 */
@Service
public class WormStorageServiceImpl implements WormStorageService {

    private static final Logger log = LoggerFactory.getLogger(WormStorageServiceImpl.class);

    private static final String WORM_BUCKET_PREFIX = "validated-";
    private static final long RETENTION_YEARS = 10;

    private final MinioClient minioClient;
    private final BlockchainService blockchainService;

    @Value("${minio.enabled:true}")
    private boolean minioEnabled;

    public WormStorageServiceImpl(MinioClient minioClient, BlockchainService blockchainService) {
        this.minioClient = minioClient;
        this.blockchainService = blockchainService;
    }

    @Override
    public WormStorageResult storeImmutable(UUID documentId, byte[] pdfBytes, String tenantId) {
        log.info("Storing document {} in WORM storage for tenant {}", documentId, tenantId);

        String bucketName = WORM_BUCKET_PREFIX + tenantId;
        String objectKey = "documents/" + documentId + ".pdf";

        // Calculate content hash
        String contentHash = DigestUtils.sha256Hex(pdfBytes);

        // Check if document already exists
        if (documentExists(bucketName, objectKey)) {
            throw new WormViolationException(
                "Document already exists in WORM storage", documentId, "store");
        }

        try {
            // Ensure bucket exists with object lock enabled
            ensureBucketWithObjectLock(bucketName);

            // Set retention date (10 years from now)
            Instant retentionUntil = Instant.now().plus(RETENTION_YEARS * 365, ChronoUnit.DAYS);
            ZonedDateTime retentionDate = ZonedDateTime.now().plusYears(RETENTION_YEARS);

            // Prepare metadata
            Map<String, String> userMetadata = new HashMap<>();
            userMetadata.put("content-hash", contentHash);
            userMetadata.put("created-at", Instant.now().toString());
            userMetadata.put("tenant-id", tenantId);
            userMetadata.put("document-id", documentId.toString());

            if (minioEnabled) {
                // Upload with Object Lock COMPLIANCE mode
                minioClient.putObject(
                    PutObjectArgs.builder()
                        .bucket(bucketName)
                        .object(objectKey)
                        .stream(new ByteArrayInputStream(pdfBytes), pdfBytes.length, -1)
                        .contentType("application/pdf")
                        .userMetadata(userMetadata)
                        .build()
                );

                // Set retention
                minioClient.setObjectRetention(
                    SetObjectRetentionArgs.builder()
                        .bucket(bucketName)
                        .object(objectKey)
                        .config(new Retention(RetentionMode.COMPLIANCE, retentionDate))
                        .build()
                );

                log.info("Document {} stored with COMPLIANCE retention until {}", documentId, retentionDate);
            } else {
                log.warn("MinIO disabled - simulating WORM storage for document {}", documentId);
            }

            // Get blockchain hash (will be set when blockchain entry is created)
            String blockHash = contentHash; // Placeholder until actual blockchain entry

            return new WormStorageResult(
                documentId,
                bucketName,
                objectKey,
                contentHash,
                blockHash,
                retentionUntil
            );

        } catch (WormViolationException e) {
            throw e;
        } catch (Exception e) {
            log.error("Failed to store document {} in WORM storage", documentId, e);
            throw new RuntimeException("WORM storage failed: " + e.getMessage(), e);
        }
    }

    @Override
    public byte[] retrieveDocument(String bucket, String objectKey, String tenantId) {
        log.debug("Retrieving document from WORM storage: {}/{}", bucket, objectKey);

        if (!minioEnabled) {
            log.warn("MinIO disabled - returning empty document");
            return new byte[0];
        }

        try {
            try (InputStream stream = minioClient.getObject(
                    GetObjectArgs.builder()
                        .bucket(bucket)
                        .object(objectKey)
                        .build())) {
                return stream.readAllBytes();
            }
        } catch (Exception e) {
            log.error("Failed to retrieve document from WORM storage: {}/{}", bucket, objectKey, e);
            throw new RuntimeException("Failed to retrieve document: " + e.getMessage(), e);
        }
    }

    @Override
    public boolean verifyIntegrity(String bucket, String objectKey, String expectedHash) {
        log.debug("Verifying integrity for: {}/{}", bucket, objectKey);

        if (!minioEnabled) {
            log.warn("MinIO disabled - skipping integrity verification");
            return true;
        }

        try {
            byte[] content = retrieveDocument(bucket, objectKey, null);
            String actualHash = DigestUtils.sha256Hex(content);
            boolean valid = actualHash.equals(expectedHash);

            if (!valid) {
                log.error("Integrity verification failed for {}/{}. Expected: {}, Actual: {}",
                    bucket, objectKey, expectedHash, actualHash);
            }

            return valid;
        } catch (Exception e) {
            log.error("Integrity verification error for {}/{}", bucket, objectKey, e);
            return false;
        }
    }

    @Override
    public boolean documentExists(String bucket, String objectKey) {
        if (!minioEnabled) {
            return false;
        }

        try {
            minioClient.statObject(
                StatObjectArgs.builder()
                    .bucket(bucket)
                    .object(objectKey)
                    .build()
            );
            return true;
        } catch (Exception e) {
            // Object doesn't exist or other error
            return false;
        }
    }

    @Override
    public Instant getRetentionEndDate(String bucket, String objectKey) {
        if (!minioEnabled) {
            return Instant.now().plus(RETENTION_YEARS * 365, ChronoUnit.DAYS);
        }

        try {
            Retention retention = minioClient.getObjectRetention(
                GetObjectRetentionArgs.builder()
                    .bucket(bucket)
                    .object(objectKey)
                    .build()
            );
            return retention.retainUntilDate().toInstant();
        } catch (Exception e) {
            log.error("Failed to get retention date for {}/{}", bucket, objectKey, e);
            return null;
        }
    }

    private void ensureBucketWithObjectLock(String bucketName) throws Exception {
        if (!minioEnabled) {
            return;
        }

        boolean exists = minioClient.bucketExists(
            BucketExistsArgs.builder()
                .bucket(bucketName)
                .build()
        );

        if (!exists) {
            // Create bucket with object lock enabled
            minioClient.makeBucket(
                MakeBucketArgs.builder()
                    .bucket(bucketName)
                    .objectLock(true)
                    .build()
            );
            log.info("Created WORM-enabled bucket: {}", bucketName);
        }
    }
}
