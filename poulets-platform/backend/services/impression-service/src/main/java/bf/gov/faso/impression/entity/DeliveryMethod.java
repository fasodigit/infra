package bf.gov.faso.impression.entity;

/**
 * Enumeration of document delivery methods.
 */
public enum DeliveryMethod {

    /**
     * In-person pickup at the office.
     */
    GUICHET,

    /**
     * Postal delivery.
     */
    COURRIER,

    /**
     * Courier delivery service.
     */
    COURSIER,

    /**
     * Electronic delivery (email).
     */
    ELECTRONIQUE,

    /**
     * Delivery to another administrative office.
     */
    INTER_ADMINISTRATION
}
