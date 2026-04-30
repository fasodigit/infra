// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.model.AdminRoleGrant;
import bf.gov.faso.auth.service.admin.AdminRoleGrantService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;

@RestController
@RequestMapping("/admin/grants")
public class AdminRoleGrantController {

    private final AdminRoleGrantService grantService;
    private final AdminAuthHelper auth;

    public AdminRoleGrantController(AdminRoleGrantService grantService, AdminAuthHelper auth) {
        this.grantService = grantService;
        this.auth = auth;
    }

    @PostMapping("/request")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    @RequiresStepUp(maxAgeSeconds = 300)
    public ResponseEntity<AdminRoleGrant> request(
            @org.springframework.web.bind.annotation.RequestBody RequestGrantRequest req,
            @RequestHeader(value = "Idempotency-Key", required = false) String idempotencyKey) {
        if (!auth.acquireIdempotency(idempotencyKey)) {
            return ResponseEntity.status(409).build();
        }
        UUID grantor = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(grantService.requestGrant(grantor, req.granteeId,
                req.roleId, req.justification, req.capabilities));
    }

    @PostMapping("/{grantId}/approve")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    @RequiresStepUp(maxAgeSeconds = 300)
    public ResponseEntity<AdminRoleGrant> approve(
            @PathVariable UUID grantId,
            @org.springframework.web.bind.annotation.RequestBody(required = false) ApproveRequest req) {
        UUID approver = auth.currentUserId().orElseThrow();
        Set<String> caps = req == null ? null : req.capabilities;
        return ResponseEntity.ok(grantService.approveGrant(grantId, approver, caps));
    }

    @PostMapping("/{grantId}/reject")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    @RequiresStepUp(maxAgeSeconds = 300)
    public ResponseEntity<AdminRoleGrant> reject(
            @PathVariable UUID grantId,
            @org.springframework.web.bind.annotation.RequestBody RejectRequest req) {
        UUID approver = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(grantService.rejectGrant(grantId, approver, req.reason));
    }

    @GetMapping("/pending")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public ResponseEntity<List<AdminRoleGrant>> pending() {
        UUID approver = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(grantService.listPendingForApprover(approver));
    }

    @GetMapping("/grantee/{granteeId}")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<List<AdminRoleGrant>> forGrantee(@PathVariable UUID granteeId) {
        return ResponseEntity.ok(grantService.listForGrantee(granteeId));
    }

    public static class RequestGrantRequest {
        public UUID granteeId;
        public UUID roleId;
        @NotBlank public String justification;
        /** Delta amendment 2026-04-30 — fine-grained capability set. */
        public Set<String> capabilities;
    }

    public static class ApproveRequest {
        /** Delta amendment 2026-04-30 — capability set committed at approval. */
        public Set<String> capabilities;
    }

    public static class RejectRequest {
        public String reason;
    }
}
