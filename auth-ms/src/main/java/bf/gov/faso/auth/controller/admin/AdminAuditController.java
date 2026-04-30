// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.service.admin.AdminSettingsService;
import bf.gov.faso.auth.service.admin.AdminAuditService;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/audit")
public class AdminAuditController {

    private final AdminAuditService auditService;
    private final AdminSettingsService settingsService;

    public AdminAuditController(AdminAuditService auditService, AdminSettingsService settingsService) {
        this.auditService = auditService;
        this.settingsService = settingsService;
    }

    @GetMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<List<Map<String, Object>>> query(
            @RequestParam(required = false) String action,
            @RequestParam(required = false) UUID actorId,
            @RequestParam(required = false) String from,
            @RequestParam(required = false) String to,
            @RequestParam(required = false, defaultValue = "100") int limit) {
        Instant fromI = from == null ? null : Instant.parse(from);
        Instant toI = to == null ? null : Instant.parse(to);
        return ResponseEntity.ok(auditService.query(action, actorId, fromI, toI, limit));
    }

    @GetMapping(value = "/export.csv", produces = "text/csv")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public ResponseEntity<String> exportCsv(
            @RequestParam(required = false) String action,
            @RequestParam(required = false) UUID actorId,
            @RequestParam(required = false) String from,
            @RequestParam(required = false) String to) {
        if (!settingsService.getBool("audit.export_csv_enabled", true)) {
            return ResponseEntity.status(403).body("CSV export disabled by settings");
        }
        Instant fromI = from == null ? null : Instant.parse(from);
        Instant toI = to == null ? null : Instant.parse(to);
        String csv = auditService.exportCsv(action, actorId, fromI, toI);
        return ResponseEntity.ok()
                .contentType(MediaType.parseMediaType("text/csv"))
                .body(csv);
    }
}
