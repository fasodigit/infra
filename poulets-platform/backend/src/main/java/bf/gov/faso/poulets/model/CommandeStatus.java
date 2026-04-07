package bf.gov.faso.poulets.model;

/**
 * Order lifecycle status.
 */
public enum CommandeStatus {
    PENDING,
    CONFIRMED,
    PREPARING,
    READY,
    DELIVERED,
    CANCELLED
}
