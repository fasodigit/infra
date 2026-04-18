package bf.gov.faso.impression.grpc;

import bf.gov.faso.impression.service.ImpressionService;
import bf.gov.faso.impression.grpc.proto.*;
import io.grpc.Status;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.protobuf.ByteString;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

@GrpcService
public class ImpressionGrpcService extends ImpressionServiceGrpc.ImpressionServiceImplBase {

    private static final Logger log = LoggerFactory.getLogger(ImpressionGrpcService.class);

    private final ImpressionService impressionService;

    public ImpressionGrpcService(ImpressionService impressionService) {
        this.impressionService = impressionService;
    }

    @Override
    public void addToQueue(bf.gov.faso.impression.grpc.proto.AddToQueueRequest request,
                           StreamObserver<bf.gov.faso.impression.grpc.proto.PrintJobResponse> observer) {
        try {
            log.info("gRPC: Adding document {} to print queue (tenant={})",
                request.getDocumentId(), request.getTenantId());

            // Convert proto request to internal DTO
            Map<String, Object> metadata = new HashMap<>(request.getMetadataMap());

            bf.gov.faso.impression.dto.request.AddToQueueRequest internalRequest =
                new bf.gov.faso.impression.dto.request.AddToQueueRequest(
                    UUID.fromString(request.getDocumentId()),
                    UUID.fromString(request.getDemandeId()),
                    UUID.fromString(request.getClientId()),
                    request.getDocumentType(),
                    request.getDocumentReference(),
                    request.getPriority() > 0 ? request.getPriority() : 5,
                    request.getCopiesCount() > 0 ? request.getCopiesCount() : 1,
                    request.getPdfStoragePath(),
                    request.getNotes(),
                    metadata,
                    request.getQrVerificationCode().isEmpty() ? null : request.getQrVerificationCode(),
                    request.getVerificationUrl().isEmpty() ? null : request.getVerificationUrl()
                );

            bf.gov.faso.impression.dto.response.PrintJobResponse result =
                impressionService.addToQueue(internalRequest, request.getTenantId());

            // Convert internal response to proto
            bf.gov.faso.impression.grpc.proto.PrintJobResponse protoResponse =
                bf.gov.faso.impression.grpc.proto.PrintJobResponse.newBuilder()
                    .setId(result.id().toString())
                    .setDocumentId(result.documentId().toString())
                    .setDemandeId(result.demandeId().toString())
                    .setTenantId(result.tenantId() != null ? result.tenantId() : request.getTenantId())
                    .setClientId(result.clientId() != null ? result.clientId().toString() : "")
                    .setStatus(PrintStatusEnum.EN_ATTENTE)
                    .setPriority(result.priority())
                    .setDocumentType(result.documentType() != null ? result.documentType() : "")
                    .setDocumentReference(result.documentReference() != null ? result.documentReference() : "")
                    .setCopiesCount(result.copiesCount())
                    .setCreatedAt(result.createdAt() != null ? result.createdAt().toEpochMilli() : 0)
                    .build();

            observer.onNext(protoResponse);
            observer.onCompleted();

            log.info("gRPC: Document added to print queue, jobId={}", result.id());

        } catch (IllegalArgumentException e) {
            log.error("gRPC: Invalid request parameter: {}", e.getMessage(), e);
            observer.onError(Status.INVALID_ARGUMENT
                .withDescription("Invalid request parameter: " + e.getMessage())
                .asRuntimeException());
        } catch (Exception e) {
            log.error("gRPC: Failed to add to print queue: {}", e.getMessage(), e);
            observer.onError(Status.INTERNAL
                .withDescription("Failed to add to print queue: " + e.getMessage())
                .asRuntimeException());
        }
    }

