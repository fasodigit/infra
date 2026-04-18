package bf.gov.faso.impression.controller;

import bf.gov.shared.transfer.download.RangeRequestHandler;
import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.dto.request.DeliveryRequest;
import bf.gov.faso.impression.dto.request.ReprintRequest;
import bf.gov.faso.impression.dto.response.*;
import bf.gov.faso.impression.entity.PrintStatus;
import bf.gov.faso.impression.security.JwtUser;
import bf.gov.faso.impression.service.BlockchainService;
import bf.gov.faso.impression.service.ImpressionService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.Parameter;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.security.SecurityRequirement;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.web.bind.annotation.*;

import java.time.Instant;
import java.util.UUID;

/**
 * REST controller for impression operations.
 *
 * All endpoints require OPERATEUR_IMPRESSION role.
 */
@RestController
@RequestMapping("/api/v1/impression")
@Tag(name = "Impression", description = "Document printing and delivery operations")
@SecurityRequirement(name = "bearerAuth")
public class ImpressionController {

    private static final Logger log = LoggerFactory.getLogger(ImpressionController.class);

    private final ImpressionService impressionService;
    private final BlockchainService blockchainService;
    private final RangeRequestHandler rangeRequestHandler;

    public ImpressionController(
            ImpressionService impressionService,
            BlockchainService blockchainService,
            RangeRequestHandler rangeRequestHandler) {
        this.impressionService = impressionService;
        this.blockchainService = blockchainService;
        this.rangeRequestHandler = rangeRequestHandler;
    }

    @GetMapping("/queue")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION')")
    @Operation(
        summary = "Get print queue",
        description = "List all documents pending printing ordered by priority"
    )
    @ApiResponse(responseCode = "200", description = "Print queue retrieved")
    public ResponseEntity<PageResponse<PrintJobResponse>> getQueue(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {

        log.info("Getting print queue for tenant {}", user.getTenantId());
        return ResponseEntity.ok(impressionService.getQueue(user.getTenantId(), page, size));
    }

    @PostMapping("/queue")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('OPERATEUR_VALIDATION')")
    @Operation(
        summary = "Add to print queue",
        description = "Add a validated document to the print queue"
    )
    @ApiResponse(responseCode = "201", description = "Document added to queue")
    public ResponseEntity<PrintJobResponse> addToQueue(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @Valid @RequestBody AddToQueueRequest request) {

        log.info("Adding document {} to print queue", request.documentId());
        PrintJobResponse response = impressionService.addToQueue(request, user.getTenantId());
        return ResponseEntity.status(HttpStatus.CREATED).body(response);
    }

    @PostMapping("/{printJobId}/print")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION')")
    @Operation(
        summary = "Print document",
        description = "Mark document as printed and finalize with WORM lock"
    )
    @ApiResponse(responseCode = "200", description = "Document printed successfully")
    @ApiResponse(responseCode = "409", description = "Cannot print - invalid state or WORM violation")
    public ResponseEntity<PrintJobResponse> printDocument(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId) {

        log.info("Printing document for job {} by operator {}", printJobId, user.getUserId());
        PrintJobResponse response = impressionService.printDocument(
            printJobId, user.getUserId(), user.getTenantId());
        return ResponseEntity.ok(response);
    }

    @PostMapping("/{printJobId}/reprint")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION')")
    @Operation(
        summary = "Request reprint",
        description = "Request a reprint of a printed document (requires justification)"
    )
    @ApiResponse(responseCode = "200", description = "Reprint requested")
    @ApiResponse(responseCode = "403", description = "Reprint not authorized for WORM-locked document")
    public ResponseEntity<PrintJobResponse> requestReprint(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId,
            @Valid @RequestBody ReprintRequest request) {

        log.info("Reprint requested for job {} by operator {}", printJobId, user.getUserId());

        // Ensure the path variable matches the request body
        ReprintRequest fullRequest = new ReprintRequest(
            printJobId,
            request.reason(),
            request.copiesCount(),
            request.authorizedBy(),
            request.authorizationReference(),
            request.notes()
        );

        PrintJobResponse response = impressionService.requestReprint(
            fullRequest, user.getUserId(), user.getTenantId());
        return ResponseEntity.ok(response);
    }

    @PostMapping("/{printJobId}/authorize-reprint")
    @PreAuthorize("hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Authorize reprint",
        description = "Authorize a reprint for a WORM-locked document (manager/admin only)"
    )
    @ApiResponse(responseCode = "200", description = "Reprint authorized")
    public ResponseEntity<PrintJobResponse> authorizeReprint(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId) {

        log.info("Reprint authorized for job {} by {}", printJobId, user.getUserId());
        PrintJobResponse response = impressionService.authorizeReprint(
            printJobId, user.getUserId(), user.getTenantId());
        return ResponseEntity.ok(response);
    }

    @GetMapping("/{printJobId}/pdf")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Download printed PDF",
        description = "Download the PDF of a printed document from WORM storage. Supports HTTP Range requests for resumable downloads."
    )
    @ApiResponse(responseCode = "200", description = "PDF retrieved")
    @ApiResponse(responseCode = "206", description = "Partial content (Range request)")
    @ApiResponse(responseCode = "409", description = "Document not yet printed")
    public ResponseEntity<byte[]> downloadPdf(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId,
            @RequestHeader(value = HttpHeaders.RANGE, required = false) String rangeHeader) {

        log.info("Downloading PDF for job {} by {}", printJobId, user.getUserId());
        byte[] pdfBytes = impressionService.getPrintJobPdf(printJobId, user.getTenantId());
        return rangeRequestHandler.handleRequest(pdfBytes, printJobId + ".pdf", MediaType.APPLICATION_PDF, rangeHeader);
    }

