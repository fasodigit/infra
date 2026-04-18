package bf.gov.faso.cache.stream;

import java.time.Instant;
import java.util.HashMap;
import java.util.Map;

/**
 * Immutable record representing a single workflow step in a demande's lifecycle.
 * <p>
 * Stored as a DragonflyDB Stream entry with field-value pairs.
 * Only workflow-essential fields — no PII, no form data.
 *
 * @param step           workflow step (SOUMIS, VERIFIE, VALIDE, IMPRIME, etc.)
 * @param previousStatus previous status before this transition
 * @param operatorId     UUID of the agent who performed this step (null for SOUMIS)
 * @param operatorName   display name of the agent
 * @param comment        rejection reason or operator comment (nullable)
 * @param numeroActe     official act number, set from VALIDE onward (nullable)
 * @param documentHash   SHA-256 of generated PDF, set from IMPRIME onward (nullable)
 * @param durationMs     processing duration for this step in milliseconds
 * @param timestamp      when this step occurred (ISO-8601)
 */
public record WorkflowStepEntry(
        String step,
        String previousStatus,
        String operatorId,
        String operatorName,
        String comment,
        String numeroActe,
        String documentHash,
        long durationMs,
        Instant timestamp
) {

    /**
     * Creates a minimal entry for status transitions without operator context.
     */
    public static WorkflowStepEntry of(String step, String previousStatus) {
        return new WorkflowStepEntry(step, previousStatus, null, null, null, null, null, 0, Instant.now());
    }

    /**
     * Creates an entry with operator context.
     */
    public static WorkflowStepEntry of(String step, String previousStatus, String operatorId, String operatorName) {
        return new WorkflowStepEntry(step, previousStatus, operatorId, operatorName, null, null, null, 0, Instant.now());
    }

    /**
     * Serializes this entry to a flat Map for DragonflyDB Stream XADD.
     * Null fields are omitted to minimize storage.
     */
    public Map<String, String> toMap() {
        var map = new HashMap<String, String>(10);
        map.put("step", step);
        if (previousStatus != null) map.put("previousStatus", previousStatus);
        if (operatorId != null) map.put("operatorId", operatorId);
        if (operatorName != null) map.put("operatorName", operatorName);
        if (comment != null) map.put("comment", comment);
        if (numeroActe != null) map.put("numeroActe", numeroActe);
        if (documentHash != null) map.put("documentHash", documentHash);
        if (durationMs > 0) map.put("durationMs", String.valueOf(durationMs));
        map.put("timestamp", timestamp != null ? timestamp.toString() : Instant.now().toString());
        return map;
    }

    /**
     * Deserializes a Stream record's field map back into a WorkflowStepEntry.
     */
    public static WorkflowStepEntry fromMap(Map<String, String> fields) {
        return new WorkflowStepEntry(
                fields.getOrDefault("step", "UNKNOWN"),
                fields.get("previousStatus"),
                fields.get("operatorId"),
                fields.get("operatorName"),
                fields.get("comment"),
                fields.get("numeroActe"),
                fields.get("documentHash"),
                parseLong(fields.get("durationMs")),
                parseInstant(fields.get("timestamp"))
        );
    }

    private static long parseLong(String value) {
        if (value == null || value.isEmpty()) return 0;
        try {
            return Long.parseLong(value);
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    private static Instant parseInstant(String value) {
        if (value == null || value.isEmpty()) return Instant.now();
        try {
            return Instant.parse(value);
        } catch (Exception e) {
            return Instant.now();
        }
    }
}
