package bf.gov.faso.cache.lua;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.core.io.ClassPathResource;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.script.RedisScript;
import org.springframework.stereotype.Service;

import java.util.List;
import java.util.UUID;

/**
 * Service for executing atomic Lua scripts on DragonflyDB.
 * <p>
 * Provides:
 * <ul>
 *     <li>{@link #signDocument} — idempotent document signature with fingerprint dedup</li>
 *     <li>{@link #checkRateLimit} — sliding window rate limiter</li>
 * </ul>
 * <p>
 * All scripts are loaded once from classpath and cached by Spring Data Redis.
 */
@Service
public class LuaScriptService {

    private static final Logger log = LoggerFactory.getLogger(LuaScriptService.class);

    private final StringRedisTemplate redisTemplate;
    private final RedisScript<String> signDocumentScript;
    private final RedisScript<String> rateLimitScript;
    private final RedisScript<String> writeBehindDedupScript;

    public LuaScriptService(StringRedisTemplate redisTemplate) {
        this.redisTemplate = redisTemplate;
        this.signDocumentScript = RedisScript.of(
                new ClassPathResource("lua/sign_document.lua"), String.class);
        this.rateLimitScript = RedisScript.of(
                new ClassPathResource("lua/rate_limit.lua"), String.class);
        this.writeBehindDedupScript = RedisScript.of(
                new ClassPathResource("lua/write_behind_dedup.lua"), String.class);
    }

    /**
     * Atomically sign a document with idempotency guarantee.
     * <p>
     * Uses SHA-384 fingerprint dedup set to prevent double-signing.
     * A temporary lock prevents concurrent signature of the same document.
     *
     * @param tenantId    tenant identifier
     * @param payload     JSON payload of the signature event
     * @param fingerprint SHA-384 hex fingerprint of the document
     * @param lockTtl     lock TTL in seconds (e.g., 30)
     * @param docType     document type (e.g., "ACTE_NAISSANCE", "JUGEMENT")
     * @return JSON: {"status":"OK|ALREADY_SIGNED|CONCURRENT_SIGNATURE","stream_id":"..."}
     */
    public String signDocument(String tenantId, String payload, String fingerprint,
                               int lockTtl, String docType) {
        try {
            return redisTemplate.execute(signDocumentScript,
                    List.of(
                            "tenant:" + tenantId + ":stream:signatures",
                            "tenant:" + tenantId + ":signed:fingerprints",
                            "tenant:" + tenantId + ":sign:lock:" + fingerprint
                    ),
                    payload, fingerprint, String.valueOf(lockTtl), docType);
        } catch (Exception e) {
            log.error("Lua signDocument failed [tenant={}, docType={}]: {}",
                    tenantId, docType, e.getMessage(), e);
            throw e;
        }
    }

    /**
     * Atomic write-behind with stream deduplication.
     * <p>
     * Performs 3 operations atomically in a single Lua script:
     * <ol>
     *     <li>SET cache data (latest state wins, with TTL)</li>
     *     <li>SADD entity ID to pending flush set (idempotent)</li>
     *     <li>XADD to persist stream ONLY if this state transition hasn't been
     *         recorded within the dedup window (prevents duplicate audit entries)</li>
     * </ol>
     * <p>
     * This resolves the duplicate stream entries problem when an entity undergoes
     * multiple transitions before the flush scheduler runs, or when HTTP retries
     * cause the same mutation to be applied twice.
     *
     * @param prefix        cache key prefix (e.g., "ec:demande")
     * @param streamKey     persist stream key (e.g., "ec:persist:demande")
     * @param entityId      entity UUID as string
     * @param entityJson    full DTO serialized as JSON
     * @param oldStatut     previous status (for audit trail)
     * @param newStatut     new status (for audit trail + dedup key)
     * @param operateurId   operator performing the transition
     * @param cacheTtlSecs  TTL for cache entry (default 1814400 = 21 days)
     * @param dedupWindowSecs dedup window in seconds (default 5)
     * @return "XADD:{stream_id}" if new stream entry, "DEDUP" if deduplicated
     */
    public String writeBehindDedup(String prefix, String streamKey,
                                   String entityId, String entityJson,
                                   String oldStatut, String newStatut,
                                   String operateurId,
                                   int cacheTtlSecs, int dedupWindowSecs) {
        try {
            return redisTemplate.execute(writeBehindDedupScript,
                    List.of(
                            prefix + ":data:" + entityId,
                            prefix + ":wb:pending",
                            streamKey,
                            prefix + ":dedup:" + entityId + ":" + newStatut
                    ),
                    entityJson, entityId, oldStatut, newStatut,
                    String.valueOf(cacheTtlSecs), String.valueOf(dedupWindowSecs),
                    operateurId);
        } catch (Exception e) {
            log.error("Lua writeBehindDedup failed [entity={}, transition={}→{}]: {}",
                    entityId, oldStatut, newStatut, e.getMessage(), e);
            throw e;
        }
    }

    /**
     * Convenience overload with default TTL (21 days) and dedup window (5 seconds).
     */
    public String writeBehindDedup(String prefix, String streamKey,
                                   String entityId, String entityJson,
                                   String oldStatut, String newStatut,
                                   String operateurId) {
        return writeBehindDedup(prefix, streamKey, entityId, entityJson,
                oldStatut, newStatut, operateurId, 1814400, 5);
    }

    /**
     * Check and enforce a sliding window rate limit.
     * <p>
     * Uses a sorted set with timestamp scores. Expired entries are pruned
     * on each call, providing an accurate sliding window.
     *
     * @param tenantId    tenant identifier
     * @param resource    resource being rate-limited (e.g., "api", "login")
     * @param identity    identity of the caller (e.g., user ID, IP address)
     * @param maxRequests maximum requests allowed in the window
     * @param windowMs    window duration in milliseconds
     * @return JSON: {"status":"OK|RATE_LIMITED","remaining":N,"retry_after_ms":N}
     */
    public String checkRateLimit(String tenantId, String resource, String identity,
                                 int maxRequests, long windowMs) {
        long nowMs = System.currentTimeMillis();
        String requestId = UUID.randomUUID().toString();
        try {
            return redisTemplate.execute(rateLimitScript,
                    List.of("tenant:" + tenantId + ":ratelimit:" + resource + ":" + identity),
                    String.valueOf(maxRequests), String.valueOf(nowMs),
                    String.valueOf(windowMs), requestId);
        } catch (Exception e) {
            log.error("Lua checkRateLimit failed [tenant={}, resource={}, identity={}]: {}",
                    tenantId, resource, identity, e.getMessage(), e);
            throw e;
        }
    }
}