    @Override
    public void printDocument(bf.gov.faso.impression.grpc.proto.PrintRequest request,
                              StreamObserver<FinalizedDocument> observer) {
        try {
            UUID printJobId = UUID.fromString(request.getPrintJobId());
            UUID operatorId = UUID.fromString(request.getOperatorId());
            String tenantId = request.getTenantId();

            log.info("gRPC: Printing document for job {} by operator {} (tenant={})",
                printJobId, operatorId, tenantId);

            bf.gov.faso.impression.dto.response.PrintJobResponse result =
                impressionService.printDocument(printJobId, operatorId, tenantId);

            bf.gov.faso.impression.grpc.proto.PrintJobResponse jobResponse =
                bf.gov.faso.impression.grpc.proto.PrintJobResponse.newBuilder()
                    .setId(result.id().toString())
                    .setDocumentId(result.documentId().toString())
                    .setDemandeId(result.demandeId().toString())
                    .setTenantId(result.tenantId() != null ? result.tenantId() : tenantId)
                    .setClientId(result.clientId() != null ? result.clientId().toString() : "")
                    .setStatus(PrintStatusEnum.valueOf(result.status().name()))
                    .setPriority(result.priority())
                    .setDocumentType(result.documentType() != null ? result.documentType() : "")
                    .setDocumentReference(result.documentReference() != null ? result.documentReference() : "")
                    .setOperatorId(operatorId.toString())
                    .setCopiesCount(result.copiesCount())
                    .setCopiesPrinted(result.copiesPrinted())
                    .setWormLocked(result.wormLocked())
                    .setDocumentHash(result.documentHash() != null ? result.documentHash() : "")
                    .setBlockchainHash(result.blockchainHash() != null ? result.blockchainHash() : "")
                    .setCreatedAt(result.createdAt() != null ? result.createdAt().toEpochMilli() : 0)
                    .setUpdatedAt(result.updatedAt() != null ? result.updatedAt().toEpochMilli() : 0)
                    .build();

            FinalizedDocument response = FinalizedDocument.newBuilder()
                .setJob(jobResponse)
                .setWormBucket(result.wormBucket() != null ? result.wormBucket() : "")
                .setWormObjectKey(result.wormObjectKey() != null ? result.wormObjectKey() : "")
                .setWormRetentionUntil(result.wormRetentionUntil() != null
                    ? result.wormRetentionUntil().toEpochMilli() : 0)
                .setDocumentHash(result.documentHash() != null ? result.documentHash() : "")
                .setBlockchainHash(result.blockchainHash() != null ? result.blockchainHash() : "")
                .build();

            observer.onNext(response);
            observer.onCompleted();

            log.info("gRPC: Document printed successfully, jobId={}, hash={}",
                result.id(), result.documentHash());

        } catch (IllegalArgumentException e) {
            log.error("gRPC: Invalid print request: {}", e.getMessage());
            observer.onError(Status.INVALID_ARGUMENT
                .withDescription(e.getMessage())
                .asRuntimeException());
        } catch (Exception e) {
            log.error("gRPC: Print failed for job {}: {}", request.getPrintJobId(), e.getMessage(), e);
            observer.onError(Status.INTERNAL
                .withDescription("Print failed: " + e.getMessage())
                .asRuntimeException());
        }
    }

    @Override
    public void getPrintedPdf(GetPrintedPdfRequest request,
                              StreamObserver<GetPrintedPdfResponse> observer) {
        try {
            UUID demandeId = UUID.fromString(request.getDemandeId());
            String tenantId = request.getTenantId();

            log.debug("gRPC: Fetching printed PDF for demande={}", demandeId);

            byte[] pdfBytes = impressionService.getLatestPdfByDemande(demandeId, tenantId);

            GetPrintedPdfResponse response = GetPrintedPdfResponse.newBuilder()
                .setFound(true)
                .setPdfContent(ByteString.copyFrom(pdfBytes))
                .setFilename("acte-" + demandeId + ".pdf")
                .build();

            observer.onNext(response);
            observer.onCompleted();

            log.info("gRPC: Sent printed PDF for demande={}, size={}", demandeId, pdfBytes.length);

        } catch (Exception e) {
            log.warn("gRPC: PDF not found for demande={}: {}", request.getDemandeId(), e.getMessage());

            GetPrintedPdfResponse response = GetPrintedPdfResponse.newBuilder()
                .setFound(false)
                .build();
            observer.onNext(response);
            observer.onCompleted();
        }
    }
}
