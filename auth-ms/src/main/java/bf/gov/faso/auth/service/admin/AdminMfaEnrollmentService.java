// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.model.MfaStatus;
import bf.gov.faso.auth.repository.DeviceRegistrationRepository;
import bf.gov.faso.auth.repository.MfaStatusRepository;
import bf.gov.faso.auth.repository.RecoveryCodeRepository;
import bf.gov.faso.auth.repository.TotpEnrollmentRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Orchestrates the MFA enrolment flow:
 * OTP (proof of identity) -> PassKey OR TOTP -> 10 backup recovery codes.
 * <p>
 * Materialises {@code mfa_status} after each transition so the dashboard
 * stays cheap to query.
 */
@Service
public class AdminMfaEnrollmentService {

    private static final Logger log = LoggerFactory.getLogger(AdminMfaEnrollmentService.class);

    private final OtpService otpService;
    private final TotpService totpService;
    private final WebAuthnService webAuthnService;
    private final RecoveryCodeService recoveryCodeService;
    private final TotpEnrollmentRepository totpRepo;
    private final RecoveryCodeRepository recoveryRepo;
    private final DeviceRegistrationRepository deviceRepo;
    private final MfaStatusRepository statusRepo;
    private final AdminAuditService auditService;

    public AdminMfaEnrollmentService(OtpService otpService,
                                     TotpService totpService,
                                     WebAuthnService webAuthnService,
                                     RecoveryCodeService recoveryCodeService,
                                     TotpEnrollmentRepository totpRepo,
                                     RecoveryCodeRepository recoveryRepo,
                                     DeviceRegistrationRepository deviceRepo,
                                     MfaStatusRepository statusRepo,
                                     AdminAuditService auditService) {
        this.otpService = otpService;
        this.totpService = totpService;
        this.webAuthnService = webAuthnService;
        this.recoveryCodeService = recoveryCodeService;
        this.totpRepo = totpRepo;
        this.recoveryRepo = recoveryRepo;
        this.deviceRepo = deviceRepo;
        this.statusRepo = statusRepo;
        this.auditService = auditService;
    }

    /**
     * Step 1: ask the user for an OTP (sent over their primary email).
     */
    public String beginEnrollment(UUID userId) {
        return otpService.issue(userId, "ENROLLMENT");
    }

    /**
     * Step 3 (after PassKey or TOTP succeeded): provision recovery codes and
     * refresh {@code mfa_status}.
     */
    @Transactional
    public List<String> finalizeEnrollment(UUID userId, String motif) {
        List<String> codes = recoveryCodeService.generate(userId, motif);
        recomputeStatus(userId);
        auditService.log("mfa.enrollment.finalized", userId, "user:" + userId, null,
                Map.of("recoveryCodesCount", codes.size()), null);
        return codes;
    }

    @Transactional
    public MfaStatus recomputeStatus(UUID userId) {
        MfaStatus status = statusRepo.findById(userId).orElse(new MfaStatus(userId));
        status.setTotpEnabled(totpRepo.findByUserIdAndDisabledAtIsNull(userId).isPresent());
        status.setBackupCodesRemaining((int) recoveryRepo.countUnusedByUserId(userId));
        status.setTrustedDevicesCount(
                (int) deviceRepo.countByUserIdAndTrustedAtIsNotNullAndRevokedAtIsNull(userId));
        // PassKey count comes from WebAuthnService (in-memory map fallback).
        status.setPasskeyCount(webAuthnService.countCredentials(userId));
        status.setUpdatedAt(Instant.now());
        return statusRepo.save(status);
    }

    public MfaStatus getOrCreate(UUID userId) {
        return statusRepo.findById(userId).orElseGet(() -> {
            log.info("Creating fresh mfa_status row for user={}", userId);
            return statusRepo.save(new MfaStatus(userId));
        });
    }
}
