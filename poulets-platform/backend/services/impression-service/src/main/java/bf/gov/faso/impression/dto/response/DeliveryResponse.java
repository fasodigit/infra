package bf.gov.faso.impression.dto.response;

import bf.gov.faso.impression.entity.DeliveryMethod;
import bf.gov.faso.impression.entity.DeliveryRecord;

import java.time.Instant;
import java.util.Map;
import java.util.UUID;

/**
 * DTO Record for delivery response.
 */
public record DeliveryResponse(
    UUID id,
    UUID printJobId,
    UUID documentId,
    String tenantId,
    UUID clientId,
    UUID operatorId,
    DeliveryMethod deliveryMethod,
    String recipientName,
    String recipientIdNumber,
    String recipientIdType,
    String recipientPhone,
    String recipientEmail,
    String recipientRelationship,
    boolean hasSignature,
    String signatureHash,
    String deliveryLocation,
    String trackingNumber,
    String courierName,
    Map<String, Object> metadata,
    String notes,
    Instant deliveredAt
) {
    /**
     * Factory method to create a response from entity.
     */
    public static DeliveryResponse fromEntity(DeliveryRecord record) {
        return new DeliveryResponse(
            record.getId(),
            record.getPrintJobId(),
            record.getDocumentId(),
            record.getTenantId(),
            record.getClientId(),
            record.getOperatorId(),
            record.getDeliveryMethod(),
            record.getRecipientName(),
            record.getRecipientIdNumber(),
            record.getRecipientIdType(),
            record.getRecipientPhone(),
            record.getRecipientEmail(),
            record.getRecipientRelationship(),
            record.getSignatureData() != null && !record.getSignatureData().isEmpty(),
            record.getSignatureHash(),
            record.getDeliveryLocation(),
            record.getTrackingNumber(),
            record.getCourierName(),
            record.getMetadata(),
            record.getNotes(),
            record.getDeliveredAt()
        );
    }
}
