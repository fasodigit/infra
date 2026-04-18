package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.dto.response.BlockchainVerificationResponse;
import bf.gov.faso.impression.entity.BlockchainAction;
import bf.gov.faso.impression.entity.BlockchainEntry;
import bf.gov.faso.impression.exception.BlockchainIntegrityException;
import bf.gov.faso.impression.kafka.PrintEventProducer;
import bf.gov.faso.impression.repository.BlockchainRepository;
import bf.gov.faso.impression.service.BlockchainService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * Implementation of the blockchain audit trail service.
 */
@Service
@Transactional
public class BlockchainServiceImpl implements BlockchainService {

    private static final Logger log = LoggerFactory.getLogger(BlockchainServiceImpl.class);

    private static final int BATCH_SIZE = 50;

    private final BlockchainRepository blockchainRepository;
    private final PrintEventProducer printEventProducer;

    public BlockchainServiceImpl(
            BlockchainRepository blockchainRepository,
            PrintEventProducer printEventProducer) {
        this.blockchainRepository = blockchainRepository;
        this.printEventProducer = printEventProducer;
    }

    @Override
    public BlockchainEntry addEntry(
            UUID documentId,
            UUID printJobId,
            String documentHash,
            UUID operatorId,
            String tenantId,
            BlockchainAction action) {
        return addEntry(documentId, printJobId, documentHash, operatorId, tenantId, action, null, null, null);
    }

    @Override
    public BlockchainEntry addEntry(
            UUID documentId,
            UUID printJobId,
            String documentHash,
            UUID operatorId,
            String tenantId,
            BlockchainAction action,
            String details,
            String clientIp,
            String userAgent) {

        log.info("Adding blockchain entry for document {} action {} in tenant {}",
            documentId, action, tenantId);

        // Get the previous block's hash
        BlockchainEntry lastEntry = blockchainRepository.findTopByTenantIdOrderByBlockNumberDesc(tenantId)
            .orElse(null);

        String previousHash = (lastEntry != null) ? lastEntry.getBlockHash() : "GENESIS";
        Long nextBlockNumber = blockchainRepository.getNextBlockNumber(tenantId);

        // Create new entry
        BlockchainEntry entry = new BlockchainEntry(documentId, documentHash, tenantId, operatorId, action);
        entry.setPrintJobId(printJobId);
        entry.setPreviousBlockHash(previousHash);
        entry.setBlockNumber(nextBlockNumber);
        entry.setDetails(details);
        entry.setClientIp(clientIp);
        entry.setUserAgent(userAgent);

        // Hash is calculated in @PrePersist
        entry = blockchainRepository.save(entry);

        log.info("Blockchain entry created: block #{} hash {}", entry.getBlockNumber(), entry.getBlockHash());

        return entry;
    }

    @Override
    @Transactional(readOnly = true)
    public BlockchainVerificationResponse verifyBlock(String blockHash, String tenantId) {
        log.info("Verifying block {} for tenant {}", blockHash, tenantId);

        BlockchainEntry entry = blockchainRepository.findByBlockHashAndTenantId(blockHash, tenantId)
            .orElse(null);

        if (entry == null) {
            return new BlockchainVerificationResponse(
                null, null, null, null, blockHash, null, null,
                null, null, tenantId, null,
                false, false, "Block not found"
            );
        }

        // Verify the block's integrity
        boolean integrityValid = entry.verifyIntegrity();

        // Verify chain linkage
        boolean chainValid = verifyChainLinkage(entry, tenantId);

        String message = integrityValid && chainValid
            ? "Block verification successful"
            : "Block verification failed: " +
                (!integrityValid ? "integrity invalid " : "") +
                (!chainValid ? "chain linkage broken" : "");

        return BlockchainVerificationResponse.fromEntity(entry, integrityValid, chainValid, message);
    }

    @Override
    @Transactional(readOnly = true)
    public boolean verifyChainIntegrity(String tenantId) {
        log.info("Verifying full blockchain integrity for tenant {}", tenantId);

        List<BlockchainEntry> chain = blockchainRepository.findByTenantIdOrderByBlockNumberAsc(tenantId);

        if (chain.isEmpty()) {
            log.warn("No blockchain entries found for tenant {}", tenantId);
            return true;
        }

        String expectedPreviousHash = "GENESIS";

        for (BlockchainEntry entry : chain) {
            // Verify chain linkage
            if (!entry.getPreviousBlockHash().equals(expectedPreviousHash)) {
                log.error("Chain linkage broken at block {}. Expected previous: {}, Found: {}",
                    entry.getBlockNumber(), expectedPreviousHash, entry.getPreviousBlockHash());
                return false;
            }

            // Verify block integrity
            if (!entry.verifyIntegrity()) {
                log.error("Block integrity invalid at block {}", entry.getBlockNumber());
                return false;
            }

            expectedPreviousHash = entry.getBlockHash();
        }

        // Check for gaps in block numbers
        long gaps = blockchainRepository.countBlockNumberGaps(tenantId);
        if (gaps > 0) {
            log.error("Found {} gaps in block numbers for tenant {}", gaps, tenantId);
            return false;
        }

        log.info("Blockchain integrity verified for tenant {}. {} blocks checked.", tenantId, chain.size());
        return true;
    }

