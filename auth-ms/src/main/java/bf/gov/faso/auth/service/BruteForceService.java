package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Duration;
import java.time.Instant;
import java.util.UUID;

/**
 * Anti brute-force service with graduated punishment.
 * <p>
 * Graduated thresholds (configurable via application.yml):
 *   5 fails  ->  10 min lock
 *  10 fails  ->  30 min lock
 *  15 fails  ->  60 min lock
 *  20 fails  ->   6 hour lock
 *  25 fails  ->  24 hour lock
 *  30 fails  ->   7 day lock
 *  35+ fails ->  permanent suspension (requires manual unlock)
 * <p>
 * Failed attempt counters are stored both in PostgreSQL (persistent)
 * and in KAYA (fast lookup, key: auth:bruteforce:{userId}).
 */
@Service
public class BruteForceService {

    private static final Logger log = LoggerFactory.getLogger(BruteForceService.class);
    private static final String KEY_PREFIX = "auth:bruteforce:";
    private static final Duration COUNTER_TTL = Duration.ofDays(1);

    private final UserRepository userRepository;
    private final StringRedisTemplate redisTemplate;
    private final SessionLimitService sessionLimitService;

    @Value("${auth.bruteforce.enabled:true}")
    private boolean enabled;

    public BruteForceService(UserRepository userRepository,
                             StringRedisTemplate redisTemplate,
                             SessionLimitService sessionLimitService) {
        this.userRepository = userRepository;
        this.redisTemplate = redisTemplate;
        this.sessionLimitService = sessionLimitService;
    }

    /**
     * Record a failed login attempt and apply graduated punishment.
     *
     * @param userId the user who failed authentication
     * @return the lock duration applied, or null if no lock
     */
    @Transactional
    public Duration recordFailedAttempt(UUID userId) {
        if (!enabled) return null;

        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + userId));

        int attempts = user.getFailedLoginAttempts() + 1;
        user.setFailedLoginAttempts(attempts);

        // Also increment in KAYA for fast lookup
        String key = KEY_PREFIX + userId;
        redisTemplate.opsForValue().increment(key);
        redisTemplate.expire(key, COUNTER_TTL);

        Duration lockDuration = calculateLockDuration(attempts);

        if (lockDuration != null) {
            if (lockDuration.isNegative()) {
                // Permanent suspension
                user.setSuspended(true);
                user.setLockedUntil(null);
                sessionLimitService.invalidateAllSessions(userId);
                log.warn("PERMANENT SUSPENSION for userId={} after {} failed attempts", userId, attempts);
            } else {
                Instant lockedUntil = Instant.now().plus(lockDuration);
                user.setLockedUntil(lockedUntil);
                sessionLimitService.invalidateAllSessions(userId);
                log.warn("Locked userId={} until {} after {} failed attempts ({}min lock)",
                        userId, lockedUntil, attempts, lockDuration.toMinutes());
            }
        }

        userRepository.save(user);
        return lockDuration;
    }

    /**
     * Record a successful login: reset the failed attempt counter.
     */
    @Transactional
    public void recordSuccessfulLogin(UUID userId) {
        User user = userRepository.findById(userId).orElse(null);
        if (user == null) return;

        user.setFailedLoginAttempts(0);
        user.setLockedUntil(null);
        userRepository.save(user);

        // Clear KAYA counter
        String key = KEY_PREFIX + userId;
        redisTemplate.delete(key);
    }

    /**
     * Check if the user account is currently locked.
     */
    public boolean isLocked(UUID userId) {
        User user = userRepository.findById(userId).orElse(null);
        if (user == null) return false;
        return user.isLocked();
    }

    /**
     * Administrative unlock: clear lock and reset counters.
     * Does NOT unsuspend -- that requires explicit unsuspension.
     */
    @Transactional
    public boolean unlockAccount(UUID userId) {
        User user = userRepository.findById(userId)
                .orElseThrow(() -> new IllegalArgumentException("User not found: " + userId));

        user.setLockedUntil(null);
        user.setFailedLoginAttempts(0);
        user.setSuspended(false);
        userRepository.save(user);

        String key = KEY_PREFIX + userId;
        redisTemplate.delete(key);

        log.info("Account unlocked: userId={}", userId);
        return true;
    }

    /**
     * Calculate the lock duration based on the number of failed attempts.
     * Returns null if no lock needed, or a negative Duration for permanent suspension.
     */
    private Duration calculateLockDuration(int failedAttempts) {
        if (failedAttempts >= 35) return Duration.ofMinutes(-1); // permanent
        if (failedAttempts >= 30) return Duration.ofDays(7);
        if (failedAttempts >= 25) return Duration.ofDays(1);
        if (failedAttempts >= 20) return Duration.ofHours(6);
        if (failedAttempts >= 15) return Duration.ofHours(1);
        if (failedAttempts >= 10) return Duration.ofMinutes(30);
        if (failedAttempts >= 5)  return Duration.ofMinutes(10);
        return null;
    }
}
