package bf.gov.faso.impression.exception;

import java.util.UUID;

/**
 * Exception thrown when a reprint is requested without proper authorization.
 */
public class ReprintNotAuthorizedException extends RuntimeException {

    private final UUID printJobId;
    private final boolean wormLocked;

    public ReprintNotAuthorizedException(String message) {
        super(message);
        this.printJobId = null;
        this.wormLocked = false;
    }

    public ReprintNotAuthorizedException(UUID printJobId, boolean wormLocked) {
        super(String.format("Reprint not authorized for print job %s%s",
            printJobId, wormLocked ? " (WORM-locked document)" : ""));
        this.printJobId = printJobId;
        this.wormLocked = wormLocked;
    }

    public UUID getPrintJobId() {
        return printJobId;
    }

    public boolean isWormLocked() {
        return wormLocked;
    }
}
