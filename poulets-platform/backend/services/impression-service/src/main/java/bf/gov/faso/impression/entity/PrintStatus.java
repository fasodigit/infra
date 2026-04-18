package bf.gov.faso.impression.entity;

/**
 * Enumeration representing the status of a print job.
 */
public enum PrintStatus {

    /**
     * Document is queued and waiting to be printed.
     */
    EN_ATTENTE,

    /**
     * Document is currently being processed for printing.
     */
    EN_COURS,

    /**
     * Document has been printed successfully.
     */
    IMPRIME,

    /**
     * Document has been delivered to the client.
     */
    DELIVRE,

    /**
     * Print job was cancelled.
     */
    ANNULE,

    /**
     * Print job failed due to an error.
     */
    ERREUR,

    /**
     * Reprint has been requested.
     */
    REPRINT_DEMANDE,

    /**
     * Document is locked in WORM storage (cannot be reprinted without authorization).
     */
    VERROUILLE_WORM
}
