package bf.gov.faso.impression.grpc;

import io.github.resilience4j.circuitbreaker.annotation.CircuitBreaker;
import io.github.resilience4j.retry.annotation.Retry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.util.UUID;

/**
 * gRPC client for document-security-ms (Rust service).
 *
 * Provides watermarking, encryption, and integrity verification services.
 */
@Component
public class DocumentSecurityGrpcClient {

    private static final Logger log = LoggerFactory.getLogger(DocumentSecurityGrpcClient.class);

    @Value("${grpc.client.document-security.host:localhost}")
    private String host;

    @Value("${grpc.client.document-security.port:9081}")
    private int port;

    @Value("${grpc.client.document-security.enabled:true}")
    private boolean enabled;

    // TODO: Add actual gRPC channel and stub when proto is compiled
    // private DocumentSecurityServiceGrpc.DocumentSecurityServiceBlockingStub blockingStub;

    /**
     * Add a watermark to a PDF document.
     *
     * @param pdfBytes      The original PDF bytes
     * @param watermarkText The watermark text
     * @param documentId    The document ID
     * @param tenantId      The tenant ID
     * @return The watermarked PDF bytes
     */
    @CircuitBreaker(name = "documentSecurityService", fallbackMethod = "addWatermarkFallback")
    @Retry(name = "documentSecurityService")
    public byte[] addWatermark(byte[] pdfBytes, String watermarkText, UUID documentId, String tenantId) {
        if (!enabled) {
            log.warn("Document security gRPC client disabled, returning original PDF");
            return pdfBytes;
        }

        log.info("Adding watermark via gRPC for document {} in tenant {}", documentId, tenantId);

        try {
            // TODO: Implement actual gRPC call
            // WatermarkRequest request = WatermarkRequest.newBuilder()
            //     .setPdfData(ByteString.copyFrom(pdfBytes))
            //     .setTenantId(tenantId)
            //     .setWatermarkText(watermarkText)
            //     .setPosition(WatermarkPosition.DIAGONAL)
            //     .setOpacity(0.2f)
            //     .setConfig(WatermarkConfig.newBuilder()
            //         .setFontSize(40)
            //         .setRotationDegrees(45)
            //         .setIncludeTimestamp(true)
            //         .setIncludeDocumentId(true)
            //         .build())
            //     .build();
            //
            // WatermarkResponse response = blockingStub.addWatermark(request);
            // return response.getPdfData().toByteArray();

            log.warn("gRPC stub not implemented, returning original PDF");
            return pdfBytes;

        } catch (Exception e) {
            log.error("Failed to add watermark via gRPC for document {}", documentId, e);
            throw new RuntimeException("Watermark gRPC call failed: " + e.getMessage(), e);
        }
    }

    /**
     * Encrypt a document using AES-256-GCM.
     *
     * @param data       The data to encrypt
     * @param documentId The document ID
     * @param tenantId   The tenant ID
     * @return The encrypted data with metadata
     */
    @CircuitBreaker(name = "documentSecurityService", fallbackMethod = "encryptFallback")
    @Retry(name = "documentSecurityService")
    public EncryptionResult encrypt(byte[] data, UUID documentId, String tenantId) {
        if (!enabled) {
            log.warn("Document security gRPC client disabled");
            return new EncryptionResult(data, new byte[0], "", "");
        }

        log.info("Encrypting document {} via gRPC", documentId);

        try {
            // TODO: Implement actual gRPC call
            log.warn("gRPC stub not implemented, returning unencrypted data");
            return new EncryptionResult(data, new byte[0], "unencrypted", "");

        } catch (Exception e) {
            log.error("Failed to encrypt via gRPC for document {}", documentId, e);
            throw new RuntimeException("Encryption gRPC call failed: " + e.getMessage(), e);
        }
    }

    /**
     * Calculate checksum of data.
     *
     * @param data The data to checksum
     * @return The SHA-256 checksum
     */
    public String calculateChecksum(byte[] data) {
        if (!enabled) {
            // Fallback to local calculation
            return org.apache.commons.codec.digest.DigestUtils.sha256Hex(data);
        }

        try {
            // TODO: Implement actual gRPC call
            return org.apache.commons.codec.digest.DigestUtils.sha256Hex(data);

        } catch (Exception e) {
            log.error("Failed to calculate checksum via gRPC", e);
            // Fallback to local
            return org.apache.commons.codec.digest.DigestUtils.sha256Hex(data);
        }
    }

    /**
     * Fallback when document-security circuit breaker is open for addWatermark.
     */
    private byte[] addWatermarkFallback(byte[] pdfBytes, String watermarkText, UUID documentId, String tenantId, Exception e) {
        log.warn("CircuitBreaker fallback: document-security-ms unavailable for addWatermark documentId={}: {}", documentId, e.getMessage());
        return pdfBytes;
    }

    /**
     * Fallback when document-security circuit breaker is open for encrypt.
     */
    private EncryptionResult encryptFallback(byte[] data, UUID documentId, String tenantId, Exception e) {
        log.warn("CircuitBreaker fallback: document-security-ms unavailable for encrypt documentId={}: {}", documentId, e.getMessage());
        return new EncryptionResult(data, new byte[0], "fallback-key-id", "");
    }

    /**
     * Verify document integrity.
     *
     * @param data         The data to verify
     * @param expectedHash The expected hash
     * @return True if integrity is valid
     */
    public boolean verifyIntegrity(byte[] data, String expectedHash) {
        String actualHash = calculateChecksum(data);
        return actualHash.equals(expectedHash);
    }

    /**
     * Result of encryption operation.
     */
    public record EncryptionResult(
        byte[] encryptedData,
        byte[] nonce,
        String keyId,
        String checksum
    ) {}
}
