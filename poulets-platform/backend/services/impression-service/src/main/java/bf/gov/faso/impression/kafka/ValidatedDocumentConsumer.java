package bf.gov.faso.impression.kafka;

import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.service.ImpressionService;
import bf.gov.shared.eventbus.EventRecord;
import bf.gov.shared.eventbus.consume.StreamSubscribe;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.Map;
import java.util.UUID;

/**
 * DragonflyDB Stream consumer for validated documents.
 * Replaces the gRPC addToQueue() call from validation-service.
 * Creates a PrintJob in the impression queue when a document is validated.
 */
@Component
public class ValidatedDocumentConsumer {

    private static final Logger log = LoggerFactory.getLogger(ValidatedDocumentConsumer.class);

    private final ImpressionService impressionService;

    public ValidatedDocumentConsumer(ImpressionService impressionService) {
        this.impressionService = impressionService;
    }

    @StreamSubscribe(stream = "ec:validated.documents", group = "ec-impression-validated")
    public void handleValidatedDocument(EventRecord record) {
        try {
            String documentId = record.field("documentId", "");
            String demandeId = record.field("demandeId", "");
            String clientId = record.field("clientId", "");
            String tenantId = record.field("tenantId", "default");
            String documentType = record.field("documentType", "ACTE_DIVERS");
            String documentReference = record.field("documentReference", "");
            String pdfStoragePath = record.field("pdfStoragePath", "");
            String qrVerificationCode = record.field("qrVerificationCode", "");
            String verificationUrl = record.field("verificationUrl", "");

            log.info("Received validated document event: documentId={}, demandeId={}, type={}, tenant={}",
                    documentId, demandeId, documentType, tenantId);

            if (documentId.isEmpty() || demandeId.isEmpty()) {
                log.warn("Skipping validated document event with missing IDs: documentId={}, demandeId={}",
                        documentId, demandeId);
                return;
            }

            AddToQueueRequest request = new AddToQueueRequest(
                    UUID.fromString(documentId),
                    UUID.fromString(demandeId),
                    clientId.isEmpty() ? UUID.fromString(demandeId) : UUID.fromString(clientId),
                    documentType,
                    documentReference,
                    5, // default priority
                    1, // default copies
                    pdfStoragePath,
                    "Auto-queued from ec:validated.documents stream",
                    Map.of("source", "stream-validated-document",
                           "qrVerificationCode", qrVerificationCode,
                           "verificationUrl", verificationUrl),
                    qrVerificationCode,
                    verificationUrl
            );

            impressionService.addToQueue(request, tenantId);
            log.info("PrintJob created for validated document: documentId={}, tenant={}", documentId, tenantId);

        } catch (Exception e) {
            log.error("Error processing ec:validated.documents event: {}", e.getMessage(), e);
        }
    }
}
