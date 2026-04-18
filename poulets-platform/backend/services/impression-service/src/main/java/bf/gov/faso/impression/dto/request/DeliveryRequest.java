package bf.gov.faso.impression.dto.request;

import bf.gov.faso.impression.entity.DeliveryMethod;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

import java.util.Map;
import java.util.UUID;

/**
 * DTO for document delivery requests.
 */
public record DeliveryRequest(
    @NotNull(message = "Print job ID is required")
    UUID printJobId,

    @NotNull(message = "Delivery method is required")
    DeliveryMethod deliveryMethod,

    @NotBlank(message = "Recipient name is required")
    @Size(max = 255, message = "Recipient name cannot exceed 255 characters")
    String recipientName,

    @Size(max = 50, message = "Recipient ID number cannot exceed 50 characters")
    String recipientIdNumber,

    @Size(max = 50, message = "Recipient ID type cannot exceed 50 characters")
    String recipientIdType,

    @Size(max = 20, message = "Recipient phone cannot exceed 20 characters")
    String recipientPhone,

    @Size(max = 255, message = "Recipient email cannot exceed 255 characters")
    String recipientEmail,

    @Size(max = 100, message = "Recipient relationship cannot exceed 100 characters")
    String recipientRelationship,

    String signatureData,

    @Size(max = 500, message = "Delivery location cannot exceed 500 characters")
    String deliveryLocation,

    @Size(max = 100, message = "Tracking number cannot exceed 100 characters")
    String trackingNumber,

    @Size(max = 100, message = "Courier name cannot exceed 100 characters")
    String courierName,

    @Size(max = 2000, message = "Notes cannot exceed 2000 characters")
    String notes,

    Map<String, Object> metadata
) {}
