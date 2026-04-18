package bf.gov.faso.impression.entity;

import jakarta.persistence.*;
import org.apache.commons.codec.digest.DigestUtils;
import org.springframework.data.annotation.CreatedDate;
import org.springframework.data.jpa.domain.support.AuditingEntityListener;

import java.time.Instant;
import java.util.Objects;
import java.util.UUID;

/**
 * Entity representing an entry in the blockchain audit trail.
 *
 * Each entry is cryptographically linked to the previous entry via hash chaining,
 * ensuring immutability and traceability of all print operations.
 */
@Entity
@Table(
    name = "blockchain_entries",
    indexes = {
        @Index(name = "idx_blockchain_document_id", columnList = "document_id"),
        @Index(name = "idx_blockchain_tenant_id", columnList = "tenant_id"),
        @Index(name = "idx_blockchain_block_hash", columnList = "block_hash"),
        @Index(name = "idx_blockchain_timestamp", columnList = "timestamp"),
        @Index(name = "idx_blockchain_action", columnList = "action")
    }
)
@EntityListeners(AuditingEntityListener.class)
public class BlockchainEntry {

    @Id
    @Column(name = "id", updatable = false, nullable = false)
    private UUID id;

    @Column(name = "document_id", nullable = false)
    private UUID documentId;

    @Column(name = "print_job_id")
    private UUID printJobId;

    @Column(name = "document_hash", nullable = false, length = 128)
    private String documentHash;

    @Column(name = "previous_block_hash", nullable = false, length = 128)
    private String previousBlockHash;

    @Column(name = "block_hash", nullable = false, length = 128)
    private String blockHash;

    @CreatedDate
    @Column(name = "timestamp", nullable = false, updatable = false)
    private Instant timestamp;

    @Column(name = "tenant_id", nullable = false)
    private String tenantId;

    @Column(name = "operator_id", nullable = false)
    private UUID operatorId;

    @Enumerated(EnumType.STRING)
    @Column(name = "action", nullable = false, length = 30)
    private BlockchainAction action;

    @Column(name = "block_number", nullable = false)
    private Long blockNumber;

    @Column(name = "nonce", nullable = false)
    private Long nonce;

    @Column(name = "details", length = 2000)
    private String details;

    @Column(name = "client_ip", length = 50)
    private String clientIp;

    @Column(name = "user_agent", length = 500)
    private String userAgent;

    @Column(name = "synced_to_audit_log", nullable = false)
    private boolean syncedToAuditLog = false;

    @Column(name = "synced_at")
    private Instant syncedAt;

    // Constructors
    public BlockchainEntry() {
        this.id = UUID.randomUUID();
        this.nonce = System.nanoTime();
    }

    public BlockchainEntry(UUID documentId, String documentHash, String tenantId,
                           UUID operatorId, BlockchainAction action) {
        this();
        this.documentId = documentId;
        this.documentHash = documentHash;
        this.tenantId = tenantId;
        this.operatorId = operatorId;
        this.action = action;
    }

    // Business methods

    /**
     * Calculates and sets the block hash based on the block's content.
     * This should be called before persisting the entity.
     */
    @PrePersist
    public void calculateBlockHash() {
        if (this.timestamp == null) {
            this.timestamp = Instant.now();
        }

        String data = String.join("|",
            documentId.toString(),
            documentHash,
            previousBlockHash != null ? previousBlockHash : "GENESIS",
            timestamp.toString(),
            action.name(),
            operatorId.toString(),
            tenantId,
            String.valueOf(nonce)
        );

        this.blockHash = DigestUtils.sha256Hex(data);
    }

    /**
     * Verifies that the block hash is valid.
     */
    public boolean verifyIntegrity() {
        String data = String.join("|",
            documentId.toString(),
            documentHash,
            previousBlockHash != null ? previousBlockHash : "GENESIS",
            timestamp.toString(),
            action.name(),
            operatorId.toString(),
            tenantId,
            String.valueOf(nonce)
        );

        String calculatedHash = DigestUtils.sha256Hex(data);
        return calculatedHash.equals(blockHash);
    }

