package bf.gov.faso.impression.dto.request;

import jakarta.validation.constraints.Max;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotNull;

import java.util.Map;
import java.util.UUID;

/**
 * DTO for print job requests.
 */
public record PrintRequest(
    @NotNull(message = "Document ID is required")
    UUID documentId,

    @NotNull(message = "Demande ID is required")
    UUID demandeId,

    @Min(value = 1, message = "Copies count must be at least 1")
    @Max(value = 10, message = "Copies count cannot exceed 10")
    int copiesCount,

    @Min(value = 1, message = "Priority must be between 1 and 10")
    @Max(value = 10, message = "Priority must be between 1 and 10")
    int priority,

    String notes,

    Map<String, Object> metadata
) {
    public PrintRequest {
        if (copiesCount == 0) {
            copiesCount = 1;
        }
        if (priority == 0) {
            priority = 5;
        }
    }
}
