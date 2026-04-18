package bf.gov.faso.cache.stream;

import java.util.Map;

/**
 * Callback interface for processing DragonflyDB Stream messages.
 * <p>
 * Implementations handle individual stream records received by a consumer group.
 * Used with {@link StreamOutboxService#readGroup} in a polling loop.
 * <p>
 * Example:
 * <pre>
 * StreamListener listener = (streamKey, recordId, fields) -> {
 *     String type = fields.get("type");
 *     String payload = fields.get("payload");
 *     log.info("Processing event type={} from stream={}", type, streamKey);
 *     // Process the event...
 * };
 * </pre>
 */
@FunctionalInterface
public interface StreamListener {

    /**
     * Called when a stream message is received.
     *
     * @param streamKey the stream key the message was read from
     * @param recordId  the unique record ID assigned by DragonflyDB
     * @param fields    the field-value pairs of the stream record
     */
    void onMessage(String streamKey, String recordId, Map<String, String> fields);
}
