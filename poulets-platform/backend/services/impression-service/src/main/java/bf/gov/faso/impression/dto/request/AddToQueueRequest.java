package bf.gov.faso.impression.dto.request;

import jakarta.validation.constraints.Max;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;

import java.util.Map;
import java.util.UUID;

/**
 * DTO for adding a document to the print queue.
 */
public record AddToQueueRequest(
    @NotNull(message = "Document ID is required")
    UUID documentId,

    @NotNull(message = "Demande ID is required")
    UUID demandeId,

    @NotNull(message = "Client ID is required")
    UUID clientId,

    @NotBlank(message = "Document type is required")
    String documentType,

    String documentReference,

    @Min(value = 1, message = "Priority must be between 1 and 10")
    @Max(value = 10, message = "Priority must be between 1 and 10")
    int priority,

    @Min(value = 1, message = "Copies count must be at least 1")
    @Max(value = 10, message = "Copies count cannot exceed 10")
    int copiesCount,

    String pdfStoragePath,

    String notes,

    Map<String, Object> metadata,

    /** Code de verification HMAC-signe provenant de validation-acte-service */
    String qrVerificationCode,

    /** URL complete de verification publique */
    String verificationUrl
) {
    public AddToQueueRequest {
        if (priority == 0) {
            priority = 5;
        }
        if (copiesCount == 0) {
            copiesCount = 1;
        }
    }
}
