// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.model.CapabilityRegistry;
import bf.gov.faso.auth.repository.CapabilityRegistryRepository;
import bf.gov.faso.auth.service.admin.CapabilityService;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Capability registry + uniqueness check (delta amendment 2026-04-30 §1).
 */
@RestController
@RequestMapping("/admin/capabilities")
public class AdminCapabilityController {

    private final CapabilityRegistryRepository registryRepo;
    private final CapabilityService capabilityService;

    public AdminCapabilityController(CapabilityRegistryRepository registryRepo,
                                     CapabilityService capabilityService) {
        this.registryRepo = registryRepo;
        this.capabilityService = capabilityService;
    }

    @GetMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<List<Map<String, Object>>> list() {
        List<Map<String, Object>> out = registryRepo.findAll().stream()
                .map(AdminCapabilityController::toMap)
                .toList();
        return ResponseEntity.ok(out);
    }

    @PostMapping("/check-uniqueness")
    @PreAuthorize("hasRole('SUPER_ADMIN')")
    public ResponseEntity<Map<String, Object>> checkUniqueness(
            @org.springframework.web.bind.annotation.RequestBody UniquenessRequest req) {
        CapabilityService.AdminLevel role = parseRole(req.role);
        var report = capabilityService.checkUniqueness(req.caps == null ? Set.of() : req.caps, role);
        return ResponseEntity.ok(Map.of(
                "duplicates", report.duplicates,
                "hasDuplicates", report.hasDuplicates()
        ));
    }

    private static Map<String, Object> toMap(CapabilityRegistry c) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("key", c.getKey());
        m.put("category", c.getCategory());
        m.put("descriptionI18nKey", c.getDescriptionI18nKey());
        m.put("applicableToRoles", c.getApplicableToRoles());
        return m;
    }

    private static CapabilityService.AdminLevel parseRole(String role) {
        if (role == null) return null;
        try {
            return CapabilityService.AdminLevel.valueOf(role.toUpperCase());
        } catch (IllegalArgumentException e) {
            return null;
        }
    }

    public static class UniquenessRequest {
        public Set<String> caps;
        public String role;
    }
}
