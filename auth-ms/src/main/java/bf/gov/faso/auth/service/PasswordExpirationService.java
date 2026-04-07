package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;

/**
 * Service for enforcing password expiration (90-day cycle).
 * <p>
 * Responsibilities:
 * - Daily check for passwords approaching expiration (D-14 and D-7 notifications)
 * - Enforce expired passwords by invalidating sessions
 * - Log password expiration events for audit
 * <p>
 * Notifications are logged (actual notification delivery would be handled
 * by a notification-ms in the full architecture).
 */
@Service
public class PasswordExpirationService {

    private static final Logger log = LoggerFactory.getLogger(PasswordExpirationService.class);

    private final UserRepository userRepository;
    private final SessionLimitService sessionLimitService;

    @Value("${auth.password.expiration-days:90}")
    private int expirationDays;

    public PasswordExpirationService(UserRepository userRepository,
                                     SessionLimitService sessionLimitService) {
        this.userRepository = userRepository;
        this.sessionLimitService = sessionLimitService;
    }

    /**
     * Daily check: identify users with passwords expiring in 14 or 7 days.
     * Runs every day at 06:00 UTC.
     */
    @Scheduled(cron = "0 0 6 * * *")
    public void checkExpiringPasswords() {
        Instant now = Instant.now();

        // D-14 notification
        Instant fourteenDaysFromNow = now.plus(14, ChronoUnit.DAYS);
        Instant thirteenDaysFromNow = now.plus(13, ChronoUnit.DAYS);
        List<User> expiringIn14 = userRepository.findUsersWithExpiringPasswords(fourteenDaysFromNow);
        List<User> filtered14 = expiringIn14.stream()
                .filter(u -> u.getPasswordExpiresAt().isAfter(thirteenDaysFromNow))
                .toList();

        for (User user : filtered14) {
            log.warn("PASSWORD EXPIRING IN 14 DAYS: userId={} email={} expiresAt={}",
                    user.getId(), user.getEmail(), user.getPasswordExpiresAt());
            // In production, this would dispatch an event to notification-ms
        }

        // D-7 notification
        Instant sevenDaysFromNow = now.plus(7, ChronoUnit.DAYS);
        Instant sixDaysFromNow = now.plus(6, ChronoUnit.DAYS);
        List<User> expiringIn7 = userRepository.findUsersWithExpiringPasswords(sevenDaysFromNow);
        List<User> filtered7 = expiringIn7.stream()
                .filter(u -> u.getPasswordExpiresAt().isAfter(sixDaysFromNow))
                .toList();

        for (User user : filtered7) {
            log.warn("PASSWORD EXPIRING IN 7 DAYS: userId={} email={} expiresAt={}",
                    user.getId(), user.getEmail(), user.getPasswordExpiresAt());
        }

        if (!filtered14.isEmpty() || !filtered7.isEmpty()) {
            log.info("Password expiration check: {} users at D-14, {} users at D-7",
                    filtered14.size(), filtered7.size());
        }
    }

    /**
     * Daily check: enforce expired passwords by invalidating sessions.
     * Runs every day at 06:30 UTC, after the notification check.
     */
    @Scheduled(cron = "0 30 6 * * *")
    @Transactional
    public void enforceExpiredPasswords() {
        Instant now = Instant.now();
        List<User> expired = userRepository.findUsersWithExpiredPasswords(now);

        for (User user : expired) {
            // Invalidate all sessions for users with expired passwords
            sessionLimitService.invalidateAllSessions(user.getId());
            log.warn("ENFORCED PASSWORD EXPIRATION: userId={} email={} expiredAt={}",
                    user.getId(), user.getEmail(), user.getPasswordExpiresAt());
        }

        if (!expired.isEmpty()) {
            log.info("Password expiration enforcement: {} users had expired passwords", expired.size());
        }
    }

    /**
     * Reset password expiration for a user (called after successful password change).
     */
    @Transactional
    public void resetPasswordExpiration(User user) {
        Instant now = Instant.now();
        user.setPasswordChangedAt(now);
        user.setPasswordExpiresAt(now.plus(expirationDays, ChronoUnit.DAYS));
        userRepository.save(user);
        log.info("Password expiration reset for userId={}, new expiry={}",
                user.getId(), user.getPasswordExpiresAt());
    }
}
