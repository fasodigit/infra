package bf.gov.faso.impression.kafka;

import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.entity.PrintStatus;
import bf.gov.faso.impression.repository.PrintJobRepository;
import bf.gov.faso.impression.service.ImpressionService;
import bf.gov.shared.eventbus.EventRecord;
import bf.gov.shared.eventbus.consume.StreamSubscribe;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * DragonflyDB Stream consumer that listens to ec:permis-armes.events:
 * - PERMIS_ARME_VALIDE: creates a PrintJob in EN_ATTENTE
 * - PERMIS_ARME_IMPRIME: finds the EN_ATTENTE job and triggers printDocument()
 */
@Component
public class PermisArmesPrintConsumer {

    private static final Logger log = LoggerFactory.getLogger(PermisArmesPrintConsumer.class);

    private final ImpressionService impressionService;
    private final PrintJobRepository printJobRepository;
    private final ObjectMapper objectMapper;

    public PermisArmesPrintConsumer(ImpressionService impressionService,
                                   PrintJobRepository printJobRepository,
                                   ObjectMapper objectMapper) {
        this.impressionService = impressionService;
        this.printJobRepository = printJobRepository;
        this.objectMapper = objectMapper;
    }

    @StreamSubscribe(stream = "ec:permis-armes.events", group = "ec-impression-permis-armes")
    public void handlePermisArmeEvent(EventRecord record) {
        try {
            String eventType;
            String data = record.field("data");
            JsonNode node;

            if (data != null) {
                node = objectMapper.readTree(data);
                eventType = node.path("eventType").asText("");
            } else {
                eventType = record.eventType();
                // Build a JsonNode from payload for field access
                node = objectMapper.valueToTree(record.payload());
            }

            switch (eventType) {
                case "PERMIS_ARME_VALIDE" -> handleValide(node);
                case "PERMIS_ARME_IMPRIME" -> handleImprime(node);
                default -> log.debug("Ignoring event type: {}", eventType);
            }

        } catch (Exception e) {
            log.error("Error processing ec:permis-armes.events: {}", e.getMessage(), e);
        }
    }

    private void handleValide(JsonNode node) {
        String demandeId = node.path("demandeId").asText();
        String clientId = node.path("clientId").asText();
        String tenantId = node.path("tenantId").asText();
        String demandeRef = node.path("demandeRef").asText("");
        String numeroPermis = node.path("metadata").path("numeroPermis").asText("");

        log.info("PERMIS_ARME_VALIDE received — demandeId: {}, tenant: {}, permis: {}",
            demandeId, tenantId, numeroPermis);

        AddToQueueRequest request = new AddToQueueRequest(
            UUID.fromString(demandeId),
            UUID.fromString(demandeId),
            UUID.fromString(clientId),
            "PERMIS_PORT_ARMES",
            demandeRef,
            5, 1, null,
            "Auto-queue from PERMIS_ARME_VALIDE — permis: " + numeroPermis,
            Map.of("numeroPermis", numeroPermis, "source", "stream-auto-queue"),
            null, null
        );

        impressionService.addToQueue(request, tenantId);
        log.info("PrintJob auto-created for permis {} in tenant {}", numeroPermis, tenantId);
    }

    private void handleImprime(JsonNode node) {
        String demandeId = node.path("demandeId").asText();
        String tenantId = node.path("tenantId").asText();
        String operateurId = node.path("operateurId").asText();

        log.info("PERMIS_ARME_IMPRIME received — demandeId: {}, tenant: {}, operator: {}",
            demandeId, tenantId, operateurId);

        List<PrintJob> jobs = printJobRepository.findByDemandeIdAndTenantId(
            UUID.fromString(demandeId), tenantId);

        PrintJob pendingJob = jobs.stream()
            .filter(j -> j.getStatus() == PrintStatus.EN_ATTENTE)
            .findFirst()
            .orElse(null);

        if (pendingJob == null) {
            log.warn("No EN_ATTENTE print job found for demandeId: {} in tenant: {}", demandeId, tenantId);
            return;
        }

        UUID operatorId = operateurId.isEmpty()
            ? pendingJob.getClientId()
            : UUID.fromString(operateurId);

        impressionService.printDocument(pendingJob.getId(), operatorId, tenantId);
        log.info("PrintJob {} processed (IMPRIME->WORM) for demandeId: {}", pendingJob.getId(), demandeId);
    }
}
