package bf.gov.faso.impression.exception;

import bf.gov.faso.impression.entity.PrintStatus;

import java.util.UUID;

/**
 * Exception thrown when a print operation is attempted on a job with an invalid state.
 */
public class InvalidPrintStateException extends RuntimeException {

    private final UUID printJobId;
    private final PrintStatus currentStatus;
    private final PrintStatus expectedStatus;

    public InvalidPrintStateException(String message) {
        super(message);
        this.printJobId = null;
        this.currentStatus = null;
        this.expectedStatus = null;
    }

    public InvalidPrintStateException(UUID printJobId, PrintStatus currentStatus, PrintStatus expectedStatus) {
        super(String.format("Invalid print job state. Job: %s, Current: %s, Expected: %s",
            printJobId, currentStatus, expectedStatus));
        this.printJobId = printJobId;
        this.currentStatus = currentStatus;
        this.expectedStatus = expectedStatus;
    }

    public InvalidPrintStateException(UUID printJobId, PrintStatus currentStatus, String operation) {
        super(String.format("Cannot perform '%s' on print job %s with status %s",
            operation, printJobId, currentStatus));
        this.printJobId = printJobId;
        this.currentStatus = currentStatus;
        this.expectedStatus = null;
    }

    public UUID getPrintJobId() {
        return printJobId;
    }

    public PrintStatus getCurrentStatus() {
        return currentStatus;
    }

    public PrintStatus getExpectedStatus() {
        return expectedStatus;
    }
}
