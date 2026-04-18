package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.cache.PrintJobCacheDTO;
import bf.gov.faso.impression.cache.PrintJobSearchCacheService;
import bf.gov.faso.impression.client.VerificationGrpcClient;
import bf.gov.shared.eventbus.publish.EventPublisher;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.RedisTemplate;
import bf.gov.faso.impression.crypto.FileEncryptionService;
import bf.gov.faso.impression.dto.request.AddToQueueRequest;
import bf.gov.faso.impression.dto.request.DeliveryRequest;
import bf.gov.faso.impression.dto.request.ReprintRequest;
import bf.gov.faso.impression.dto.response.DeliveryResponse;
import bf.gov.faso.impression.dto.response.PageResponse;
import bf.gov.faso.impression.dto.response.PrintJobResponse;
import bf.gov.faso.impression.dto.response.PrintStatisticsResponse;
import bf.gov.faso.impression.entity.*;
import bf.gov.faso.impression.exception.*;
import bf.gov.faso.impression.kafka.PrintEventProducer;
import bf.gov.faso.impression.repository.DeliveryRecordRepository;
import bf.gov.faso.impression.repository.PrintJobRepository;
import bf.gov.faso.impression.service.*;
import org.apache.commons.codec.digest.DigestUtils;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Pageable;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.*;
import java.util.stream.Collectors;

/**
 * Implementation of the impression service.
 *
 * NOTE: No class-level @Transactional — each method declares its own transaction scope
 * to avoid holding DB connections during external I/O (gRPC, file ops, WORM, cache).
 */
@Service
public class ImpressionServiceImpl implements ImpressionService {

    private static final Logger log = LoggerFactory.getLogger(ImpressionServiceImpl.class);

    @Value("${ec.cache.write-behind.enabled:false}")
    private boolean writeBehindEnabled;

    private final PrintJobRepository printJobRepository;
    private final DeliveryRecordRepository deliveryRecordRepository;
    private final WormStorageService wormStorageService;
    private final BlockchainService blockchainService;
    private final PrintQueueService printQueueService;
    private final PdfGenerationService pdfGenerationService;
    private final PrintEventProducer printEventProducer;
    private final EventPublisher eventPublisher;
    private final RedisTemplate<String, String> redisTemplate;
    private final FileEncryptionService fileEncryptionService;
    private final VerificationGrpcClient verificationGrpcClient;
    private final PdfArchiveService pdfArchiveService;
    private final PrintJobTransactionalHelper txHelper;
    private final PrintJobSearchCacheService printJobSearchCacheService;

    public ImpressionServiceImpl(
            PrintJobRepository printJobRepository,
            DeliveryRecordRepository deliveryRecordRepository,
            WormStorageService wormStorageService,
            BlockchainService blockchainService,
            PrintQueueService printQueueService,
            PdfGenerationService pdfGenerationService,
            PrintEventProducer printEventProducer,
            EventPublisher eventPublisher,
            RedisTemplate<String, String> redisTemplate,
            FileEncryptionService fileEncryptionService,
            VerificationGrpcClient verificationGrpcClient,
            PdfArchiveService pdfArchiveService,
            PrintJobTransactionalHelper txHelper,
            PrintJobSearchCacheService printJobSearchCacheService) {
        this.printJobRepository = printJobRepository;
        this.deliveryRecordRepository = deliveryRecordRepository;
        this.wormStorageService = wormStorageService;
        this.blockchainService = blockchainService;
        this.printQueueService = printQueueService;
        this.pdfGenerationService = pdfGenerationService;
        this.printEventProducer = printEventProducer;
        this.eventPublisher = eventPublisher;
        this.redisTemplate = redisTemplate;
        this.fileEncryptionService = fileEncryptionService;
        this.verificationGrpcClient = verificationGrpcClient;
        this.pdfArchiveService = pdfArchiveService;
        this.txHelper = txHelper;
        this.printJobSearchCacheService = printJobSearchCacheService;
    }

