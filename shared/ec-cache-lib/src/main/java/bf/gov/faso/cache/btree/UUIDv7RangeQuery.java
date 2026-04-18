package bf.gov.faso.cache.btree;

import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalTime;
import java.time.ZoneOffset;
import java.util.UUID;

/**
 * Utility for UUIDv7-based B+Tree range scans.
 * <p>
 * UUIDv7 (RFC 9562) encodes a Unix millisecond timestamp in the upper 48 bits,
 * making primary keys naturally chronologically sortable. This enables efficient
 * B+Tree leaf scans on PK columns without a separate {@code created_at} index.
 * <p>
 * Usage example (JPA / Spring Data):
 * <pre>
 * UUID lower = UUIDv7RangeQuery.lowerBound(Instant.parse("2026-01-01T00:00:00Z"));
 * UUID upper = UUIDv7RangeQuery.upperBound(Instant.now());
 * List&lt;Entity&gt; results = repo.findByIdBetween(lower, upper);
 * </pre>
 * <p>
 * The resulting range scan traverses only the relevant B+Tree leaf pages,
 * avoiding full-table scans.
 */
public final class UUIDv7RangeQuery {

    private UUIDv7RangeQuery() {}

    /**
     * Compute the lower bound UUID for a given timestamp.
     * All UUIDv7 IDs generated at or after {@code from} will be {@code >= lowerBound(from)}.
     *
     * @param from the start of the time range (inclusive)
     * @return a UUID with the timestamp bits set and all random bits zeroed
     */
    public static UUID lowerBound(Instant from) {
        long millis = from.toEpochMilli();
        // UUIDv7: timestamp in upper 48 bits of msb, version nibble = 7
        long msb = (millis << 16) | 0x7000L;  // version 7, random bits = 0
        long lsb = 0x8000_0000_0000_0000L;    // variant 10, random bits = 0
        return new UUID(msb, lsb);
    }

    /**
     * Compute the upper bound UUID for a given timestamp.
     * All UUIDv7 IDs generated at or before {@code to} will be {@code <= upperBound(to)}.
     *
     * @param to the end of the time range (inclusive)
     * @return a UUID with the timestamp bits set and all random bits maximized
     */
    public static UUID upperBound(Instant to) {
        long millis = to.toEpochMilli();
        long msb = (millis << 16) | 0x7FFFL;  // version 7, random bits = max
        long lsb = 0xBFFF_FFFF_FFFF_FFFFL;    // variant 10, random bits = max
        return new UUID(msb, lsb);
    }

    /**
     * Lower bound for the start of a given date (00:00:00 UTC).
     */
    public static UUID lowerBoundForDate(LocalDate date) {
        return lowerBound(date.atStartOfDay().toInstant(ZoneOffset.UTC));
    }

    /**
     * Upper bound for the end of a given date (23:59:59.999 UTC).
     */
    public static UUID upperBoundForDate(LocalDate date) {
        Instant endOfDay = date.atTime(LocalTime.MAX).toInstant(ZoneOffset.UTC);
        return upperBound(endOfDay);
    }

    /**
     * Extract the Unix millisecond timestamp from a UUIDv7.
     *
     * @param uuid a UUIDv7
     * @return the embedded timestamp as an Instant
     * @throws IllegalArgumentException if the UUID is not version 7
     */
    public static Instant extractTimestamp(UUID uuid) {
        if (uuid.version() != 7) {
            throw new IllegalArgumentException("Not a UUIDv7: version=" + uuid.version());
        }
        long msb = uuid.getMostSignificantBits();
        long millis = msb >>> 16;
        return Instant.ofEpochMilli(millis);
    }
}
