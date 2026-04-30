// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.DeviceRegistration;
import bf.gov.faso.auth.repository.DeviceRegistrationRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Duration;
import java.time.Instant;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Trusted-device registry. Persists fingerprints in PostgreSQL and mirrors
 * the trust-decision in KAYA at {@code dev:{userId}:{fp}} with TTL =
 * {@code device_trust.ttl_days} from admin_settings.
 */
@Service
public class DeviceTrustService {

    private static final Logger log = LoggerFactory.getLogger(DeviceTrustService.class);
    private static final String KAYA_PREFIX = "dev:";

    private final DeviceRegistrationRepository repo;
    private final StringRedisTemplate redis;
    private final AdminSettingsService settingsService;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;

    public DeviceTrustService(DeviceRegistrationRepository repo,
                              StringRedisTemplate redis,
                              AdminSettingsService settingsService,
                              AdminAuditService auditService,
                              AdminEventProducer eventProducer) {
        this.repo = repo;
        this.redis = redis;
        this.settingsService = settingsService;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
    }

    /**
     * Compute the SHA-256 fingerprint from UA + IP/24 + Accept-Language.
     */
    public String computeFingerprint(String userAgent, String ipAddress, String acceptLanguage) {
        String ipPrefix = truncateIpToSlash24(ipAddress);
        String input = (userAgent == null ? "" : userAgent) + "|" +
                ipPrefix + "|" +
                (acceptLanguage == null ? "" : acceptLanguage);
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] digest = md.digest(input.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(digest);
        } catch (Exception e) {
            throw new IllegalStateException("SHA-256 unavailable", e);
        }
    }

    @Transactional
    public DeviceRegistration register(UUID userId, String userAgent,
                                       String ipAddress, String acceptLanguage,
                                       String deviceType) {
        String fp = computeFingerprint(userAgent, ipAddress, acceptLanguage);
        Optional<DeviceRegistration> existing = repo.findByUserIdAndFingerprint(userId, fp);
        DeviceRegistration reg = existing.orElseGet(
                () -> new DeviceRegistration(userId, fp, userAgent, ipAddress));
        reg.setDeviceType(deviceType);
        reg.setUaString(userAgent);
        reg.setIpAddress(ipAddress);
        reg.setLastUsedAt(Instant.now());
        if (reg.getRevokedAt() != null) {
            reg.setRevokedAt(null);
        }
        DeviceRegistration saved = repo.save(reg);
        log.info("Device registered user={} fingerprint={} (id={})", userId, fp, saved.getId());
        return saved;
    }

    @Transactional
    public DeviceRegistration trust(UUID userId, UUID deviceId, UUID actorId) {
        DeviceRegistration reg = repo.findById(deviceId)
                .orElseThrow(() -> new IllegalArgumentException("device not found: " + deviceId));
        if (!reg.getUserId().equals(userId)) {
            throw new IllegalArgumentException("device/user mismatch");
        }
        reg.setTrustedAt(Instant.now());
        reg.setRevokedAt(null);
        repo.save(reg);

        long ttlDays = settingsService.getInt("device_trust.ttl_days", 30);
        redis.opsForValue().set(KAYA_PREFIX + userId + ":" + reg.getFingerprint(),
                "trusted", Duration.ofDays(ttlDays));

        eventProducer.publishDeviceTrusted(userId, reg.getFingerprint());
        auditService.log("device.trusted", actorId, "device:" + deviceId, null,
                Map.of("userId", userId.toString(), "fingerprint", reg.getFingerprint()), null);
        return reg;
    }

    @Transactional
    public boolean revoke(UUID userId, UUID deviceId, UUID actorId) {
        Optional<DeviceRegistration> opt = repo.findById(deviceId);
        if (opt.isEmpty() || !opt.get().getUserId().equals(userId)) return false;
        DeviceRegistration reg = opt.get();
        reg.setRevokedAt(Instant.now());
        repo.save(reg);
        redis.delete(KAYA_PREFIX + userId + ":" + reg.getFingerprint());
        auditService.log("device.revoked", actorId, "device:" + deviceId, null,
                Map.of("userId", userId.toString()), null);
        return true;
    }

    public boolean isTrusted(UUID userId, String fingerprint) {
        Boolean kayaHit = redis.hasKey(KAYA_PREFIX + userId + ":" + fingerprint);
        if (Boolean.TRUE.equals(kayaHit)) return true;
        return repo.findByUserIdAndFingerprint(userId, fingerprint)
                .map(DeviceRegistration::isTrusted)
                .orElse(false);
    }

    public List<DeviceRegistration> listForUser(UUID userId) {
        return repo.findByUserIdAndRevokedAtIsNull(userId);
    }

    private String truncateIpToSlash24(String ip) {
        if (ip == null) return "";
        // IPv4 / 24
        int lastDot = ip.lastIndexOf('.');
        if (lastDot > 0 && ip.indexOf(':') < 0) {
            return ip.substring(0, lastDot) + ".0";
        }
        // IPv6: keep first 64 bits.
        if (ip.indexOf(':') >= 0) {
            String[] parts = ip.split(":");
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < Math.min(4, parts.length); i++) {
                if (i > 0) sb.append(':');
                sb.append(parts[i]);
            }
            return sb.toString();
        }
        return ip;
    }
}
