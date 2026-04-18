package bf.gov.faso.impression.scheduler;

import bf.gov.faso.cache.dragonfly.DragonflyDBCacheService;
import bf.gov.faso.impression.cache.PrintJobCacheDTO;
import bf.gov.faso.impression.cache.PrintJobSearchCacheService;
import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.entity.PrintStatus;
import bf.gov.faso.impression.repository.PrintJobRepository;
import jakarta.annotation.PreDestroy;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.*;

/**
 * Scheduled flush of dirty print jobs from DragonflyDB cache to PostgreSQL.
 *
 * Only active when write-behind is enabled (ec.cache.write-behind.enabled=true).
 * Reads pending IDs from the ec:impression:wb:pending set, loads current cache state,
 * merges into DB entities, and batch-saves.
 *
 * Shutdown ordering:
 * 1. Spring stops accepting new requests (server.shutdown=graceful)
 * 2. Existing requests drain (30s timeout via spring.lifecycle.timeout-per-shutdown-phase)
 * 3. @PreDestroy: flush pending write-behind DragonflyDB -> PostgreSQL
 * 4. Spring destroys remaining beans (HikariCP, DragonflyDB connections)
 */
@Component
@ConditionalOnProperty(name = "ec.cache.write-behind.enabled", havingValue = "true")
public class PrintJobPersistenceScheduler {

    private static final Logger log = LoggerFactory.getLogger(PrintJobPersistenceScheduler.class);
    private static final int BATCH_SIZE = 50;

    private final PrintJobSearchCacheService printJobSearchCacheService;
    private final DragonflyDBCacheService dragonflyDBCacheService;
    private final PrintJobRepository printJobRepository;

    public PrintJobPersistenceScheduler(
            PrintJobSearchCacheService printJobSearchCacheService,
            DragonflyDBCacheService dragonflyDBCacheService,
            PrintJobRepository printJobRepository
    ) {
        this.printJobSearchCacheService = printJobSearchCacheService;
        this.dragonflyDBCacheService = dragonflyDBCacheService;
        this.printJobRepository = printJobRepository;
    }

    @Scheduled(cron = "${ec.cache.write-behind.flush-cron:0 0 21 * * *}")
    @Transactional
    public void flushPendingToDatabase() {
        long start = System.currentTimeMillis();

        Set<String> pendingIds = printJobSearchCacheService.getPendingFlushIds();
        if (pendingIds == null || pendingIds.isEmpty()) {
            log.debug("No pending print jobs to flush");
            return;
        }

        log.info("Starting flush of {} pending print jobs to PostgreSQL", pendingIds.size());

        List<String> idList = new ArrayList<>(pendingIds);
        int totalFlushed = 0;
        Set<String> flushedIds = new HashSet<>();

        for (int i = 0; i < idList.size(); i += BATCH_SIZE) {
            int end = Math.min(i + BATCH_SIZE, idList.size());
            List<String> batch = idList.subList(i, end);

            int batchFlushed = flushBatch(batch, flushedIds);
            totalFlushed += batchFlushed;
        }

        // Remove successfully flushed IDs from the pending set
        if (!flushedIds.isEmpty()) {
            printJobSearchCacheService.removePendingFlush(flushedIds);
        }

        long duration = System.currentTimeMillis() - start;
        log.info("Flushed {} print jobs to PostgreSQL in {}ms ({} batches)",
            totalFlushed, duration, (idList.size() + BATCH_SIZE - 1) / BATCH_SIZE);
    }

