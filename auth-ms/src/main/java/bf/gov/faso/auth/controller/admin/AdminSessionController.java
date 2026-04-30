// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.service.JtiBlacklistService;
import bf.gov.faso.auth.service.SessionLimitService;
import bf.gov.faso.auth.service.admin.AdminAuditService;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/sessions")
public class AdminSessionController {

    private final SessionLimitService sessionLimitService;
    private final JtiBlacklistService blacklistService;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;
    private final AdminAuthHelper auth;

    public AdminSessionController(SessionLimitService sessionLimitService,
                                  JtiBlacklistService blacklistService,
                                  AdminAuditService auditService,
                                  AdminEventProducer eventProducer,
                                  AdminAuthHelper auth) {
        this.sessionLimitService = sessionLimitService;
        this.blacklistService = blacklistService;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
        this.auth = auth;
    }

    @GetMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<Map<String, Object>> list(@RequestParam UUID userId) {
        return ResponseEntity.ok(Map.of(
                "userId", userId,
                "activeCount", sessionLimitService.getActiveSessionCount(userId)));
    }

    @DeleteMapping("/{jti}")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<Map<String, Object>> revokeOne(
            @PathVariable String jti,
            @RequestParam UUID userId,
            @RequestParam(required = false) String reason) {
        sessionLimitService.removeSession(userId, jti);
        blacklistService.blacklist(jti, reason == null ? "admin-revoke" : reason);
        UUID actor = auth.currentUserId().orElse(null);
        auditService.log("session.revoked", actor, "session:" + jti, null,
                Map.of("userId", userId.toString(), "reason", reason == null ? "" : reason), null);
        eventProducer.publishSessionRevoked(userId, jti, reason);
        return ResponseEntity.ok(Map.of("revoked", true));
    }

    @DeleteMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<Map<String, Object>> revokeAll(@RequestParam UUID userId) {
        sessionLimitService.invalidateAllSessions(userId);
        UUID actor = auth.currentUserId().orElse(null);
        auditService.log("session.revoked.all", actor, "user:" + userId, null,
                Map.of("targetUserId", userId.toString()), null);
        return ResponseEntity.ok(Map.of("revokedAll", true));
    }
}
