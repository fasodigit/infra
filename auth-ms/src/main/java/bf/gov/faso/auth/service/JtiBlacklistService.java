package bf.gov.faso.auth.service;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.time.Duration;

/**
 * JWT revocation service using KAYA (Redis-compatible) as the blacklist store.
 * <p>
 * When a token is blacklisted, its JTI is stored in KAYA with a TTL matching
 * the token's remaining lifetime. ARMAGEDDON queries this on the management
 * plane when needed; for hot-path validation, ARMAGEDDON uses its own local
 * JWKS cache + jwt_authn filter.
 * <p>
 * Key format: auth:jti:blacklist:{jti}
 * Value: reason string or "revoked"
 * TTL: remaining token lifetime (max 24h for access tokens, 7d for refresh)
 */
@Service
public class JtiBlacklistService {

    private static final Logger log = LoggerFactory.getLogger(JtiBlacklistService.class);
    private static final String KEY_PREFIX = "auth:jti:blacklist:";
    private static final Duration DEFAULT_TTL = Duration.ofHours(24);

    private final StringRedisTemplate redisTemplate;

    public JtiBlacklistService(StringRedisTemplate redisTemplate) {
        this.redisTemplate = redisTemplate;
    }

    /**
     * Blacklist a JWT by its JTI claim.
     *
     * @param jti    the JWT ID to blacklist
     * @param reason human-readable reason for the blacklisting
     * @param ttl    how long to keep the entry (should match token's remaining lifetime)
     */
    public void blacklist(String jti, String reason, Duration ttl) {
        String key = KEY_PREFIX + jti;
        String value = (reason != null && !reason.isBlank()) ? reason : "revoked";
        Duration effectiveTtl = (ttl != null && !ttl.isNegative() && !ttl.isZero()) ? ttl : DEFAULT_TTL;

        redisTemplate.opsForValue().set(key, value, effectiveTtl);
        log.info("Blacklisted JTI={} reason='{}' ttl={}s", jti, value, effectiveTtl.toSeconds());
    }

    /**
     * Blacklist with default TTL (24 hours).
     */
    public void blacklist(String jti, String reason) {
        blacklist(jti, reason, DEFAULT_TTL);
    }

    /**
     * Check if a JTI is blacklisted.
     *
     * @param jti the JWT ID to check
     * @return true if the JTI is in the blacklist
     */
    public boolean isBlacklisted(String jti) {
        String key = KEY_PREFIX + jti;
        Boolean exists = redisTemplate.hasKey(key);
        return Boolean.TRUE.equals(exists);
    }

    /**
     * Remove a JTI from the blacklist (administrative action).
     */
    public void remove(String jti) {
        String key = KEY_PREFIX + jti;
        Boolean deleted = redisTemplate.delete(key);
        if (Boolean.TRUE.equals(deleted)) {
            log.info("Removed JTI={} from blacklist", jti);
        }
    }

    /**
     * Get the blacklist reason for a JTI, if it exists.
     */
    public String getBlacklistReason(String jti) {
        String key = KEY_PREFIX + jti;
        return redisTemplate.opsForValue().get(key);
    }
}