    private int flushBatch(List<String> batchIds, Set<String> flushedIds) {
        List<PrintJob> toSave = new ArrayList<>();

        for (String idStr : batchIds) {
            try {
                Optional<PrintJobCacheDTO> cached = printJobSearchCacheService.getFromCache(idStr);
                if (cached.isEmpty()) {
                    log.warn("Cache miss for pending print job {} — skipping", idStr);
                    flushedIds.add(idStr); // Remove from pending to avoid infinite retry
                    continue;
                }

                PrintJobCacheDTO dto = cached.get();
                UUID printJobId = UUID.fromString(idStr);

                Optional<PrintJob> existingOpt = printJobRepository.findById(printJobId);
                if (existingOpt.isPresent()) {
                    // UPDATE — apply cached state to existing entity
                    applyDtoToEntity(dto, existingOpt.get());
                    toSave.add(existingOpt.get());
                } else {
                    log.warn("Print job {} exists in cache but not in DB — skipping", idStr);
                }
                flushedIds.add(idStr);

            } catch (Exception e) {
                log.error("Failed to prepare flush for print job {}: {}", idStr, e.getMessage());
            }
        }

        if (!toSave.isEmpty()) {
            try {
                printJobRepository.saveAll(toSave);
                log.debug("Batch saved {} print jobs", toSave.size());
            } catch (Exception e) {
                log.error("Batch save failed for {} print jobs: {}", toSave.size(), e.getMessage());
                // Remove from flushedIds so they'll be retried
                toSave.forEach(j -> flushedIds.remove(j.getId().toString()));
                return 0;
            }
        }

        return toSave.size();
    }

    /**
     * Graceful shutdown hook: flush all pending write-behind entries to PostgreSQL
     * before the application context is destroyed.
     */
    @PreDestroy
    public void onShutdown() {
        log.info("Graceful shutdown: flushing pending write-behind print job entries...");
        try {
            Set<String> pendingIds = printJobSearchCacheService.getPendingFlushIds();
            int pendingCount = (pendingIds != null) ? pendingIds.size() : 0;

            if (pendingCount == 0) {
                log.info("Graceful shutdown: no pending write-behind print job entries to flush");
                return;
            }

            flushPendingToDatabase();
            log.info("Graceful shutdown: successfully flushed {} write-behind print job entries to PostgreSQL", pendingCount);
        } catch (Exception e) {
            log.warn("Graceful shutdown: failed to flush write-behind print job entries — {}. "
                + "Entries remain in DragonflyDB pending set and will be flushed on next startup.",
                e.getMessage());
        }
    }

    private void applyDtoToEntity(PrintJobCacheDTO dto, PrintJob entity) {
        if (dto.status() != null && !dto.status().isEmpty()) {
            entity.setStatus(PrintStatus.valueOf(dto.status()));
        }
        if (dto.operatorId() != null && !dto.operatorId().isEmpty()) {
            entity.setOperatorId(UUID.fromString(dto.operatorId()));
        }
        if (dto.documentHash() != null) {
            entity.setDocumentHash(dto.documentHash());
        }
        if (dto.blockchainHash() != null) {
            entity.setBlockchainHash(dto.blockchainHash());
        }
        if (dto.pdfStoragePath() != null) {
            entity.setPdfStoragePath(dto.pdfStoragePath());
        }
        if (dto.qrVerificationCode() != null) {
            entity.setQrVerificationCode(dto.qrVerificationCode());
        }
        if (dto.verificationUrl() != null) {
            entity.setVerificationUrl(dto.verificationUrl());
        }
        // WORM fields
        if (dto.wormBucket() != null) {
            entity.setWormBucket(dto.wormBucket());
        }
        if (dto.wormObjectKey() != null) {
            entity.setWormObjectKey(dto.wormObjectKey());
        }
        entity.setWormLocked("true".equals(dto.wormLocked()));
        if (dto.wormLockedAt() != null) {
            entity.setWormLockedAt(Instant.parse(dto.wormLockedAt()));
        }
        if (dto.wormRetentionUntil() != null) {
            entity.setWormRetentionUntil(Instant.parse(dto.wormRetentionUntil()));
        }
        // Print tracking
        if (dto.copiesPrinted() != null && !dto.copiesPrinted().isEmpty()) {
            entity.setCopiesPrinted(Integer.parseInt(dto.copiesPrinted()));
        }
        if (dto.printedAt() != null) {
            entity.setPrintedAt(Instant.parse(dto.printedAt()));
        }
        if (dto.deliveredAt() != null) {
            entity.setDeliveredAt(Instant.parse(dto.deliveredAt()));
        }
        if (dto.deliveredTo() != null) {
            entity.setDeliveredTo(dto.deliveredTo());
        }
        if (dto.deliveryMethod() != null) {
            entity.setDeliveryMethod(dto.deliveryMethod());
        }
        if (dto.recipientSignature() != null) {
            entity.setRecipientSignature(dto.recipientSignature());
        }
        // Error
        if (dto.errorMessage() != null) {
            entity.setErrorMessage(dto.errorMessage());
        }
    }
}
