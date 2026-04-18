package bf.gov.faso.impression.dto.response;

import java.util.Map;

/**
 * DTO for print statistics.
 */
public record PrintStatisticsResponse(
    long totalJobs,
    long enAttente,
    long enCours,
    long imprime,
    long delivre,
    long annule,
    long erreur,
    long reprintDemande,
    long wormLocked,
    long totalCopiesPrinted,
    long totalDeliveries,
    Map<String, Long> byDocumentType,
    Map<String, Long> byDeliveryMethod,
    double averageQueueTimeMinutes,
    double averagePrintToDeliveryTimeMinutes
) {}
