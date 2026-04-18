package bf.gov.faso.impression.service;

import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.dto.request.DeliveryRequest;
import bf.gov.faso.impression.dto.request.ReprintRequest;
import bf.gov.faso.impression.dto.response.DeliveryResponse;
import bf.gov.faso.impression.dto.response.PageResponse;
import bf.gov.faso.impression.dto.response.PrintJobResponse;
import bf.gov.faso.impression.dto.response.PrintStatisticsResponse;
import bf.gov.faso.impression.entity.PrintStatus;

import java.time.Instant;
import java.util.UUID;

/**
 * Service interface for print operations.
 *
 * All methods are tenant-aware and require the tenant ID from the security context.
 * Operations are restricted to users with OPERATEUR_IMPRESSION role.
 */
public interface ImpressionService {

    /**
     * Add a document to the print queue.
     *
     * @param request   The queue request
     * @param tenantId  The tenant ID
     * @return The created print job
     */
    PrintJobResponse addToQueue(AddToQueueRequest request, String tenantId);

    /**
     * Get the print queue (pending jobs).
     *
     * @param tenantId  The tenant ID
     * @param page      Page number
     * @param size      Page size
     * @return Paginated list of pending print jobs
     */
    PageResponse<PrintJobResponse> getQueue(String tenantId, int page, int size);

    /**
     * Get the next job from the queue.
     *
     * @param tenantId  The tenant ID
     * @return The next print job (or null if queue is empty)
     */
    PrintJobResponse getNextInQueue(String tenantId);

    /**
     * Print a document and finalize it (WORM lock).
     *
     * @param printJobId  The print job ID
     * @param operatorId  The operator performing the print
     * @param tenantId    The tenant ID
     * @return The finalized print job
     */
    PrintJobResponse printDocument(UUID printJobId, UUID operatorId, String tenantId);

    /**
     * Request a reprint of a document.
     *
     * @param request     The reprint request
     * @param operatorId  The operator requesting the reprint
     * @param tenantId    The tenant ID
     * @return The reprint job (or updated original job)
     */
    PrintJobResponse requestReprint(ReprintRequest request, UUID operatorId, String tenantId);

    /**
     * Authorize a reprint for a WORM-locked document.
     *
     * @param printJobId   The print job ID
     * @param authorizedBy The manager/admin authorizing the reprint
     * @param tenantId     The tenant ID
     * @return The updated print job
     */
    PrintJobResponse authorizeReprint(UUID printJobId, UUID authorizedBy, String tenantId);

    /**
     * Deliver a printed document to the client.
     *
     * @param request     The delivery request
     * @param operatorId  The operator performing the delivery
     * @param tenantId    The tenant ID
     * @return The delivery confirmation
     */
    DeliveryResponse deliverDocument(DeliveryRequest request, UUID operatorId, String tenantId);

    /**
     * Get print job by ID.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return The print job
     */
    PrintJobResponse getPrintJob(UUID printJobId, String tenantId);

    /**
     * Get print job status.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return The print status
     */
    PrintStatus getPrintStatus(UUID printJobId, String tenantId);

    /**
     * Get all print jobs for a document.
     *
     * @param documentId  The document ID
     * @param tenantId    The tenant ID
     * @param page        Page number
     * @param size        Page size
     * @return Paginated list of print jobs
     */
    PageResponse<PrintJobResponse> getPrintJobsByDocument(UUID documentId, String tenantId, int page, int size);

    /**
     * Get all print jobs for a demande.
     *
     * @param demandeId   The demande ID
     * @param tenantId    The tenant ID
     * @param page        Page number
     * @param size        Page size
     * @return Paginated list of print jobs
     */
    PageResponse<PrintJobResponse> getPrintJobsByDemande(UUID demandeId, String tenantId, int page, int size);

    /**
     * Get all print jobs by status.
     *
     * @param status    The print status
     * @param tenantId  The tenant ID
     * @param page      Page number
     * @param size      Page size
     * @return Paginated list of print jobs
     */
    PageResponse<PrintJobResponse> getPrintJobsByStatus(PrintStatus status, String tenantId, int page, int size);

    /**
     * Get print jobs pending delivery.
     *
     * @param tenantId  The tenant ID
     * @param page      Page number
     * @param size      Page size
     * @return Paginated list of print jobs ready for delivery
     */
    PageResponse<PrintJobResponse> getPendingDeliveries(String tenantId, int page, int size);

    /**
     * Get print jobs by date range.
     *
     * @param tenantId   The tenant ID
     * @param startDate  Start date
     * @param endDate    End date
     * @param page       Page number
     * @param size       Page size
     * @return Paginated list of print jobs
     */
    PageResponse<PrintJobResponse> getPrintJobsByDateRange(
        String tenantId, Instant startDate, Instant endDate, int page, int size);

    /**
     * Cancel a print job.
     *
     * @param printJobId  The print job ID
     * @param operatorId  The operator cancelling the job
     * @param tenantId    The tenant ID
     */
    void cancelPrintJob(UUID printJobId, UUID operatorId, String tenantId);

    /**
     * Get print statistics.
     *
     * @param tenantId  The tenant ID
     * @return Print statistics
     */
    PrintStatisticsResponse getStatistics(String tenantId);

    /**
     * Get the PDF bytes for a printed document.
     *
     * @param printJobId  The print job ID
     * @param tenantId    The tenant ID
     * @return The PDF content as byte array
     */
    byte[] getPrintJobPdf(UUID printJobId, String tenantId);

    /**
     * Get the latest PDF for a demande (most recent printed/WORM-locked job).
     *
     * @param demandeId  The demande ID
     * @param tenantId   The tenant ID
     * @return The PDF content as byte array
     */
    byte[] getLatestPdfByDemande(UUID demandeId, String tenantId);
}
