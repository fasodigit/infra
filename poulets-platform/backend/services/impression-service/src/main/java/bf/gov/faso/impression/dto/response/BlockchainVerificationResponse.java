package bf.gov.faso.impression.dto.response;

import bf.gov.faso.impression.entity.BlockchainAction;
import bf.gov.faso.impression.entity.BlockchainEntry;

import java.time.Instant;
import java.util.UUID;

/**
 * DTO Record for blockchain verification response.
 */
public record BlockchainVerificationResponse(
    UUID id,
    UUID documentId,
    UUID printJobId,
    String documentHash,
    String blockHash,
    String previousBlockHash,
    Long blockNumber,
    BlockchainAction action,
    UUID operatorId,
    String tenantId,
    Instant timestamp,
    boolean integrityValid,
    boolean chainValid,
    String verificationMessage
) {
    /**
     * Factory method to create a response from entity with verification result.
     */
    public static BlockchainVerificationResponse fromEntity(
            BlockchainEntry entry,
            boolean integrityValid,
            boolean chainValid,
            String verificationMessage) {
        return new BlockchainVerificationResponse(
            entry.getId(),
            entry.getDocumentId(),
            entry.getPrintJobId(),
            entry.getDocumentHash(),
            entry.getBlockHash(),
            entry.getPreviousBlockHash(),
            entry.getBlockNumber(),
            entry.getAction(),
            entry.getOperatorId(),
            entry.getTenantId(),
            entry.getTimestamp(),
            integrityValid,
            chainValid,
            verificationMessage
        );
    }
}
