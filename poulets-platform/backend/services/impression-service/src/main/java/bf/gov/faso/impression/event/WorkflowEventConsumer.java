package bf.gov.faso.impression.event;

import bf.gov.shared.eventbus.EventRecord;
import bf.gov.shared.eventbus.consume.StreamSubscribe;
import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.service.ImpressionService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.Map;
import java.util.UUID;

/**
 * Consumes demande.impression-requested events from DragonflyDB Streams.
 * Replaces the gRPC server endpoint that validation-acte-service used to call
 * (ImpressionServiceGrpcClient.addToQueue).
 *
 * Creates a print job in the impression queue for the validated document.
 */
@Component
public class WorkflowEventConsumer {

    private static final Logger log = LoggerFactory.getLogger(WorkflowEventConsumer.class);

    private final ImpressionService impressionService;

    public WorkflowEventConsumer(ImpressionService impressionService) {
        this.impressionService = impressionService;
    }

    @StreamSubscribe(stream = "ec:demande.impression-requested", group = "impression-service", batchSize = 10)
    public void onImpressionRequested(EventRecord record) {
        String documentId = record.field("documentId");
        String demandeId = record.field("demandeId");
        String tenantId = record.field("tenantId");
        String clientId = record.field("clientId");
        String documentType = record.field("documentType");
        String documentReference = record.field("documentReference");
        String pdfStoragePath = record.field("pdfStoragePath");
        String qrVerificationCode = record.field("qrVerificationCode");
        String verificationUrl = record.field("verificationUrl");
        log.info("Received demande.impression-requested [documentId={}, demandeId={}]",
            documentId, demandeId);
        try {
            // AddToQueueRequest record order:
            // documentId, demandeId, clientId, documentType, documentReference,
            // priority, copiesCount, pdfStoragePath, notes, metadata,
            // qrVerificationCode, verificationUrl
            AddToQueueRequest request = new AddToQueueRequest(
                UUID.fromString(documentId),
                UUID.fromString(demandeId),
                clientId != null ? UUID.fromString(clientId) : null,
                documentType != null ? documentType : "ACTE_DIVERS",
                documentReference,
                5, // default priority
                1, // copies count
                pdfStoragePath != null ? pdfStoragePath : "",
                "Auto-queued via workflow stream",
                Map.of("source", "workflow-stream"),
                qrVerificationCode,
                verificationUrl
            );

            impressionService.addToQueue(request, tenantId);
            log.info("Document {} added to impression queue for demande {}", documentId, demandeId);
        } catch (Exception e) {
            log.error("Failed to add to impression queue [doc={}, demande={}]: {}",
                documentId, demandeId, e.getMessage(), e);
            throw e;
        }
    }
}