    @GetMapping("/by-demande/{demandeId}/pdf")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER') or hasRole('ADMIN') or hasRole('CITOYEN')")
    @Operation(
        summary = "Download PDF by demande",
        description = "Download the latest printed PDF for a demande. Supports HTTP Range requests for resumable downloads."
    )
    @ApiResponse(responseCode = "200", description = "PDF retrieved")
    @ApiResponse(responseCode = "206", description = "Partial content (Range request)")
    @ApiResponse(responseCode = "404", description = "No printed document found for this demande")
    public ResponseEntity<byte[]> downloadPdfByDemande(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID demandeId,
            @RequestHeader(value = HttpHeaders.RANGE, required = false) String rangeHeader) {

        log.info("Downloading PDF for demande {} by {}", demandeId, user.getUserId());
        byte[] pdfBytes = impressionService.getLatestPdfByDemande(demandeId, user.getTenantId());
        return rangeRequestHandler.handleRequest(pdfBytes, "document-" + demandeId + ".pdf", MediaType.APPLICATION_PDF, rangeHeader);
    }

    @GetMapping("/{printJobId}/status")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Get print status",
        description = "Get the current status of a print job"
    )
    @ApiResponse(responseCode = "200", description = "Status retrieved")
    public ResponseEntity<PrintStatus> getStatus(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId) {

        PrintStatus status = impressionService.getPrintStatus(printJobId, user.getTenantId());
        return ResponseEntity.ok(status);
    }

    @GetMapping("/{printJobId}")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Get print job details",
        description = "Get full details of a print job"
    )
    @ApiResponse(responseCode = "200", description = "Print job details retrieved")
    public ResponseEntity<PrintJobResponse> getPrintJob(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId) {

        PrintJobResponse response = impressionService.getPrintJob(printJobId, user.getTenantId());
        return ResponseEntity.ok(response);
    }

    @PostMapping("/{printJobId}/deliver")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION')")
    @Operation(
        summary = "Deliver document",
        description = "Mark document as delivered to the client"
    )
    @ApiResponse(responseCode = "200", description = "Document delivered")
    public ResponseEntity<DeliveryResponse> deliverDocument(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId,
            @Valid @RequestBody DeliveryRequest request) {

        log.info("Delivering document for job {} by operator {}", printJobId, user.getUserId());

        // Ensure the path variable matches the request body
        DeliveryRequest fullRequest = new DeliveryRequest(
            printJobId,
            request.deliveryMethod(),
            request.recipientName(),
            request.recipientIdNumber(),
            request.recipientIdType(),
            request.recipientPhone(),
            request.recipientEmail(),
            request.recipientRelationship(),
            request.signatureData(),
            request.deliveryLocation(),
            request.trackingNumber(),
            request.courierName(),
            request.notes(),
            request.metadata()
        );

        DeliveryResponse response = impressionService.deliverDocument(
            fullRequest, user.getUserId(), user.getTenantId());
        return ResponseEntity.ok(response);
    }

    @GetMapping("/pending-delivery")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION')")
    @Operation(
        summary = "Get pending deliveries",
        description = "List all printed documents pending delivery"
    )
    @ApiResponse(responseCode = "200", description = "Pending deliveries retrieved")
    public ResponseEntity<PageResponse<PrintJobResponse>> getPendingDeliveries(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {

        return ResponseEntity.ok(impressionService.getPendingDeliveries(user.getTenantId(), page, size));
    }

    @DeleteMapping("/{printJobId}")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER')")
    @Operation(
        summary = "Cancel print job",
        description = "Cancel a print job (not allowed for WORM-locked documents)"
    )
    @ApiResponse(responseCode = "204", description = "Print job cancelled")
    @ApiResponse(responseCode = "409", description = "Cannot cancel - WORM locked or already delivered")
    public ResponseEntity<Void> cancelPrintJob(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID printJobId) {

        log.info("Cancelling print job {} by {}", printJobId, user.getUserId());
        impressionService.cancelPrintJob(printJobId, user.getUserId(), user.getTenantId());
        return ResponseEntity.noContent().build();
    }

    @GetMapping("/blockchain/{blockHash}")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Verify blockchain hash",
        description = "Verify a document's blockchain entry"
    )
    @ApiResponse(responseCode = "200", description = "Blockchain verification result")
    public ResponseEntity<BlockchainVerificationResponse> verifyBlockchain(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable String blockHash) {

        log.info("Verifying blockchain hash {} for tenant {}", blockHash, user.getTenantId());
        BlockchainVerificationResponse response = blockchainService.verifyBlock(blockHash, user.getTenantId());
        return ResponseEntity.ok(response);
    }

    @GetMapping("/blockchain/verify-chain")
    @PreAuthorize("hasRole('ADMIN')")
    @Operation(
        summary = "Verify full blockchain",
        description = "Verify the integrity of the entire blockchain for the tenant"
    )
    @ApiResponse(responseCode = "200", description = "Chain integrity verification result")
    public ResponseEntity<Boolean> verifyChain(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user) {

        log.info("Verifying full blockchain for tenant {}", user.getTenantId());
        boolean valid = blockchainService.verifyChainIntegrity(user.getTenantId());
        return ResponseEntity.ok(valid);
    }

    @GetMapping("/statistics")
    @PreAuthorize("hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Get print statistics",
        description = "Get printing statistics for the tenant"
    )
    @ApiResponse(responseCode = "200", description = "Statistics retrieved")
    public ResponseEntity<PrintStatisticsResponse> getStatistics(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user) {

        PrintStatisticsResponse stats = impressionService.getStatistics(user.getTenantId());
        return ResponseEntity.ok(stats);
    }

    @GetMapping("/by-status/{status}")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER')")
    @Operation(
        summary = "Get jobs by status",
        description = "List print jobs filtered by status"
    )
    @ApiResponse(responseCode = "200", description = "Print jobs retrieved")
    public ResponseEntity<PageResponse<PrintJobResponse>> getByStatus(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable PrintStatus status,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {

        return ResponseEntity.ok(impressionService.getPrintJobsByStatus(status, user.getTenantId(), page, size));
    }

    @GetMapping("/by-date-range")
    @PreAuthorize("hasRole('MANAGER') or hasRole('ADMIN')")
    @Operation(
        summary = "Get jobs by date range",
        description = "List print jobs within a date range"
    )
    @ApiResponse(responseCode = "200", description = "Print jobs retrieved")
    public ResponseEntity<PageResponse<PrintJobResponse>> getByDateRange(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @RequestParam Instant startDate,
            @RequestParam Instant endDate,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {

        return ResponseEntity.ok(impressionService.getPrintJobsByDateRange(
            user.getTenantId(), startDate, endDate, page, size));
    }

    @GetMapping("/by-document/{documentId}")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER')")
    @Operation(
        summary = "Get jobs by document",
        description = "List all print jobs for a specific document"
    )
    @ApiResponse(responseCode = "200", description = "Print jobs retrieved")
    public ResponseEntity<PageResponse<PrintJobResponse>> getByDocument(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID documentId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {

        return ResponseEntity.ok(impressionService.getPrintJobsByDocument(
            documentId, user.getTenantId(), page, size));
    }

    @GetMapping("/by-demande/{demandeId}")
    @PreAuthorize("hasRole('OPERATEUR_IMPRESSION') or hasRole('MANAGER')")
    @Operation(
        summary = "Get jobs by demande",
        description = "List all print jobs for a specific demande"
    )
    @ApiResponse(responseCode = "200", description = "Print jobs retrieved")
    public ResponseEntity<PageResponse<PrintJobResponse>> getByDemande(
            @Parameter(hidden = true) @AuthenticationPrincipal JwtUser user,
            @PathVariable UUID demandeId,
            @RequestParam(defaultValue = "0") int page,
            @RequestParam(defaultValue = "20") int size) {

        return ResponseEntity.ok(impressionService.getPrintJobsByDemande(
            demandeId, user.getTenantId(), page, size));
    }

}
