package bf.gov.faso.impression.service;

import bf.gov.faso.impression.entity.BlockchainAction;
import bf.gov.faso.impression.entity.BlockchainEntry;
import bf.gov.faso.impression.kafka.PrintEventProducer;
import bf.gov.faso.impression.repository.BlockchainRepository;
import bf.gov.faso.impression.service.impl.BlockchainServiceImpl;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.ArgumentCaptor;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class BlockchainServiceTest {

    @Mock
    private BlockchainRepository blockchainRepository;

    @Mock
    private PrintEventProducer printEventProducer;

    private BlockchainService blockchainService;

    private static final String TENANT_ID = "test-tenant";
    private static final UUID OPERATOR_ID = UUID.randomUUID();
    private static final UUID DOCUMENT_ID = UUID.randomUUID();
    private static final String DOCUMENT_HASH = "abc123def456";

    @BeforeEach
    void setUp() {
        blockchainService = new BlockchainServiceImpl(blockchainRepository, printEventProducer);
    }

    @Test
    @DisplayName("Should initialize blockchain with genesis block")
    void shouldInitializeChainWithGenesisBlock() {
        // Given
        when(blockchainRepository.existsByTenantIdAndAction(TENANT_ID, BlockchainAction.GENESIS))
            .thenReturn(false);
        when(blockchainRepository.save(any(BlockchainEntry.class)))
            .thenAnswer(invocation -> invocation.getArgument(0));

        // When
        BlockchainEntry result = blockchainService.initializeChain(TENANT_ID, OPERATOR_ID);

        // Then
        assertThat(result).isNotNull();
        assertThat(result.getAction()).isEqualTo(BlockchainAction.GENESIS);
        assertThat(result.getTenantId()).isEqualTo(TENANT_ID);
        assertThat(result.getOperatorId()).isEqualTo(OPERATOR_ID);
        assertThat(result.getPreviousBlockHash()).isEqualTo("GENESIS");
        assertThat(result.getBlockNumber()).isEqualTo(0L);

        verify(blockchainRepository).save(any(BlockchainEntry.class));
    }

    @Test
    @DisplayName("Should add entry to existing chain")
    void shouldAddEntryToExistingChain() {
        // Given
        BlockchainEntry lastEntry = new BlockchainEntry();
        lastEntry.setBlockHash("previous-hash");
        lastEntry.setBlockNumber(5L);

        when(blockchainRepository.findTopByTenantIdOrderByBlockNumberDesc(TENANT_ID))
            .thenReturn(Optional.of(lastEntry));
        when(blockchainRepository.getNextBlockNumber(TENANT_ID)).thenReturn(6L);
        when(blockchainRepository.save(any(BlockchainEntry.class)))
            .thenAnswer(invocation -> invocation.getArgument(0));

        // When
        BlockchainEntry result = blockchainService.addEntry(
            DOCUMENT_ID,
            UUID.randomUUID(),
            DOCUMENT_HASH,
            OPERATOR_ID,
            TENANT_ID,
            BlockchainAction.PRINT
        );

        // Then
        assertThat(result).isNotNull();
        assertThat(result.getDocumentId()).isEqualTo(DOCUMENT_ID);
        assertThat(result.getDocumentHash()).isEqualTo(DOCUMENT_HASH);
        assertThat(result.getPreviousBlockHash()).isEqualTo("previous-hash");
        assertThat(result.getBlockNumber()).isEqualTo(6L);
        assertThat(result.getAction()).isEqualTo(BlockchainAction.PRINT);
    }

    @Test
    @DisplayName("Should verify chain integrity with valid chain")
    void shouldVerifyChainIntegrityWithValidChain() {
        // Given
        BlockchainEntry genesis = createTestEntry(0L, "GENESIS", "hash0");
        BlockchainEntry entry1 = createTestEntry(1L, "hash0", "hash1");
        BlockchainEntry entry2 = createTestEntry(2L, "hash1", "hash2");

        when(blockchainRepository.findByTenantIdOrderByBlockNumberAsc(TENANT_ID))
            .thenReturn(List.of(genesis, entry1, entry2));
        when(blockchainRepository.countBlockNumberGaps(TENANT_ID)).thenReturn(0L);

        // When
        boolean isValid = blockchainService.verifyChainIntegrity(TENANT_ID);

        // Then
        assertThat(isValid).isTrue();
    }

    @Test
    @DisplayName("Should detect broken chain linkage")
    void shouldDetectBrokenChainLinkage() {
        // Given
        BlockchainEntry genesis = createTestEntry(0L, "GENESIS", "hash0");
        BlockchainEntry entry1 = createTestEntry(1L, "wrong-hash", "hash1"); // Wrong previous hash

        when(blockchainRepository.findByTenantIdOrderByBlockNumberAsc(TENANT_ID))
            .thenReturn(List.of(genesis, entry1));

        // When
        boolean isValid = blockchainService.verifyChainIntegrity(TENANT_ID);

        // Then
        assertThat(isValid).isFalse();
    }

    @Test
    @DisplayName("Should check if chain is initialized")
    void shouldCheckIfChainIsInitialized() {
        // Given
        when(blockchainRepository.existsByTenantIdAndAction(TENANT_ID, BlockchainAction.GENESIS))
            .thenReturn(true);

        // When
        boolean isInitialized = blockchainService.isChainInitialized(TENANT_ID);

        // Then
        assertThat(isInitialized).isTrue();
    }

    @Test
    @DisplayName("Should get entries for document")
    void shouldGetEntriesForDocument() {
        // Given
        List<BlockchainEntry> entries = List.of(
            createTestEntry(1L, "prev", "hash1"),
            createTestEntry(2L, "hash1", "hash2")
        );

        when(blockchainRepository.findByDocumentIdAndTenantIdOrderByTimestampAsc(DOCUMENT_ID, TENANT_ID))
            .thenReturn(entries);

        // When
        List<BlockchainEntry> result = blockchainService.getEntriesForDocument(DOCUMENT_ID, TENANT_ID);

        // Then
        assertThat(result).hasSize(2);
    }

    private BlockchainEntry createTestEntry(Long blockNumber, String previousHash, String blockHash) {
        BlockchainEntry entry = new BlockchainEntry();
        entry.setId(UUID.randomUUID());
        entry.setDocumentId(DOCUMENT_ID);
        entry.setDocumentHash(DOCUMENT_HASH);
        entry.setTenantId(TENANT_ID);
        entry.setOperatorId(OPERATOR_ID);
        entry.setBlockNumber(blockNumber);
        entry.setPreviousBlockHash(previousHash);
        entry.setBlockHash(blockHash);
        entry.setAction(blockNumber == 0L ? BlockchainAction.GENESIS : BlockchainAction.PRINT);
        entry.setNonce(System.nanoTime());
        return entry;
    }
}