    /**
     * Creates a genesis block for a new tenant.
     */
    public static BlockchainEntry createGenesisBlock(String tenantId, UUID operatorId) {
        BlockchainEntry genesis = new BlockchainEntry();
        genesis.setDocumentId(UUID.fromString("00000000-0000-0000-0000-000000000000"));
        genesis.setDocumentHash("GENESIS");
        genesis.setTenantId(tenantId);
        genesis.setOperatorId(operatorId);
        genesis.setAction(BlockchainAction.GENESIS);
        genesis.setPreviousBlockHash("GENESIS");
        genesis.setBlockNumber(0L);
        genesis.setDetails("Genesis block for tenant: " + tenantId);
        return genesis;
    }

    // Getters and Setters
    public UUID getId() {
        return id;
    }

    public void setId(UUID id) {
        this.id = id;
    }

    public UUID getDocumentId() {
        return documentId;
    }

    public void setDocumentId(UUID documentId) {
        this.documentId = documentId;
    }

    public UUID getPrintJobId() {
        return printJobId;
    }

    public void setPrintJobId(UUID printJobId) {
        this.printJobId = printJobId;
    }

    public String getDocumentHash() {
        return documentHash;
    }

    public void setDocumentHash(String documentHash) {
        this.documentHash = documentHash;
    }

    public String getPreviousBlockHash() {
        return previousBlockHash;
    }

    public void setPreviousBlockHash(String previousBlockHash) {
        this.previousBlockHash = previousBlockHash;
    }

    public String getBlockHash() {
        return blockHash;
    }

    public void setBlockHash(String blockHash) {
        this.blockHash = blockHash;
    }

    public Instant getTimestamp() {
        return timestamp;
    }

    public void setTimestamp(Instant timestamp) {
        this.timestamp = timestamp;
    }

    public String getTenantId() {
        return tenantId;
    }

    public void setTenantId(String tenantId) {
        this.tenantId = tenantId;
    }

    public UUID getOperatorId() {
        return operatorId;
    }

    public void setOperatorId(UUID operatorId) {
        this.operatorId = operatorId;
    }

    public BlockchainAction getAction() {
        return action;
    }

    public void setAction(BlockchainAction action) {
        this.action = action;
    }

    public Long getBlockNumber() {
        return blockNumber;
    }

    public void setBlockNumber(Long blockNumber) {
        this.blockNumber = blockNumber;
    }

    public Long getNonce() {
        return nonce;
    }

    public void setNonce(Long nonce) {
        this.nonce = nonce;
    }

    public String getDetails() {
        return details;
    }

    public void setDetails(String details) {
        this.details = details;
    }

    public String getClientIp() {
        return clientIp;
    }

    public void setClientIp(String clientIp) {
        this.clientIp = clientIp;
    }

    public String getUserAgent() {
        return userAgent;
    }

    public void setUserAgent(String userAgent) {
        this.userAgent = userAgent;
    }

    public boolean isSyncedToAuditLog() {
        return syncedToAuditLog;
    }

    public void setSyncedToAuditLog(boolean syncedToAuditLog) {
        this.syncedToAuditLog = syncedToAuditLog;
    }

    public Instant getSyncedAt() {
        return syncedAt;
    }

    public void setSyncedAt(Instant syncedAt) {
        this.syncedAt = syncedAt;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        BlockchainEntry that = (BlockchainEntry) o;
        return Objects.equals(id, that.id);
    }

    @Override
    public int hashCode() {
        return Objects.hash(id);
    }

    @Override
    public String toString() {
        return "BlockchainEntry{" +
                "id=" + id +
                ", documentId=" + documentId +
                ", action=" + action +
                ", blockNumber=" + blockNumber +
                ", blockHash='" + blockHash + '\'' +
                ", timestamp=" + timestamp +
                '}';
    }
}
