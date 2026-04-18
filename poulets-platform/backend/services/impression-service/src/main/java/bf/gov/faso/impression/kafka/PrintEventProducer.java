package bf.gov.faso.impression.kafka;

import bf.gov.faso.impression.entity.BlockchainAction;
import bf.gov.faso.impression.entity.DeliveryMethod;
import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.entity.PrintStatus;
import bf.gov.shared.eventbus.publish.EventPublisher;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.time.Instant;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Event producer for print-related events.
 * Publishes events to DragonflyDB Streams via event-bus-lib.
 */
@Component
public class PrintEventProducer {

    private static final Logger log = LoggerFactory.getLogger(PrintEventProducer.class);

    private static final String STREAM_PRINT_EVENTS = "ec:print.events";
    private static final String STREAM_BLOCKCHAIN_EVENTS = "ec:blockchain.events";
    private static final String STREAM_ACTE_IMPRIME = "ec:acte.imprime";

    private final EventPublisher eventPublisher;
    private final ObjectMapper objectMapper;

    @Value("${event-bus.enabled:true}")
    private boolean eventBusEnabled;

    public PrintEventProducer(EventPublisher eventPublisher, ObjectMapper objectMapper) {
        this.eventPublisher = eventPublisher;
        this.objectMapper = objectMapper;
    }

    public void publishPrintStatusChange(
            UUID printJobId, UUID documentId, String tenantId,
            PrintStatus oldStatus, PrintStatus newStatus, UUID operatorId) {

        PrintStatusChangeEvent event = new PrintStatusChangeEvent(
            UUID.randomUUID(), "PRINT_STATUS_CHANGE", Instant.now(),
            printJobId, documentId, tenantId, oldStatus, newStatus, operatorId
        );

        eventPublisher.publish(STREAM_PRINT_EVENTS, printJobId.toString(), "PRINT_STATUS_CHANGE", event);
    }

    public void publishDocumentPrinted(
            UUID printJobId, UUID documentId, String tenantId,
            UUID operatorId, String documentHash, String blockchainHash) {

        DocumentPrintedEvent event = new DocumentPrintedEvent(
            UUID.randomUUID(), "DOCUMENT_PRINTED", Instant.now(),
            printJobId, documentId, tenantId, operatorId, documentHash, blockchainHash
        );

        eventPublisher.publish(STREAM_PRINT_EVENTS, printJobId.toString(), "DOCUMENT_PRINTED", event);
    }

    public void publishDocumentDelivered(
            UUID printJobId, UUID documentId, String tenantId,
            UUID operatorId, String recipientName, DeliveryMethod deliveryMethod) {

        DocumentDeliveredEvent event = new DocumentDeliveredEvent(
            UUID.randomUUID(), "DOCUMENT_DELIVERED", Instant.now(),
            printJobId, documentId, tenantId, operatorId, recipientName, deliveryMethod
        );

        eventPublisher.publish(STREAM_PRINT_EVENTS, printJobId.toString(), "DOCUMENT_DELIVERED", event);
    }

    public void publishBlockchainEntry(
            UUID entryId, UUID documentId, String tenantId,
            BlockchainAction action, String blockHash, Long blockNumber) {

        BlockchainEntryEvent event = new BlockchainEntryEvent(
            entryId, "BLOCKCHAIN_ENTRY", Instant.now(),
            documentId, tenantId, action, blockHash, blockNumber
        );

        eventPublisher.publish(STREAM_BLOCKCHAIN_EVENTS, entryId.toString(), "BLOCKCHAIN_ENTRY", event);
    }

    public void publishWormLock(
            UUID printJobId, UUID documentId, String tenantId,
            String bucket, String objectKey, Instant retentionUntil) {

        WormLockEvent event = new WormLockEvent(
            UUID.randomUUID(), "WORM_LOCK", Instant.now(),
            printJobId, documentId, tenantId, bucket, objectKey, retentionUntil
        );

        eventPublisher.publish(STREAM_PRINT_EVENTS, printJobId.toString(), "WORM_LOCK", event);
    }

    /**
     * Publish acte imprime event.
     * Consumed by document-delivery-service.
     */
    public void publishActeImprime(PrintJob job, byte[] pdfBytes) {
        Map<String, String> payload = new HashMap<>();
        payload.put("eventId", UUID.randomUUID().toString());
        payload.put("eventType", "ACTE_IMPRIME");
        payload.put("timestamp", Instant.now().toString());
        payload.put("documentId", job.getDemandeId().toString());
        payload.put("printJobId", job.getId().toString());
        payload.put("tenantId", job.getTenantId());
        payload.put("clientId", job.getClientId().toString());
        payload.put("documentHash", job.getDocumentHash());
        payload.put("typeDocument", job.getDocumentType());
        payload.put("copies", String.valueOf(job.getCopiesCount()));

        if (pdfBytes != null && pdfBytes.length > 0) {
            payload.put("pdfContent", java.util.Base64.getEncoder().encodeToString(pdfBytes));
        }
        if (job.getWormBucket() != null) {
            payload.put("wormBucket", job.getWormBucket());
        }
        if (job.getWormObjectKey() != null) {
            payload.put("wormObjectKey", job.getWormObjectKey());
        }
        if (job.getDocumentReference() != null) {
            payload.put("documentReference", job.getDocumentReference());
        }
        if (job.getQrVerificationCode() != null) {
            payload.put("verificationToken", job.getQrVerificationCode());
        }
        if (job.getVerificationUrl() != null) {
            payload.put("verificationUrl", job.getVerificationUrl());
        }

        eventPublisher.publish(STREAM_ACTE_IMPRIME, job.getDemandeId().toString(), payload);
        log.info("Published ACTE_IMPRIME event to {} for demandeId={}, pdfSize={}",
            STREAM_ACTE_IMPRIME, job.getDemandeId(), pdfBytes != null ? pdfBytes.length : 0);
    }

    // Event records
    public record PrintStatusChangeEvent(
        UUID eventId, String eventType, Instant timestamp,
        UUID printJobId, UUID documentId, String tenantId,
        PrintStatus oldStatus, PrintStatus newStatus, UUID operatorId
    ) {}

    public record DocumentPrintedEvent(
        UUID eventId, String eventType, Instant timestamp,
        UUID printJobId, UUID documentId, String tenantId,
        UUID operatorId, String documentHash, String blockchainHash
    ) {}

    public record DocumentDeliveredEvent(
        UUID eventId, String eventType, Instant timestamp,
        UUID printJobId, UUID documentId, String tenantId,
        UUID operatorId, String recipientName, DeliveryMethod deliveryMethod
    ) {}

    public record BlockchainEntryEvent(
        UUID eventId, String eventType, Instant timestamp,
        UUID documentId, String tenantId,
        BlockchainAction action, String blockHash, Long blockNumber
    ) {}

    public record WormLockEvent(
        UUID eventId, String eventType, Instant timestamp,
        UUID printJobId, UUID documentId, String tenantId,
        String bucket, String objectKey, Instant retentionUntil
    ) {}
}
