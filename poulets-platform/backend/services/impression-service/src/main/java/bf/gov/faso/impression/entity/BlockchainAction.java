package bf.gov.faso.impression.entity;

/**
 * Enumeration of actions recorded in the blockchain audit trail.
 */
public enum BlockchainAction {

    /**
     * Genesis block for a new tenant chain.
     */
    GENESIS,

    /**
     * Document was printed.
     */
    PRINT,

    /**
     * Document was delivered to the client.
     */
    DELIVER,

    /**
     * Reprint was requested.
     */
    REPRINT_REQUEST,

    /**
     * Reprint was authorized.
     */
    REPRINT_AUTHORIZED,

    /**
     * Reprint was executed.
     */
    REPRINT_EXECUTED,

    /**
     * Document was locked in WORM storage.
     */
    WORM_LOCK,

    /**
     * Print job was cancelled.
     */
    CANCEL,

    /**
     * Document integrity was verified.
     */
    VERIFY,

    /**
     * Document was accessed for viewing.
     */
    VIEW,

    /**
     * Document was downloaded.
     */
    DOWNLOAD
}
