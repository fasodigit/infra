package bf.gov.faso.impression.client;

import bf.gov.actes.grpc.verification.EcVerificationServiceGrpc;
import bf.gov.actes.grpc.verification.RegisterDocumentRequest;
import bf.gov.actes.grpc.verification.RegisterDocumentResponse;
import io.github.resilience4j.circuitbreaker.annotation.CircuitBreaker;
import io.github.resilience4j.retry.annotation.Retry;
import net.devh.boot.grpc.client.inject.GrpcClient;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.TimeUnit;

/**
 * gRPC client for ec-verification-ms.
 * Registers documents after PDF generation to obtain a verification token and URL.
 */
@Component
public class VerificationGrpcClient {

    private static final Logger log = LoggerFactory.getLogger(VerificationGrpcClient.class);

    @GrpcClient("ec-verification-ms")
    private EcVerificationServiceGrpc.EcVerificationServiceBlockingStub verificationStub;

    public record VerificationResult(String token, String verificationUrl) {}

    /**
     * Registers a document with the verification service and returns the token + URL.
     *
     * @return Optional containing token and URL, or empty if registration fails
     */
    @CircuitBreaker(name = "verificationService", fallbackMethod = "registerDocumentFallback")
    @Retry(name = "verificationService")
    public Optional<VerificationResult> registerDocument(
            UUID documentId,
            UUID demandeId,
            String tenantId,
            String reference,
            String numeroActe,
            String typeDocument,
            String issuerName,
            String canonicalData,
            String minioKey,
            long expiresAtEpoch) {
        try {
            RegisterDocumentRequest request = RegisterDocumentRequest.newBuilder()
                .setDocumentId(documentId.toString())
                .setDemandeId(demandeId.toString())
                .setTenantId(tenantId)
                .setReference(reference != null ? reference : "")
                .setNumeroActe(numeroActe != null ? numeroActe : "")
                .setTypeDocument(typeDocument != null ? typeDocument : "")
                .setIssuerName(issuerName != null ? issuerName : "")
                .setCanonicalData(canonicalData != null ? canonicalData : "")
                .setMinioKey(minioKey != null ? minioKey : "")
                .setExpiresAtEpoch(expiresAtEpoch)
                .build();

            RegisterDocumentResponse response = verificationStub
                .withDeadlineAfter(500, TimeUnit.MILLISECONDS)
                .registerDocument(request);

            if (response.getSuccess()) {
                log.info("Document registered for verification: token={}, url={}",
                    response.getToken(), response.getVerificationUrl());
                return Optional.of(new VerificationResult(
                    response.getToken(), response.getVerificationUrl()));
            } else {
                log.warn("Verification registration failed: {}", response.getErrorMessage());
                return Optional.empty();
            }
        } catch (Exception e) {
            log.warn("Failed to register document for verification (non-blocking): {}", e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Fallback when verification-service circuit breaker is open.
     */
    private Optional<VerificationResult> registerDocumentFallback(
            UUID documentId, UUID demandeId, String tenantId, String reference,
            String numeroActe, String typeDocument, String issuerName,
            String canonicalData, String minioKey, long expiresAtEpoch, Exception e) {
        log.warn("CircuitBreaker fallback: verification-service unavailable for documentId={}: {}", documentId, e.getMessage());
        return Optional.empty();
    }
}