    @Override
    @Transactional
    public PrintJobResponse addToQueue(AddToQueueRequest request, String tenantId) {
        log.info("Adding document {} to print queue for tenant {}", request.documentId(), tenantId);

        // Check if document already has an active print job
        if (printJobRepository.existsActiveJobForDocument(request.documentId(), tenantId)) {
            throw new InvalidPrintStateException(
                "Document already has an active print job: " + request.documentId());
        }

        // Ensure blockchain is initialized for this tenant
        if (!blockchainService.isChainInitialized(tenantId)) {
            blockchainService.initializeChain(tenantId, request.clientId());
        }

        PrintJob job = new PrintJob(
            request.documentId(),
            request.demandeId(),
            tenantId,
            request.clientId(),
            request.documentType()
        );
        job.setDocumentReference(request.documentReference());
        job.setPriority(request.priority());
        job.setCopiesCount(request.copiesCount());
        job.setPdfStoragePath(request.pdfStoragePath());
        job.setNotes(request.notes());
        if (request.metadata() != null) {
            job.setMetadata(filterMetadata(request.metadata()));
        }
        // Stocker le code de verification HMAC-signe provenant de validation-acte-service
        if (request.qrVerificationCode() != null) {
            job.setQrVerificationCode(request.qrVerificationCode());
        }
        if (request.verificationUrl() != null) {
            job.setVerificationUrl(request.verificationUrl());
        }
        job.setStatus(PrintStatus.EN_ATTENTE);

        job = printJobRepository.save(job);

        // Add to Redis queue
        printQueueService.addToQueue(job.getId(), tenantId, request.priority());

        // Write-behind: update cache for cache-first reads
        if (writeBehindEnabled) {
            var cacheDto = PrintJobCacheDTO.fromEntity(job);
            printJobSearchCacheService.updateFullState(cacheDto);
            printJobSearchCacheService.addToPendingFlush(job.getId().toString());
        }

        log.info("Print job {} created and added to queue", job.getId());

        return PrintJobResponse.fromEntity(job);
    }

    @Override
    @Transactional(readOnly = true)
    public PageResponse<PrintJobResponse> getQueue(String tenantId, int page, int size) {
        Pageable pageable = PageRequest.of(page, size);
        Page<PrintJob> jobs = printJobRepository.findPendingQueue(tenantId, pageable);
        return PageResponse.fromPage(jobs, PrintJobResponse::fromEntitySummary);
    }

    @Override
    @Transactional(readOnly = true)
    public PrintJobResponse getNextInQueue(String tenantId) {
        return printJobRepository.findNextInQueue(tenantId)
            .map(PrintJobResponse::fromEntity)
            .orElse(null);
    }

    @Override
    public PrintJobResponse printDocument(UUID printJobId, UUID operatorId, String tenantId) {
        log.info("Printing document for job {} by operator {}", printJobId, operatorId);

        // ── Phase 1 (short transaction): load job + mark EN_COURS ──
        PrintJob job = txHelper.markJobInProgress(printJobId, operatorId, tenantId);

        try {
            // ── Phase 2 (NO transaction): external I/O — PDF, gRPC, archive, WORM ──
            // These are the expensive operations that were blocking the DB connection.

            // Retrieve PDF from storage path (or generate if needed)
            byte[] pdfBytes = retrieveOrGeneratePdf(job);

            // Add watermark via document-security-ms gRPC
            String watermarkText = generateWatermarkText(job);
            pdfBytes = pdfGenerationService.addWatermarkViaGrpc(
                pdfBytes, watermarkText, job.getDocumentId(), tenantId);

            // Archive plaintext PDF (with QR + watermark) to DOCS-ETAT-CIVIL
            pdfArchiveService.archive(
                pdfBytes, job.getDemandeId(), job.getDocumentType(), job.getDocumentId(), job.getDocumentReference());

            // Calculate document hash (on plaintext PDF for integrity verification)
            String documentHash = pdfGenerationService.calculateHash(pdfBytes);

            // Encrypt PDF before storage (AES-256-GCM)
            byte[] storageBytes = fileEncryptionService.encrypt(pdfBytes);

            // Store encrypted PDF in WORM storage
            WormStorageService.WormStorageResult wormResult =
                wormStorageService.storeImmutable(job.getDocumentId(), storageBytes, tenantId);

            // Record in blockchain (has its own @Transactional)
            BlockchainEntry blockEntry = blockchainService.addEntry(
                job.getDocumentId(),
                job.getId(),
                documentHash,
                operatorId,
                tenantId,
                BlockchainAction.PRINT
            );

            // ── Phase 3 (short transaction): finalize job in DB ──
            job = txHelper.finalizeJobAfterPrint(
                printJobId, tenantId, operatorId, documentHash,
                blockEntry.getBlockHash(), wormResult);

            // Remove from Redis queue (outside transaction — cache operation)
            printQueueService.removeFromQueue(printJobId, tenantId);

            // Write-behind: update cache after print completion
            if (writeBehindEnabled) {
                var cacheDto = PrintJobCacheDTO.fromEntity(job);
                printJobSearchCacheService.updateFullState(cacheDto);
                printJobSearchCacheService.addToPendingFlush(job.getId().toString());
            }

            log.info("Document printed and WORM-locked for job {}", printJobId);

            // ── Phase 4 (fire-and-forget): notify downstream services ──
            notifyDownstreamAfterPrint(job, pdfBytes);

            return PrintJobResponse.fromEntity(job);

        } catch (Exception e) {
            log.error("Print failed for job {}", printJobId, e);
            txHelper.markJobFailed(printJobId, tenantId, e.getMessage());
            throw new RuntimeException("Print failed: " + e.getMessage(), e);
        }
    }

