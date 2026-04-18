package bf.gov.faso.impression.entity;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.time.Instant;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

class BlockchainEntryTest {

    private static final String TENANT_ID = "test-tenant";
    private static final UUID OPERATOR_ID = UUID.randomUUID();
    private static final UUID DOCUMENT_ID = UUID.randomUUID();
    private static final String DOCUMENT_HASH = "abc123def456";

    @Test
    @DisplayName("Should calculate block hash on pre-persist")
    void shouldCalculateBlockHashOnPrePersist() {
        // Given
        BlockchainEntry entry = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry.setPreviousBlockHash("previous-hash");

        // When
        entry.calculateBlockHash();

        // Then
        assertThat(entry.getBlockHash()).isNotNull();
        assertThat(entry.getBlockHash()).hasSize(64); // SHA-256 produces 64 hex characters
    }

    @Test
    @DisplayName("Should verify integrity for valid block")
    void shouldVerifyIntegrityForValidBlock() {
        // Given
        BlockchainEntry entry = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry.setPreviousBlockHash("previous-hash");
        entry.setTimestamp(Instant.now());
        entry.calculateBlockHash();

        // When
        boolean isValid = entry.verifyIntegrity();

        // Then
        assertThat(isValid).isTrue();
    }

    @Test
    @DisplayName("Should detect tampered block")
    void shouldDetectTamperedBlock() {
        // Given
        BlockchainEntry entry = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry.setPreviousBlockHash("previous-hash");
        entry.setTimestamp(Instant.now());
        entry.calculateBlockHash();

        // Tamper with the document hash after calculating block hash
        entry.setDocumentHash("tampered-hash");

        // When
        boolean isValid = entry.verifyIntegrity();

        // Then
        assertThat(isValid).isFalse();
    }

    @Test
    @DisplayName("Should create genesis block correctly")
    void shouldCreateGenesisBlockCorrectly() {
        // When
        BlockchainEntry genesis = BlockchainEntry.createGenesisBlock(TENANT_ID, OPERATOR_ID);

        // Then
        assertThat(genesis.getAction()).isEqualTo(BlockchainAction.GENESIS);
        assertThat(genesis.getPreviousBlockHash()).isEqualTo("GENESIS");
        assertThat(genesis.getBlockNumber()).isEqualTo(0L);
        assertThat(genesis.getTenantId()).isEqualTo(TENANT_ID);
        assertThat(genesis.getOperatorId()).isEqualTo(OPERATOR_ID);
        assertThat(genesis.getDocumentHash()).isEqualTo("GENESIS");
    }

    @Test
    @DisplayName("Should generate different hashes for different blocks")
    void shouldGenerateDifferentHashesForDifferentBlocks() {
        // Given
        BlockchainEntry entry1 = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry1.setPreviousBlockHash("hash1");
        entry1.setTimestamp(Instant.now());
        entry1.calculateBlockHash();

        BlockchainEntry entry2 = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry2.setPreviousBlockHash("hash2"); // Different previous hash
        entry2.setTimestamp(Instant.now());
        entry2.calculateBlockHash();

        // Then
        assertThat(entry1.getBlockHash()).isNotEqualTo(entry2.getBlockHash());
    }

    @Test
    @DisplayName("Should include nonce in hash calculation")
    void shouldIncludeNonceInHashCalculation() {
        // Given
        BlockchainEntry entry1 = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry1.setPreviousBlockHash("prev");
        entry1.setTimestamp(Instant.parse("2025-01-01T00:00:00Z"));
        entry1.setNonce(12345L);
        entry1.calculateBlockHash();

        BlockchainEntry entry2 = new BlockchainEntry(
            DOCUMENT_ID, DOCUMENT_HASH, TENANT_ID, OPERATOR_ID, BlockchainAction.PRINT);
        entry2.setPreviousBlockHash("prev");
        entry2.setTimestamp(Instant.parse("2025-01-01T00:00:00Z"));
        entry2.setNonce(67890L); // Different nonce
        entry2.calculateBlockHash();

        // Then
        assertThat(entry1.getBlockHash()).isNotEqualTo(entry2.getBlockHash());
    }
}
