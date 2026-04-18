package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.entity.PrintStatus;
import bf.gov.faso.impression.exception.InvalidPrintStateException;
import bf.gov.faso.impression.exception.PrintJobNotFoundException;
import bf.gov.faso.impression.repository.PrintJobRepository;
import bf.gov.faso.impression.service.WormStorageService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;

/**
 * Transactional helper for PrintJob DB operations.
 *
 * Extracted from ImpressionServiceImpl so that @Transactional annotations work
 * correctly via Spring's proxy-based AOP (self-invocation within the same class
 * bypasses the proxy and ignores @Transactional).
 *
 * Each method opens a short-lived transaction, executes the DB write, and commits
 * immediately -- keeping DB connections free during external I/O (PDF generation,
 * gRPC calls, WORM storage, file archive).
 */
@Component
public class PrintJobTransactionalHelper {

    private static final Logger log = LoggerFactory.getLogger(PrintJobTransactionalHelper.class);

    private final PrintJobRepository printJobRepository;

    public PrintJobTransactionalHelper(PrintJobRepository printJobRepository) {
        this.printJobRepository = printJobRepository;
    }

    /**
     * Phase 1: Load the print job and mark it EN_COURS.
     * Short transaction -- releases DB connection immediately after commit.
     */
    @Transactional
    public PrintJob markJobInProgress(UUID printJobId, UUID operatorId, String tenantId) {
        PrintJob job = printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));

        if (!job.canPrint()) {
            throw new InvalidPrintStateException(printJobId, job.getStatus(), "print");
        }

        job.setStatus(PrintStatus.EN_COURS);
        job.setOperatorId(operatorId);
        return printJobRepository.save(job);
    }

    /**
     * Phase 3: Finalize the job with print results (hash, blockchain, WORM lock).
     * Re-loads the entity to get a managed JPA instance.
     * Short transaction -- releases DB connection immediately after commit.
     */
    @Transactional
    public PrintJob finalizeJobAfterPrint(
            UUID printJobId, String tenantId, UUID operatorId,
            String documentHash, String blockHash,
            WormStorageService.WormStorageResult wormResult) {

        PrintJob job = printJobRepository.findByIdAndTenantId(printJobId, tenantId)
            .orElseThrow(() -> new PrintJobNotFoundException(printJobId));

        job.markAsPrinted(operatorId, documentHash, blockHash);
        job.applyWormLock(
            wormResult.bucket(),
            wormResult.objectKey(),
            wormResult.retentionUntil()
        );
        return printJobRepository.save(job);
    }

    /**
     * Mark a job as failed. Uses REQUIRES_NEW so the error state persists
     * even if the outer context is rolling back.
     */
    @Transactional(propagation = Propagation.REQUIRES_NEW)
    public void markJobFailed(UUID printJobId, String tenantId, String errorMessage) {
        try {
            PrintJob job = printJobRepository.findByIdAndTenantId(printJobId, tenantId)
                .orElse(null);
            if (job != null) {
                job.markAsFailed(errorMessage);
                printJobRepository.save(job);
            }
        } catch (Exception ex) {
            log.error("Failed to mark job {} as failed: {}", printJobId, ex.getMessage());
        }
    }
}
