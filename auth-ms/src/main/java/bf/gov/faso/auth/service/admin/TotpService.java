// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.model.TotpEnrollment;
import bf.gov.faso.auth.repository.TotpEnrollmentRepository;
import dev.samstevens.totp.code.CodeGenerator;
import dev.samstevens.totp.code.CodeVerifier;
import dev.samstevens.totp.code.DefaultCodeGenerator;
import dev.samstevens.totp.code.DefaultCodeVerifier;
import dev.samstevens.totp.code.HashingAlgorithm;
import dev.samstevens.totp.qr.QrData;
import dev.samstevens.totp.secret.DefaultSecretGenerator;
import dev.samstevens.totp.secret.SecretGenerator;
import dev.samstevens.totp.time.SystemTimeProvider;
import dev.samstevens.totp.time.TimeProvider;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * TOTP enrolment & verification (RFC 6238) using
 * {@code dev.samstevens.totp}. Secrets are AES-256-GCM-encrypted in DB
 * via {@link bf.gov.faso.auth.persistence.EncryptedStringConverter}.
 */
@Service
public class TotpService {

    private static final Logger log = LoggerFactory.getLogger(TotpService.class);

    private final TotpEnrollmentRepository repo;
    private final AdminAuditService auditService;

    @Value("${admin.totp.issuer:FasoDigitalisation}")
    private String issuer;

    @Value("${admin.totp.window:1}")
    private int verificationWindow;

    private final SecretGenerator secretGenerator = new DefaultSecretGenerator();
    private final TimeProvider timeProvider = new SystemTimeProvider();
    private final CodeGenerator codeGenerator = new DefaultCodeGenerator(HashingAlgorithm.SHA1);
    private final CodeVerifier verifier;

    public TotpService(TotpEnrollmentRepository repo, AdminAuditService auditService) {
        this.repo = repo;
        this.auditService = auditService;
        DefaultCodeVerifier v = new DefaultCodeVerifier(codeGenerator, timeProvider);
        // Allow ±1 step (30 s) drift on either side; configurable via admin.totp.window.
        v.setAllowedTimePeriodDiscrepancy(verificationWindow > 0 ? verificationWindow : 1);
        this.verifier = v;
    }

    /**
     * Begin enrolment — generate a fresh secret and return it together with
     * the {@code otpauth://} URL for QR rendering. Secret is NOT persisted
     * yet; the caller must call {@link #enrollFinish} with a valid 6-digit
     * code to confirm.
     */
    public Map<String, String> enrollBegin(UUID userId, String userEmail) {
        String secret = secretGenerator.generate();
        QrData qrData = new QrData.Builder()
                .label(userEmail)
                .secret(secret)
                .issuer(issuer)
                .algorithm(HashingAlgorithm.SHA1)
                .digits(6)
                .period(30)
                .build();

        Map<String, String> out = new HashMap<>();
        out.put("secret", secret);
        out.put("otpauthUri", qrData.getUri());
        out.put("issuer", issuer);
        log.info("TOTP enroll begin user={} email={}", userId, userEmail);
        return out;
    }

    @Transactional
    public boolean enrollFinish(UUID userId, String secret, String code) {
        if (!verifyCode(secret, code)) {
            auditService.log("totp.enroll.failed", userId, "user:" + userId, null,
                    Map.of("reason", "code-mismatch"), null);
            return false;
        }

        Optional<TotpEnrollment> existing = repo.findByUserId(userId);
        TotpEnrollment enrollment = existing.orElseGet(TotpEnrollment::new);
        enrollment.setUserId(userId);
        enrollment.setSecretEncrypted(secret); // encrypted by converter
        enrollment.setEnrolledAt(Instant.now());
        enrollment.setDisabledAt(null);
        repo.save(enrollment);

        auditService.log("totp.enroll.success", userId, "user:" + userId, null,
                Map.of("issuer", issuer), null);
        log.info("TOTP enrolled user={}", userId);
        return true;
    }

    @Transactional(readOnly = true)
    public boolean verify(UUID userId, String code) {
        Optional<TotpEnrollment> opt = repo.findByUserIdAndDisabledAtIsNull(userId);
        if (opt.isEmpty()) return false;
        boolean ok = verifyCode(opt.get().getSecretEncrypted(), code);
        if (ok) {
            opt.get().setLastUsedAt(Instant.now());
            repo.save(opt.get());
        }
        return ok;
    }

    @Transactional
    public boolean disable(UUID userId, UUID actorId) {
        Optional<TotpEnrollment> opt = repo.findByUserIdAndDisabledAtIsNull(userId);
        if (opt.isEmpty()) return false;
        TotpEnrollment e = opt.get();
        e.setDisabledAt(Instant.now());
        repo.save(e);
        auditService.log("totp.disabled", actorId, "user:" + userId, null,
                Map.of("targetUserId", userId.toString()), null);
        log.info("TOTP disabled user={} by={}", userId, actorId);
        return true;
    }

    private boolean verifyCode(String secret, String code) {
        try {
            return verifier.isValidCode(secret, code);
        } catch (Exception e) {
            log.warn("TOTP verify error: {}", e.getMessage());
            return false;
        }
    }
}
