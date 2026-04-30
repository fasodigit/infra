// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.model.RecoveryCode;
import bf.gov.faso.auth.repository.RecoveryCodeRepository;
import bf.gov.faso.auth.service.crypto.Argon2idCryptographicHashService;
import bf.gov.faso.auth.service.crypto.CryptographicHashService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.security.SecureRandom;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Single-use MFA recovery codes (XXXX-XXXX format).
 *
 * <p>Phase 4.b.3 — codes are now hashed with {@code Argon2id(HMAC-SHA256(pepper, code))}
 * via {@link CryptographicHashService}. The pepper lives in Vault under
 * {@code faso/auth-ms/recovery-pepper-v{N}} and is rotated independently of
 * the Argon2 parameters. Existing bcrypt hashes are not retroactively
 * migrated (no plaintext available) — they are forward-only invalidated next
 * time the user regenerates their recovery batch.
 */
@Service
public class RecoveryCodeService {

    private static final Logger log = LoggerFactory.getLogger(RecoveryCodeService.class);
    private static final SecureRandom RNG = new SecureRandom();
    private static final char[] ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".toCharArray();
    private static final int CODE_COUNT = 10;

    private final RecoveryCodeRepository repo;
    private final AdminAuditService auditService;
    private final AdminSettingsService settingsService;
    private final CryptographicHashService cryptoHashService;
    private final Argon2idCryptographicHashService argon2Service;

    public RecoveryCodeService(RecoveryCodeRepository repo,
                               AdminAuditService auditService,
                               AdminSettingsService settingsService,
                               CryptographicHashService cryptoHashService,
                               Argon2idCryptographicHashService argon2Service) {
        this.repo = repo;
        this.auditService = auditService;
        this.settingsService = settingsService;
        this.cryptoHashService = cryptoHashService;
        this.argon2Service = argon2Service;
    }

    /**
     * Generate {@value #CODE_COUNT} new codes and invalidate the previous batch.
     * Returns the plain codes — they are shown ONCE and never re-displayed.
     */
    @Transactional
    public List<String> generate(UUID userId, String motif) {
        // Invalidate prior un-used codes for this user.
        List<RecoveryCode> existing = repo.findUnusedByUserId(userId);
        Instant now = Instant.now();
        for (RecoveryCode rc : existing) {
            rc.setUsedAt(now);
        }
        repo.saveAll(existing);

        int count = settingsService.getInt("mfa.recovery_codes_count", CODE_COUNT);
        int ttlDays = settingsService.getInt("mfa.recovery_codes_ttl_days", 365);
        Instant expiresAt = now.plus(ttlDays, ChronoUnit.DAYS);

        List<String> plainCodes = new ArrayList<>(count);
        List<RecoveryCode> entities = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            String code = generateCode();
            plainCodes.add(code);
            entities.add(new RecoveryCode(userId, cryptoHashService.hashRecoveryCode(code),
                    motif, expiresAt));
        }
        repo.saveAll(entities);

        auditService.log("recovery_codes.generated", userId, "user:" + userId, null,
                Map.of("count", count, "motif", motif), null);
        log.info("Generated {} recovery codes user={} motif={}", count, userId, motif);
        return plainCodes;
    }

    /**
     * Attempt to consume a code. Walks all unused codes and matches via
     * {@link CryptographicHashService#verifyRecoveryCode}.
     */
    @Transactional
    public boolean use(UUID userId, String candidate) {
        if (candidate == null || candidate.isBlank()) return false;
        String normalised = candidate.trim().toUpperCase();
        List<RecoveryCode> unused = repo.findUnusedByUserId(userId);

        int pepperVersion = argon2Service.currentPepperVersion();
        for (RecoveryCode rc : unused) {
            if (rc.isExpired()) continue;
            if (cryptoHashService.verifyRecoveryCode(normalised, rc.getCodeHash(), pepperVersion)) {
                rc.setUsedAt(Instant.now());
                repo.save(rc);
                auditService.log("recovery_code.used", userId, "user:" + userId, null,
                        Map.of("codeId", rc.getId().toString()), null);
                log.info("Recovery code used user={} codeId={}", userId, rc.getId());
                return true;
            }
        }

        auditService.log("recovery_code.use.failed", userId, "user:" + userId, null,
                Map.of("reason", "no-match"), null);
        return false;
    }

    public long countRemaining(UUID userId) {
        return repo.countUnusedByUserId(userId);
    }

    private String generateCode() {
        char[] buf = new char[9];
        for (int i = 0; i < 4; i++) buf[i] = ALPHABET[RNG.nextInt(ALPHABET.length)];
        buf[4] = '-';
        for (int i = 5; i < 9; i++) buf[i] = ALPHABET[RNG.nextInt(ALPHABET.length)];
        return new String(buf);
    }
}