    /**
     * Fire-and-forget downstream notifications (DragonflyDB stream + event stream).
     * Failures here do NOT affect the print result.
     */
    private void notifyDownstreamAfterPrint(PrintJob job, byte[] pdfBytes) {
        try {
            // Mark ready for pickup via DragonflyDB stream -> triggers notify-ms email/SMS
            Map<String, String> statusPayload = new HashMap<>();
            statusPayload.put("demandeId", job.getDemandeId().toString());
            statusPayload.put("tenantId", job.getTenantId());
            statusPayload.put("newStatus", "PRET_RETRAIT");
            statusPayload.put("clientId", job.getClientId() != null ? job.getClientId().toString() : "");
            statusPayload.put("operatorId", "");
            statusPayload.put("comment", "");
            statusPayload.put("demandeRef", job.getDocumentReference() != null ? job.getDocumentReference() : "");
            if (job.getDocumentType() != null) {
                statusPayload.put("typeDocument", job.getDocumentType());
            }
            eventPublisher.publish("ec:demande.status-updated", job.getTenantId(), "STATUS_UPDATED", statusPayload);
            // Publish ACTE_IMPRIME to etatcivil.acte.imprime (for document-delivery-service)
            printEventProducer.publishActeImprime(job, pdfBytes);
        } catch (Exception ex) {
            log.error("Failed to publish notification for job {}", job.getId(), ex);
        }
    }

    @Override
    @Transactional
    public PrintJobResponse requestReprint(ReprintRequest request, UUID operatorId, String tenantId) {
        log.info("Reprint requested for job {} by operator {}", request.printJobId(), operatorId);

        PrintJob originalJob = printJobRepository.findByIdAndTenantId(request.printJobId(), tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(request.printJobId()));

        if (!originalJob.canReprint()) {
            throw new InvalidPrintStateException(request.printJobId(), originalJob.getStatus(), "reprint");
        }

        // For WORM-locked documents, check authorization
        if (originalJob.isWormLocked() && request.authorizedBy() == null) {
            // Mark as pending authorization
            originalJob.requestReprint(request.reason());
            printJobRepository.save(originalJob);

            // Record in blockchain
            blockchainService.addEntry(
                originalJob.getDocumentId(),
                originalJob.getId(),
                originalJob.getDocumentHash(),
                operatorId,
                tenantId,
                BlockchainAction.REPRINT_REQUEST,
                request.reason(),
                null,
                null
            );

            throw new ReprintNotAuthorizedException(request.printJobId(), true);
        }

        // Create a new print job for the reprint
        PrintJob reprintJob = new PrintJob(
            originalJob.getDocumentId(),
            originalJob.getDemandeId(),
            tenantId,
            originalJob.getClientId(),
            originalJob.getDocumentType()
        );
        reprintJob.setDocumentReference(originalJob.getDocumentReference());
        reprintJob.setPriority(Math.max(1, originalJob.getPriority() - 1)); // Higher priority
        reprintJob.setCopiesCount(request.copiesCount());
        reprintJob.setPdfStoragePath(originalJob.getPdfStoragePath());
        reprintJob.setOriginalPrintJobId(originalJob.getId());
        reprintJob.setReprintReason(request.reason());
        reprintJob.setReprintAuthorizedBy(request.authorizedBy());
        reprintJob.setNotes(request.notes());

        // Update original job reprint count
        originalJob.authorizeReprint(request.authorizedBy());
        printJobRepository.save(originalJob);

        reprintJob = printJobRepository.save(reprintJob);

        // Add to queue
        printQueueService.addToQueue(reprintJob.getId(), tenantId, reprintJob.getPriority());

        // Record in blockchain
        blockchainService.addEntry(
            reprintJob.getDocumentId(),
            reprintJob.getId(),
            originalJob.getDocumentHash(),
            operatorId,
            tenantId,
            BlockchainAction.REPRINT_AUTHORIZED,
            request.reason(),
            null,
            null
        );

        log.info("Reprint job {} created for original job {}", reprintJob.getId(), originalJob.getId());

        return PrintJobResponse.fromEntity(reprintJob);
    }

