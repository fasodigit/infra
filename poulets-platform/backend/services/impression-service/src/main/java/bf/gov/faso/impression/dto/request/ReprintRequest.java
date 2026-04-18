package bf.gov.faso.impression.dto.request;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Size;

import java.util.UUID;

/**
 * DTO for reprint requests.
 *
 * Reprints require a justification and may need authorization if the
 * document is WORM-locked.
 */
public record ReprintRequest(
    @NotNull(message = "Print job ID is required")
    UUID printJobId,

    @NotBlank(message = "Reason for reprint is required")
    @Size(min = 10, max = 500, message = "Reason must be between 10 and 500 characters")
    String reason,

    int copiesCount,

    UUID authorizedBy,

    String authorizationReference,

    String notes
) {
    public ReprintRequest {
        if (copiesCount == 0) {
            copiesCount = 1;
        }
    }
}
