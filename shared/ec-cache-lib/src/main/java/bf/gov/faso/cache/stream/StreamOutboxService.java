package bf.gov.faso.cache.stream;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.connection.stream.Consumer;
import org.springframework.data.redis.connection.stream.MapRecord;
import org.springframework.data.redis.connection.stream.ReadOffset;
import org.springframework.data.redis.connection.stream.RecordId;
import org.springframework.data.redis.connection.stream.StreamOffset;
import org.springframework.data.redis.connection.stream.StreamRecords;
import org.springframework.data.redis.core.RedisCallback;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.connection.stream.StreamReadOptions;

import java.time.Duration;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Service for publishing events to and managing DragonflyDB Streams.
 * <p>
 * The stream outbox pattern enables async event processing:
 * producers XADD events atomically, and consumer groups process them
 * asynchronously for durable persistence or downstream propagation.
 * <p>
 * Backed by DragonflyDB (Redis-compatible) Stream commands:
 * XADD, XGROUP CREATE, XREADGROUP, XACK, XTRIM.
 */
public class StreamOutboxService {

    private static final Logger log = LoggerFactory.getLogger(StreamOutboxService.class);

    private final StringRedisTemplate redisTemplate;
    private final StreamConsumerProperties properties;

    public StreamOutboxService(StringRedisTemplate redisTemplate,
                               StreamConsumerProperties properties) {
        this.redisTemplate = redisTemplate;
        this.properties = properties;
    }

    /**
     * Publishes an event to a DragonflyDB stream via XADD.
     *
     * @param streamKey the stream key (e.g. "ec:outbox:demande")
     * @param fields    the field-value pairs for the stream record
     * @return the RecordId assigned by DragonflyDB
     */
    public RecordId publish(String streamKey, Map<String, String> fields) {
        MapRecord<String, String, String> record = StreamRecords
                .string(fields)
                .withStreamKey(streamKey);

        RecordId id = redisTemplate.opsForStream().add(record);
        log.debug("Published to stream {}: id={}, fields={}", streamKey, id, fields.keySet());
        return id;
    }

    /**
     * Creates a consumer group on the stream (idempotent — catches BUSYGROUP).
     *
     * @param streamKey the stream key
     * @param groupName the consumer group name
     */
    public void createConsumerGroup(String streamKey, String groupName) {
        try {
            redisTemplate.opsForStream().createGroup(streamKey, ReadOffset.from("0"), groupName);
            log.info("Created consumer group '{}' on stream '{}'", groupName, streamKey);
        } catch (Exception e) {
            if (e.getMessage() != null && e.getMessage().contains("BUSYGROUP")) {
                log.debug("Consumer group '{}' already exists on stream '{}'", groupName, streamKey);
            } else {
                throw e;
            }
        }
    }

    /**
     * Reads pending messages from a consumer group via XREADGROUP.
     *
     * @param streamKey    the stream key
     * @param groupName    the consumer group name
     * @param consumerName the consumer name within the group
     * @param count        max number of messages to read
     * @return list of stream records, or empty list on timeout/error
     */
    public List<MapRecord<String, String, String>> readGroup(String streamKey,
                                                              String groupName,
                                                              String consumerName,
                                                              int count) {
        try {
            StreamReadOptions options = StreamReadOptions.empty()
                    .count(count)
                    .block(Duration.ofMillis(properties.getBlockTimeoutMs()));

            var raw = redisTemplate.opsForStream().read(
                    Consumer.from(groupName, consumerName),
                    options,
                    StreamOffset.create(streamKey, ReadOffset.lastConsumed())
            );
            return raw != null ? toStringRecords(raw) : List.of();
        } catch (Exception e) {
            log.warn("Stream readGroup failed [stream={}, group={}, consumer={}]: {}",
                    streamKey, groupName, consumerName, e.getMessage());
            return List.of();
        }
    }