    @Override
    @Transactional
    public PrintJobResponse authorizeReprint(UUID printJobId, UUID authorizedBy, String tenantId) {
        log.info("Authorizing reprint for job {} by {}", printJobId, authorizedBy);

        PrintJob job = printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));

        if (job.getStatus() != PrintStatus.REPRINT_DEMANDE) {
            throw new InvalidPrintStateException(printJobId, job.getStatus(), PrintStatus.REPRINT_DEMANDE);
        }

        job.authorizeReprint(authorizedBy);
        job.setStatus(PrintStatus.EN_ATTENTE);
        job = printJobRepository.save(job);

        // Add back to queue with high priority
        printQueueService.addToQueue(job.getId(), tenantId, 1);

        // Record in blockchain
        blockchainService.addEntry(
            job.getDocumentId(),
            job.getId(),
            job.getDocumentHash(),
            authorizedBy,
            tenantId,
            BlockchainAction.REPRINT_AUTHORIZED
        );

        log.info("Reprint authorized for job {}", printJobId);

        return PrintJobResponse.fromEntity(job);
    }

    @Override
    @Transactional
    public DeliveryResponse deliverDocument(DeliveryRequest request, UUID operatorId, String tenantId) {
        log.info("Delivering document for job {} by operator {}", request.printJobId(), operatorId);

        PrintJob job = printJobRepository.findByIdAndTenantId(request.printJobId(), tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(request.printJobId()));

        if (job.getStatus() != PrintStatus.IMPRIME && job.getStatus() != PrintStatus.VERROUILLE_WORM) {
            throw new InvalidPrintStateException(request.printJobId(), job.getStatus(), "deliver");
        }

        // Calculate signature hash if provided
        String signatureHash = null;
        if (request.signatureData() != null && !request.signatureData().isEmpty()) {
            signatureHash = DigestUtils.sha256Hex(request.signatureData());
        }

        // Create delivery record
        DeliveryRecord record = new DeliveryRecord(
            job.getId(),
            job.getDocumentId(),
            tenantId,
            job.getClientId(),
            operatorId,
            request.deliveryMethod()
        );
        record.setRecipientName(request.recipientName());
        record.setRecipientIdNumber(request.recipientIdNumber());
        record.setRecipientIdType(request.recipientIdType());
        record.setRecipientPhone(request.recipientPhone());
        record.setRecipientEmail(request.recipientEmail());
        record.setRecipientRelationship(request.recipientRelationship());
        record.setSignatureData(request.signatureData());
        record.setSignatureHash(signatureHash);
        record.setDeliveryLocation(request.deliveryLocation());
        record.setTrackingNumber(request.trackingNumber());
        record.setCourierName(request.courierName());
        record.setNotes(request.notes());
        if (request.metadata() != null) {
            record.setMetadata(request.metadata());
        }

        record = deliveryRecordRepository.save(record);

        // Update print job
        job.markAsDelivered(request.recipientName(), request.deliveryMethod().name(), signatureHash);
        printJobRepository.save(job);

        // Record in blockchain
        blockchainService.addEntry(
            job.getDocumentId(),
            job.getId(),
            job.getDocumentHash(),
            operatorId,
            tenantId,
            BlockchainAction.DELIVER,
            "Delivered to: " + request.recipientName() + " via " + request.deliveryMethod(),
            null,
            null
        );

        // Write-behind: update cache after delivery
        if (writeBehindEnabled) {
            var cacheDto = PrintJobCacheDTO.fromEntity(job);
            printJobSearchCacheService.updateFullState(cacheDto);
            printJobSearchCacheService.addToPendingFlush(job.getId().toString());
        }

        log.info("Document delivered for job {}", request.printJobId());

        return DeliveryResponse.fromEntity(record);
    }

    @Override
    @Transactional(readOnly = true)
    public PrintJobResponse getPrintJob(UUID printJobId, String tenantId) {
        return printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .map(PrintJobResponse::fromEntity)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));
    }

    @Override
    @Transactional(readOnly = true)
    public PrintStatus getPrintStatus(UUID printJobId, String tenantId) {
        return printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .map(PrintJob::getStatus)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));
    }

    @Override
    @Transactional(readOnly = true)
    public PageResponse<PrintJobResponse> getPrintJobsByDocument(UUID documentId, String tenantId, int page, int size) {
        List<PrintJob> jobs = printJobRepository.findByDocumentIdAndTenantId(documentId, tenantId);
        // Manual pagination for list results
        int start = Math.min(page * size, jobs.size());
        int end = Math.min(start + size, jobs.size());
        List<PrintJobResponse> content = jobs.subList(start, end).stream()
            .map(PrintJobResponse::fromEntitySummary)
            .toList();

        return new PageResponse<>(
            content,
            page,
            size,
            jobs.size(),
            (int) Math.ceil((double) jobs.size() / size),
            page == 0,
            end >= jobs.size(),
            end < jobs.size(),
            page > 0
        );
    }

    @Override
    @Transactional(readOnly = true)
    public PageResponse<PrintJobResponse> getPrintJobsByDemande(UUID demandeId, String tenantId, int page, int size) {
        List<PrintJob> jobs = printJobRepository.findByDemandeIdAndTenantId(demandeId, tenantId);
        int start = Math.min(page * size, jobs.size());
        int end = Math.min(start + size, jobs.size());
        List<PrintJobResponse> content = jobs.subList(start, end).stream()
            .map(PrintJobResponse::fromEntitySummary)
            .toList();

        return new PageResponse<>(
            content,
            page,
            size,
            jobs.size(),
            (int) Math.ceil((double) jobs.size() / size),
            page == 0,
            end >= jobs.size(),
            end < jobs.size(),
            page > 0
        );
    }

    @Override
    @Transactional(readOnly = true)
    public PageResponse<PrintJobResponse> getPrintJobsByStatus(PrintStatus status, String tenantId, int page, int size) {
        Pageable pageable = PageRequest.of(page, size);
        Page<PrintJob> jobs = printJobRepository.findByStatusAndTenantIdOrderByPriorityAscCreatedAtAsc(
            status, tenantId, pageable);
        return PageResponse.fromPage(jobs, PrintJobResponse::fromEntitySummary);
    }

    @Override
    @Transactional(readOnly = true)
    public PageResponse<PrintJobResponse> getPendingDeliveries(String tenantId, int page, int size) {
        Pageable pageable = PageRequest.of(page, size);
        Page<PrintJob> jobs = printJobRepository.findPendingDelivery(tenantId, pageable);
        return PageResponse.fromPage(jobs, PrintJobResponse::fromEntitySummary);
    }

    @Override
    @Transactional(readOnly = true)
    public PageResponse<PrintJobResponse> getPrintJobsByDateRange(
            String tenantId, Instant startDate, Instant endDate, int page, int size) {
        Pageable pageable = PageRequest.of(page, size);
        Page<PrintJob> jobs = printJobRepository.findByTenantIdAndCreatedAtBetween(
            tenantId, startDate, endDate, pageable);
        return PageResponse.fromPage(jobs, PrintJobResponse::fromEntitySummary);
    }

    @Override
    @Transactional
    public void cancelPrintJob(UUID printJobId, UUID operatorId, String tenantId) {
        log.info("Cancelling print job {} by operator {}", printJobId, operatorId);

        PrintJob job = printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));

        if (job.isWormLocked()) {
            throw new WormViolationException(
                "Cannot cancel WORM-locked document", job.getDocumentId(), "cancel");
        }

        if (job.getStatus() == PrintStatus.DELIVRE) {
            throw new InvalidPrintStateException(printJobId, job.getStatus(), "cancel");
        }

        job.cancel();
        printJobRepository.save(job);

        // Remove from queue if present
        printQueueService.removeFromQueue(printJobId, tenantId);

        // Record in blockchain if document was ever processed
        if (job.getDocumentHash() != null) {
            blockchainService.addEntry(
                job.getDocumentId(),
                job.getId(),
                job.getDocumentHash(),
                operatorId,
                tenantId,
                BlockchainAction.CANCEL
            );
        }

        // Write-behind: update cache after cancellation
        if (writeBehindEnabled) {
            var cacheDto = PrintJobCacheDTO.fromEntity(job);
            printJobSearchCacheService.updateFullState(cacheDto);
            printJobSearchCacheService.addToPendingFlush(job.getId().toString());
        }

        log.info("Print job {} cancelled", printJobId);
    }

    @Override
    @Transactional(readOnly = true)
    public PrintStatisticsResponse getStatistics(String tenantId) {
        long totalJobs = printJobRepository.count();
        long enAttente = printJobRepository.countByStatusAndTenantId(PrintStatus.EN_ATTENTE, tenantId);
        long enCours = printJobRepository.countByStatusAndTenantId(PrintStatus.EN_COURS, tenantId);
        long imprime = printJobRepository.countByStatusAndTenantId(PrintStatus.IMPRIME, tenantId);
        long delivre = printJobRepository.countByStatusAndTenantId(PrintStatus.DELIVRE, tenantId);
        long annule = printJobRepository.countByStatusAndTenantId(PrintStatus.ANNULE, tenantId);
        long erreur = printJobRepository.countByStatusAndTenantId(PrintStatus.ERREUR, tenantId);
        long reprintDemande = printJobRepository.countByStatusAndTenantId(PrintStatus.REPRINT_DEMANDE, tenantId);
        long wormLocked = printJobRepository.countByStatusAndTenantId(PrintStatus.VERROUILLE_WORM, tenantId);

        long totalCopies = printJobRepository.sumCopiesPrintedByTenantId(tenantId);
        long totalDeliveries = deliveryRecordRepository.countByTenantId(tenantId);

        Map<String, Long> byDocumentType = printJobRepository.countByDocumentTypeAndTenantId(tenantId)
            .stream()
            .collect(Collectors.toMap(
                arr -> (String) arr[0],
                arr -> (Long) arr[1]
            ));

        Map<String, Long> byDeliveryMethod = deliveryRecordRepository.countByDeliveryMethodAndTenantId(tenantId)
            .stream()
            .collect(Collectors.toMap(
                arr -> ((DeliveryMethod) arr[0]).name(),
                arr -> (Long) arr[1]
            ));

        Double avgQueueTime = printJobRepository.calculateAverageQueueTime(tenantId);
        Double avgDeliveryTime = printJobRepository.calculateAveragePrintToDeliveryTime(tenantId);

        return new PrintStatisticsResponse(
            totalJobs,
            enAttente,
            enCours,
            imprime,
            delivre,
            annule,
            erreur,
            reprintDemande,
            wormLocked,
            totalCopies,
            totalDeliveries,
            byDocumentType,
            byDeliveryMethod,
            avgQueueTime != null ? avgQueueTime : 0.0,
            avgDeliveryTime != null ? avgDeliveryTime : 0.0
        );
    }

    @Override
    @Transactional(readOnly = true)
    public byte[] getPrintJobPdf(UUID printJobId, String tenantId) {
        PrintJob job = printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));

        // Try WORM storage first if available
        if (job.getWormBucket() != null && job.getWormObjectKey() != null) {
            byte[] encryptedBytes = wormStorageService.retrieveDocument(
                job.getWormBucket(), job.getWormObjectKey(), tenantId);
            if (encryptedBytes != null && encryptedBytes.length > 0) {
                return fileEncryptionService.decrypt(encryptedBytes);
            }
            log.warn("WORM storage returned empty for job {}, falling back to PDF generation", printJobId);
        }

        // Fallback: regenerate PDF from template (works when MinIO disabled in dev)
        return retrieveOrGeneratePdf(job);
    }

    @Override
    @Transactional(readOnly = true)
    public byte[] getLatestPdfByDemande(UUID demandeId, String tenantId) {
        List<PrintJob> jobs = printJobRepository.findByDemandeIdAndTenantId(demandeId, tenantId);

        PrintJob latestPrinted = jobs.stream()
            .filter(j -> j.getStatus() == PrintStatus.IMPRIME
                      || j.getStatus() == PrintStatus.VERROUILLE_WORM
                      || j.getStatus() == PrintStatus.DELIVRE)
            .max(Comparator.comparing(PrintJob::getCreatedAt))
            .orElseThrow(() -> new PrintJobNotFoundException(
                "No printed document found for demande: " + demandeId, demandeId));

        return getPrintJobPdf(latestPrinted.getId(), tenantId);
    }

    // Private helper methods

    private byte[] retrieveOrGeneratePdf(PrintJob job) {
        if (job.getPdfStoragePath() != null && !job.getPdfStoragePath().isEmpty()) {
            log.debug("Would retrieve PDF from: {}", job.getPdfStoragePath());
        }

        // Build template data from job metadata + enrichment from demande-service
        Map<String, Object> templateData = new HashMap<>(job.getMetadata() != null ? job.getMetadata() : Map.of());
        templateData.put("documentId", job.getDocumentId().toString());
        templateData.put("documentReference", job.getDocumentReference());
        templateData.put("documentType", job.getDocumentType());
        templateData.put("printDate", Instant.now().toString());
        templateData.put("documentHash", job.getDocumentHash() != null ? job.getDocumentHash() : "");

        // Ajouter le code de verification HMAC-signe pour le QR code du PDF
        if (job.getQrVerificationCode() != null) {
            templateData.put("qrVerificationCode", job.getQrVerificationCode());
        }

        // Crypto hash for Handlebars template (displayed below QR code)
        if (job.getDocumentHash() != null && !job.getDocumentHash().isEmpty()) {
            templateData.put("hashSha256", job.getDocumentHash());
        }

        // Generation timestamp for footer
        templateData.put("dateGeneration",
            java.time.Instant.now().truncatedTo(java.time.temporal.ChronoUnit.SECONDS).toString());

        // Enrich with demande data from DragonflyDB cache (replaces gRPC call to demande-service)
        try {
            String cacheKey = "ec:demande:data:" + job.getDemandeId();
            String cachedJson = redisTemplate.opsForValue().get(cacheKey);
            if (cachedJson != null && !cachedJson.isEmpty()) {
                var mapper = new com.fasterxml.jackson.databind.ObjectMapper();
                @SuppressWarnings("unchecked")
                Map<String, Object> cachedData = mapper.readValue(cachedJson, Map.class);

                // Inject all cached fields
                cachedData.forEach(templateData::putIfAbsent);

                // Map common field names for PDF template compatibility
                Object nomDemandeur = cachedData.get("nomDemandeur");
                Object prenomDemandeur = cachedData.get("prenomDemandeur");
                if (nomDemandeur != null) templateData.putIfAbsent("nom", nomDemandeur.toString());
                if (prenomDemandeur != null) templateData.putIfAbsent("prenoms", prenomDemandeur.toString());

                // Marriage: map demandeur as epoux
                String docType = cachedData.getOrDefault("typeDocument", job.getDocumentType()).toString();
                if ("ACTE_MARIAGE".equals(docType) || "MARIAGE".equals(docType)) {
                    if (nomDemandeur != null) templateData.putIfAbsent("epouxNom", nomDemandeur.toString());
                    if (prenomDemandeur != null) templateData.putIfAbsent("epouxPrenoms", prenomDemandeur.toString());
                    Object dateNaissance = cachedData.get("dateNaissance");
                    if (dateNaissance != null) templateData.putIfAbsent("epouxDateNaissance", dateNaissance.toString());
                }
            } else {
                log.debug("No cached demande data found for key={}", cacheKey);
            }
        } catch (Exception e) {
            log.warn("Failed to enrich template data from DragonflyDB cache for job {}: {}",
                job.getId(), e.getMessage());
        }

        // Common fields for all certificate types
        templateData.putIfAbsent("reference", job.getDocumentReference());
        templateData.putIfAbsent("dateEmission",
            java.time.LocalDate.now().format(java.time.format.DateTimeFormatter.ofPattern("dd/MM/yyyy")));
        templateData.putIfAbsent("statut", "DOCUMENT OFFICIEL");
        templateData.putIfAbsent("lieuDelivrance", "Ouagadougou");
        templateData.putIfAbsent("dateDelivrance",
            java.time.LocalDate.now().format(java.time.format.DateTimeFormatter.ofPattern("dd/MM/yyyy")));
        templateData.putIfAbsent("dateActe",
            java.time.LocalDate.now().format(java.time.format.DateTimeFormatter.ofPattern("dd/MM/yyyy")));

        // ── QR code / verification URL resolution (3-tier fallback) ──
        // Priority: 1) ec-verification-ms registerDocument (HMAC-signed URL)
        //           2) job.getVerificationUrl() (set by validation-acte-service via ec:validated.documents)
        //           3) URL built from job.getQrVerificationCode() (HMAC code from validation-acte-service)
        //           4) Simple fallback URL based on demandeId
        // This ensures verificationUrl + qrCodeData are ALWAYS populated before PDF generation.

        String resolvedVerificationUrl = null;
        String resolvedVerificationToken = null;

        // Tier 1: Try ec-verification-ms registerDocument for best-quality HMAC-signed URL
        try {
            String canonicalJson = new com.fasterxml.jackson.databind.ObjectMapper()
                .writeValueAsString(templateData);
            var verificationResult = verificationGrpcClient.registerDocument(
                job.getDocumentId(),
                job.getDemandeId(),
                job.getTenantId(),
                job.getDocumentReference(),
                (String) templateData.getOrDefault("numeroActe", ""),
                job.getDocumentType(),
                "Etat Civil du Burkina Faso",
                canonicalJson,
                job.getPdfStoragePath(),
                0 // no expiration
            );
            if (verificationResult.isPresent()) {
                var vr = verificationResult.get();
                resolvedVerificationUrl = vr.verificationUrl();
                resolvedVerificationToken = vr.token();
                // Store on job for Kafka event propagation
                job.setQrVerificationCode(vr.token());
                job.setVerificationUrl(vr.verificationUrl());
                log.info("Verification registered for job {}: token={}", job.getId(), vr.token());
            }
        } catch (Exception e) {
            log.warn("Verification registration failed for job {}: {}", job.getId(), e.getMessage());
        }

        // Tier 2: Fall back to URL already set on PrintJob (from validation-acte-service via ec:validated.documents)
        if (resolvedVerificationUrl == null && job.getVerificationUrl() != null) {
            resolvedVerificationUrl = job.getVerificationUrl();
            resolvedVerificationToken = job.getQrVerificationCode();
            log.debug("Using PrintJob verificationUrl for job {}: {}", job.getId(), resolvedVerificationUrl);
        }

        // Tier 3: Build URL from QR verification code (HMAC code set by validation-acte-service)
        if (resolvedVerificationUrl == null && job.getQrVerificationCode() != null) {
            resolvedVerificationUrl = "https://actes.gov.bf/verify?code=" + job.getQrVerificationCode();
            resolvedVerificationToken = job.getQrVerificationCode();
            log.debug("Built verificationUrl from qrVerificationCode for job {}: {}", job.getId(), resolvedVerificationUrl);
        }

        // Tier 4: Ultimate fallback — simple URL based on demandeId (always generates a QR code)
        if (resolvedVerificationUrl == null) {
            resolvedVerificationUrl = "https://actes.gov.bf/verify/" + job.getDemandeId();
            log.warn("Using fallback verificationUrl for job {} (no verification source available): {}",
                job.getId(), resolvedVerificationUrl);
        }

        // Populate templateData — verificationUrl and qrCodeData are now ALWAYS set
        templateData.put("verificationUrl", resolvedVerificationUrl);
        templateData.put("qrCodeData", resolvedVerificationUrl);
        if (resolvedVerificationToken != null) {
            templateData.put("verificationToken", resolvedVerificationToken);
        }

        return pdfGenerationService.generatePdf(job.getDocumentType(), templateData, job.getTenantId());
    }

    private static final Set<String> ALLOWED_METADATA_KEYS = Set.of(
        "documentType", "documentHash", "verificationToken", "verificationUrl",
        "wormBucket", "wormObjectKey", "source", "pipelineFingerprint",
        "numeroActe", "numeroPermis", "documentReference"
    );

    private Map<String, Object> filterMetadata(Map<String, Object> metadata) {
        Map<String, Object> filtered = new HashMap<>();
        metadata.forEach((key, value) -> {
            if (ALLOWED_METADATA_KEYS.contains(key)) {
                filtered.put(key, value);
            }
        });
        return filtered;
    }

    private String generateWatermarkText(PrintJob job) {
        return String.format(
            "DOCUMENT OFFICIEL - %s - REF: %s - %s",
            job.getTenantId().toUpperCase(),
            job.getDocumentReference(),
            Instant.now().truncatedTo(ChronoUnit.DAYS).toString().substring(0, 10)
        );
    }
}