    @Override
    @Transactional(readOnly = true)
    public List<BlockchainEntry> getEntriesForDocument(UUID documentId, String tenantId) {
        return blockchainRepository.findByDocumentIdAndTenantIdOrderByTimestampAsc(documentId, tenantId);
    }

    @Override
    @Transactional(readOnly = true)
    public List<BlockchainEntry> getEntriesForPrintJob(UUID printJobId, String tenantId) {
        return blockchainRepository.findByPrintJobIdAndTenantIdOrderByTimestampAsc(printJobId, tenantId);
    }

    @Override
    @Transactional(readOnly = true)
    public BlockchainEntry getLatestEntry(String tenantId) {
        return blockchainRepository.findTopByTenantIdOrderByBlockNumberDesc(tenantId).orElse(null);
    }

    @Override
    public BlockchainEntry initializeChain(String tenantId, UUID operatorId) {
        log.info("Initializing blockchain for tenant {}", tenantId);

        if (isChainInitialized(tenantId)) {
            log.warn("Blockchain already initialized for tenant {}", tenantId);
            return blockchainRepository.findTopByTenantIdOrderByBlockNumberDesc(tenantId)
                .orElseThrow();
        }

        BlockchainEntry genesis = BlockchainEntry.createGenesisBlock(tenantId, operatorId);
        genesis = blockchainRepository.save(genesis);

        log.info("Genesis block created for tenant {}: {}", tenantId, genesis.getBlockHash());

        return genesis;
    }

    @Override
    @Transactional(readOnly = true)
    public boolean isChainInitialized(String tenantId) {
        return blockchainRepository.existsByTenantIdAndAction(tenantId, BlockchainAction.GENESIS);
    }

    @Override
    @Transactional(readOnly = true)
    public long getBlockCount(String tenantId) {
        return blockchainRepository.countByTenantId(tenantId);
    }

    /**
     * Scheduled task: publish pending blockchain entries to DragonflyDB Streams
     * (ec:blockchain.events) for consumption by audit-log-ms, then batch-update the
     * synced flag in PostgreSQL.
     *
     * Improvements over previous implementation:
     * - Batch saveAll() instead of individual save() calls (N+1 -> 1 query per batch of 50)
     * - Publishes to DragonflyDB Streams (sub-ms XADD) instead of synchronous gRPC
     * - Eliminates the 839ms synchronous audit-log gRPC bottleneck
     */
    @Override
    @Scheduled(fixedDelayString = "${blockchain.sync.interval:60000}")
    @Transactional
    public int syncToAuditLogService() {
        log.debug("Syncing blockchain entries to audit-log-ms via DragonflyDB Streams");

        List<BlockchainEntry> pending = blockchainRepository.findBySyncedToAuditLogFalseOrderByTimestampAsc();

        if (pending.isEmpty()) {
            return 0;
        }

        int syncedCount = 0;
        Instant now = Instant.now();
        List<BlockchainEntry> batch = new ArrayList<>(BATCH_SIZE);

        for (BlockchainEntry entry : pending) {
            try {
                // Publish to DragonflyDB Streams (non-blocking, sub-ms)
                printEventProducer.publishBlockchainEntry(
                    entry.getId(),
                    entry.getDocumentId(),
                    entry.getTenantId(),
                    entry.getAction(),
                    entry.getBlockHash(),
                    entry.getBlockNumber()
                );

                entry.setSyncedToAuditLog(true);
                entry.setSyncedAt(now);
                batch.add(entry);
                syncedCount++;

                // Flush batch to DB
                if (batch.size() >= BATCH_SIZE) {
                    blockchainRepository.saveAll(batch);
                    batch.clear();
                }

            } catch (Exception e) {
                log.error("Failed to sync blockchain entry {} to audit-log-ms", entry.getId(), e);
                // Flush whatever we have so far, then continue
                if (!batch.isEmpty()) {
                    blockchainRepository.saveAll(batch);
                    batch.clear();
                }
            }
        }

        // Flush remaining
        if (!batch.isEmpty()) {
            blockchainRepository.saveAll(batch);
        }

        if (syncedCount > 0) {
            log.info("Synced {} blockchain entries to audit-log-ms via DragonflyDB Streams", syncedCount);
        }

        return syncedCount;
    }

    private boolean verifyChainLinkage(BlockchainEntry entry, String tenantId) {
        if ("GENESIS".equals(entry.getPreviousBlockHash())) {
            // Genesis block or first block
            return entry.getBlockNumber() == 0L ||
                   !blockchainRepository.existsByTenantIdAndAction(tenantId, BlockchainAction.GENESIS);
        }

        // Verify previous block exists with matching hash
        return blockchainRepository.findByBlockHashAndTenantId(entry.getPreviousBlockHash(), tenantId)
            .map(prev -> prev.getBlockNumber() == entry.getBlockNumber() - 1)
            .orElse(false);
    }
}