    /**
     * Acknowledges processed records in the consumer group via XACK.
     *
     * @param streamKey the stream key
     * @param groupName the consumer group name
     * @param ids       the record IDs to acknowledge
     */
    public void acknowledge(String streamKey, String groupName, RecordId... ids) {
        try {
            String[] idStrings = new String[ids.length];
            for (int i = 0; i < ids.length; i++) {
                idStrings[i] = ids[i].getValue();
            }
            redisTemplate.opsForStream().acknowledge(streamKey, groupName, idStrings);
            log.debug("Acknowledged {} records in group '{}' on stream '{}'",
                    ids.length, groupName, streamKey);
        } catch (Exception e) {
            log.warn("Stream acknowledge failed [stream={}, group={}]: {}",
                    streamKey, groupName, e.getMessage());
        }
    }

    /**
     * Trims the stream to a maximum length via XTRIM.
     * <p>
     * Uses approximate trimming (~maxLen) for performance.
     *
     * @param streamKey the stream key
     * @param maxLen    maximum number of entries to retain
     */
    public void trimStream(String streamKey, long maxLen) {
        try {
            redisTemplate.opsForStream().trim(streamKey, maxLen);
            log.debug("Trimmed stream '{}' to maxLen={}", streamKey, maxLen);
        } catch (Exception e) {
            log.warn("Stream trim failed [stream={}, maxLen={}]: {}",
                    streamKey, maxLen, e.getMessage());
        }
    }

    /**
     * Trims the stream by minimum ID via XTRIM MINID.
     * <p>
     * Removes all entries with IDs older than the given minimum ID.
     * DragonflyDB stream IDs use the format {@code <timestamp_ms>-<seq>},
     * so passing a timestamp-based ID removes entries older than that time.
     *
     * @param streamKey the stream key
     * @param minId     the minimum entry ID to retain (e.g. "1710288000000-0")
     */
    public void trimByMinId(String streamKey, String minId) {
        try {
            // XTRIM <key> MINID ~ <minId> — Spring Data Redis doesn't expose MINID directly
            redisTemplate.execute((RedisCallback<Object>) connection -> {
                connection.execute("XTRIM", streamKey.getBytes(), "MINID".getBytes(),
                        "~".getBytes(), minId.getBytes());
                return null;
            });
            log.debug("Trimmed stream '{}' with MINID ~{}", streamKey, minId);
        } catch (Exception e) {
            log.warn("Stream trimByMinId failed [stream={}, minId={}]: {}",
                    streamKey, minId, e.getMessage());
        }
    }

    /**
     * Reads all entries from a stream within a range via XRANGE.
     *
     * @param streamKey the stream key
     * @param start     start ID (inclusive), use "-" for beginning
     * @param end       end ID (inclusive), use "+" for latest
     * @return list of stream records, or empty list on error
     */
    public List<MapRecord<String, String, String>> range(String streamKey, String start, String end) {
        try {
            var range = org.springframework.data.domain.Range.closed(start, end);
            var raw = redisTemplate.opsForStream().range(streamKey, range);
            return raw != null ? toStringRecords(raw) : List.of();
        } catch (Exception e) {
            log.warn("Stream range failed [stream={}, start={}, end={}]: {}",
                    streamKey, start, end, e.getMessage());
            return List.of();
        }
    }

    /**
     * Returns the number of entries in the stream via XLEN.
     *
     * @param streamKey the stream key
     * @return the stream length, or 0 on error
     */
    public long streamLength(String streamKey) {
        try {
            Long len = redisTemplate.opsForStream().size(streamKey);
            return len != null ? len : 0L;
        } catch (Exception e) {
            log.warn("Stream length failed [stream={}]: {}", streamKey, e.getMessage());
            return 0L;
        }
    }

    /**
     * Returns the configured stream consumer properties.
     */
    public StreamConsumerProperties getProperties() {
        return properties;
    }

    /**
     * Converts raw {@code MapRecord<String, Object, Object>} records
     * to {@code MapRecord<String, String, String>} by casting values to strings.
     */
    private static List<MapRecord<String, String, String>> toStringRecords(
            List<? extends MapRecord<String, ?, ?>> raw) {
        return raw.stream()
                .map(record -> {
                    Map<String, String> stringMap = new LinkedHashMap<>();
                    record.getValue().forEach((k, v) ->
                            stringMap.put(String.valueOf(k), v != null ? String.valueOf(v) : ""));
                    return StreamRecords.string(stringMap).withStreamKey(record.getStream())
                            .withId(record.getId());
                })
                .map(r -> (MapRecord<String, String, String>) r)
                .toList();
    }
}
