package bf.gov.faso.auth.service;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.data.redis.core.ZSetOperations;
import org.springframework.stereotype.Service;

import java.time.Duration;
import java.time.Instant;
import java.util.Set;
import java.util.UUID;

/**
 * Session limiting service: max N concurrent sessions per user.
 * <p>
 * Uses KAYA sorted sets: auth:sessions:{userId}
 * Score = session creation timestamp (epoch seconds)
 * Member = JTI of the access token
 * <p>
 * When a new session is created and the count exceeds max-per-user,
 * the oldest sessions are evicted (their JTIs are blacklisted).
 */
@Service
public class SessionLimitService {

    private static final Logger log = LoggerFactory.getLogger(SessionLimitService.class);
    private static final String KEY_PREFIX = "auth:sessions:";
    private static final Duration SESSION_TTL = Duration.ofDays(7);

    private final StringRedisTemplate redisTemplate;
    private final JtiBlacklistService blacklistService;

    @Value("${auth.session.max-per-user:3}")
    private int maxSessionsPerUser;

    public SessionLimitService(StringRedisTemplate redisTemplate, JtiBlacklistService blacklistService) {
        this.redisTemplate = redisTemplate;
        this.blacklistService = blacklistService;
    }

    /**
     * Register a new session for a user. If the user exceeds the max concurrent
     * sessions, the oldest sessions are evicted and their tokens blacklisted.
     *
     * @param userId the user's UUID
     * @param jti    the JWT ID of the new session's access token
     * @return list of evicted JTIs (may be empty)
     */
    public Set<String> registerSession(UUID userId, String jti) {
        String key = KEY_PREFIX + userId;
        double score = Instant.now().getEpochSecond();

        // Add the new session
        redisTemplate.opsForZSet().add(key, jti, score);

        // Clean up expired entries (older than SESSION_TTL)
        double expiredThreshold = Instant.now().minus(SESSION_TTL).getEpochSecond();
        redisTemplate.opsForZSet().removeRangeByScore(key, 0, expiredThreshold);

        // Set TTL on the sorted set key itself
        redisTemplate.expire(key, SESSION_TTL);

        // Check if over the limit
        Long sessionCount = redisTemplate.opsForZSet().zCard(key);
        if (sessionCount != null && sessionCount > maxSessionsPerUser) {
            long toEvict = sessionCount - maxSessionsPerUser;

            // Get the oldest sessions (lowest scores)
            Set<String> oldSessions = redisTemplate.opsForZSet().range(key, 0, toEvict - 1);
            if (oldSessions != null && !oldSessions.isEmpty()) {
                for (String oldJti : oldSessions) {
                    // Blacklist the evicted session's token
                    blacklistService.blacklist(oldJti, "session-limit-exceeded", SESSION_TTL);
                    redisTemplate.opsForZSet().remove(key, oldJti);
                    log.info("Evicted session jti={} for userId={} (exceeded max={})",
                            oldJti, userId, maxSessionsPerUser);
                }
                return oldSessions;
            }
        }

        log.debug("Registered session jti={} for userId={} (total={})", jti, userId, sessionCount);
        return Set.of();
    }

    /**
     * Remove a specific session (on explicit logout).
     */
    public void removeSession(UUID userId, String jti) {
        String key = KEY_PREFIX + userId;
        redisTemplate.opsForZSet().remove(key, jti);
        log.debug("Removed session jti={} for userId={}", jti, userId);
    }

    /**
     * Invalidate all sessions for a user (e.g., password reset, account lock).
     */
    public void invalidateAllSessions(UUID userId) {
        String key = KEY_PREFIX + userId;
        Set<String> allSessions = redisTemplate.opsForZSet().range(key, 0, -1);
        if (allSessions != null) {
            for (String jti : allSessions) {
                blacklistService.blacklist(jti, "all-sessions-invalidated", SESSION_TTL);
            }
        }
        redisTemplate.delete(key);
        log.info("Invalidated all sessions for userId={} (count={})",
                userId, allSessions != null ? allSessions.size() : 0);
    }

    /**
     * Get the number of active sessions for a user.
     */
    public long getActiveSessionCount(UUID userId) {
        String key = KEY_PREFIX + userId;
        Long count = redisTemplate.opsForZSet().zCard(key);
        return count != null ? count : 0;
    }
}
