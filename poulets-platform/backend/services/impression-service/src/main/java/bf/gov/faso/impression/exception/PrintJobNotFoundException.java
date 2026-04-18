package bf.gov.faso.impression.exception;

import java.util.UUID;

/**
 * Exception thrown when a print job is not found.
 */
public class PrintJobNotFoundException extends RuntimeException {

    private final UUID printJobId;

    public PrintJobNotFoundException(UUID printJobId) {
        super("Print job not found: " + printJobId);
        this.printJobId = printJobId;
    }

    public PrintJobNotFoundException(String message, UUID printJobId) {
        super(message);
        this.printJobId = printJobId;
    }

    public UUID getPrintJobId() {
        return printJobId;
    }
}
