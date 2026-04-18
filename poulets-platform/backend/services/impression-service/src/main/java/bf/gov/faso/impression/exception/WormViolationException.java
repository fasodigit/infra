package bf.gov.faso.impression.exception;

import java.util.UUID;

/**
 * Exception thrown when an operation violates WORM (Write Once Read Many) constraints.
 */
public class WormViolationException extends RuntimeException {

    private final UUID documentId;
    private final String operation;

    public WormViolationException(String message) {
        super(message);
        this.documentId = null;
        this.operation = null;
    }

    public WormViolationException(String message, UUID documentId, String operation) {
        super(String.format("%s - Document: %s, Operation: %s", message, documentId, operation));
        this.documentId = documentId;
        this.operation = operation;
    }

    public UUID getDocumentId() {
        return documentId;
    }

    public String getOperation() {
        return operation;
    }
}
