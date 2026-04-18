package bf.gov.faso.impression.exception;

/**
 * Exception thrown when blockchain integrity verification fails.
 */
public class BlockchainIntegrityException extends RuntimeException {

    private final String blockHash;
    private final String expectedHash;
    private final String calculatedHash;

    public BlockchainIntegrityException(String message) {
        super(message);
        this.blockHash = null;
        this.expectedHash = null;
        this.calculatedHash = null;
    }

    public BlockchainIntegrityException(String message, String blockHash, String expectedHash, String calculatedHash) {
        super(String.format("%s - Block: %s, Expected: %s, Calculated: %s",
            message, blockHash, expectedHash, calculatedHash));
        this.blockHash = blockHash;
        this.expectedHash = expectedHash;
        this.calculatedHash = calculatedHash;
    }

    public String getBlockHash() {
        return blockHash;
    }

    public String getExpectedHash() {
        return expectedHash;
    }

    public String getCalculatedHash() {
        return calculatedHash;
    }
}
