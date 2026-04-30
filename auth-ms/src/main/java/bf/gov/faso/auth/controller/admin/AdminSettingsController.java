// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.model.AdminSetting;
import bf.gov.faso.auth.model.AdminSettingsHistory;
import bf.gov.faso.auth.service.admin.AdminSettingsService;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/settings")
public class AdminSettingsController {

    private final AdminSettingsService settingsService;
    private final AdminAuthHelper auth;

    public AdminSettingsController(AdminSettingsService settingsService, AdminAuthHelper auth) {
        this.settingsService = settingsService;
        this.auth = auth;
    }

    @GetMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<List<AdminSetting>> getAll(@RequestParam(required = false) String category) {
        return ResponseEntity.ok(category == null
                ? settingsService.getAll()
                : settingsService.getByCategory(category));
    }

    @GetMapping("/{key}")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<AdminSetting> getOne(@PathVariable String key) {
        return settingsService.getByKey(key)
                .map(ResponseEntity::ok)
                .orElse(ResponseEntity.notFound().build());
    }

    @PutMapping("/{key}")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    @RequiresStepUp(
            maxAgeSeconds = 300,
            settingsCategories = {"audit", "mfa", "grant", "break_glass"}
    )
    public ResponseEntity<?> update(
            @PathVariable String key,
            @org.springframework.web.bind.annotation.RequestBody UpdateRequest req,
            @RequestHeader(value = "Idempotency-Key", required = false) String idempotencyKey) {
        if (!auth.acquireIdempotency(idempotencyKey)) {
            return ResponseEntity.status(409).body(Map.of("error", "duplicate idempotency-key"));
        }
        UUID actor = auth.currentUserId().orElseThrow();
        try {
            AdminSetting result = settingsService.update(key, req.value, req.expectedVersion,
                    req.motif, actor);
            return ResponseEntity.ok(result);
        } catch (AdminSettingsService.OptimisticConcurrencyException e) {
            return ResponseEntity.status(HttpStatus.CONFLICT)
                    .body(Map.of("error", "version conflict", "detail", e.getMessage()));
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest()
                    .body(Map.of("error", e.getMessage()));
        }
    }

    @GetMapping("/{key}/history")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public ResponseEntity<List<AdminSettingsHistory>> history(@PathVariable String key) {
        return ResponseEntity.ok(settingsService.getHistory(key));
    }

    @PostMapping("/{key}/revert")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    @RequiresStepUp(
            maxAgeSeconds = 300,
            settingsCategories = {"audit", "mfa", "grant", "break_glass"}
    )
    public ResponseEntity<?> revert(
            @PathVariable String key,
            @org.springframework.web.bind.annotation.RequestBody RevertRequest req) {
        UUID actor = auth.currentUserId().orElseThrow();
        try {
            return ResponseEntity.ok(
                    settingsService.revert(key, req.targetVersion, req.motif, actor));
        } catch (IllegalArgumentException e) {
            return ResponseEntity.badRequest().body(Map.of("error", e.getMessage()));
        }
    }

    public static class UpdateRequest {
        @NotBlank public String value;
        public long expectedVersion;
        @NotBlank public String motif;
    }

    public static class RevertRequest {
        public long targetVersion;
        public String motif;
    }
}
