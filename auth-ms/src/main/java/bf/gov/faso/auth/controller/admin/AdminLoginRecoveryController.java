// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.admin.AdminAuditService;
import bf.gov.faso.auth.service.admin.RecoveryCodeService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.time.Duration;
import java.util.Map;
import java.util.UUID;

/**
 * Login-time recovery code consumption (delta amendment 2026-04-30 §4).
 *
 * <p>The user has lost PassKey + TOTP and falls back to a recovery code.
 * Endpoint is wired as {@code permitAll()} (it is part of the AAL2 step of
 * the login flow). Failed attempts are throttled in KAYA at
 * {@code auth:recovery:lock:{email}}.
 */
@RestController
@RequestMapping("/admin/auth/login")
public class AdminLoginRecoveryController {

    /** Bucket TTL — 1h sliding window. */
    private static final Duration LOCK_TTL = Duration.ofHours(1);
    /** Max failed attempts per hour before lockout. */
    private static final int MAX_FAILS = 10;

    private final RecoveryCodeService recoveryCodeService;
    private final UserRepository userRepository;
    private final AdminAuditService auditService;
    private final StringRedisTemplate redis;

    public AdminLoginRecoveryController(RecoveryCodeService recoveryCodeService,
                                        UserRepository userRepository,
                                        AdminAuditService auditService,
                                        StringRedisTemplate redis) {
        this.recoveryCodeService = recoveryCodeService;
        this.userRepository = userRepository;
        this.auditService = auditService;
        this.redis = redis;
    }

    @PostMapping("/recovery-code")
    public ResponseEntity<Map<String, Object>> useRecoveryCode(
            @org.springframework.web.bind.annotation.RequestBody RecoveryCodeRequest req) {
        if (req.email == null || req.email.isBlank() ||
                req.code == null || req.code.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "email + code required");
        }
        UUID userId = userRepository.findByEmail(req.email.trim().toLowerCase())
                .map(u -> u.getId())
                .orElseThrow(() -> new ResponseStatusException(HttpStatus.FORBIDDEN, "invalid"));

        String lockKey = "auth:recovery:lock:" + userId;
        Long fails = redis.opsForValue().get(lockKey) == null
                ? 0L : Long.parseLong(redis.opsForValue().get(lockKey));
        if (fails >= MAX_FAILS) {
            throw new ResponseStatusException(HttpStatus.TOO_MANY_REQUESTS,
                    "too many failed attempts — try again in 1h");
        }

        boolean ok = recoveryCodeService.use(userId, req.code);
        if (!ok) {
            Long current = redis.opsForValue().increment(lockKey);
            if (current != null && current == 1L) redis.expire(lockKey, LOCK_TTL);
            auditService.log(AuditAction.RECOVERY_CODE_INVALID.key(), userId,
                    "user:" + userId, null,
                    Map.of("kratosFlowId", req.kratosFlowId == null ? "" : req.kratosFlowId), null);
            throw new ResponseStatusException(HttpStatus.FORBIDDEN, "code invalid");
        }

        long remaining = recoveryCodeService.countRemaining(userId);
        auditService.log(AuditAction.RECOVERY_CODE_USED.key(), userId,
                "user:" + userId, null,
                Map.of("remaining", remaining,
                        "kratosFlowId", req.kratosFlowId == null ? "" : req.kratosFlowId),
                null);
        // Reset throttle on success.
        redis.delete(lockKey);
        return ResponseEntity.ok(Map.of(
                "ok", true,
                "aal", "AAL2",
                "remaining", remaining,
                "kratosFlowId", req.kratosFlowId == null ? "" : req.kratosFlowId
        ));
    }

    public static class RecoveryCodeRequest {
        @NotBlank public String email;
        @NotBlank public String code;
        public String kratosFlowId;
    }
}
