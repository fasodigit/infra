// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.model.DeviceRegistration;
import bf.gov.faso.auth.service.admin.DeviceTrustService;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/admin/devices")
public class AdminDeviceController {

    private final DeviceTrustService deviceTrustService;
    private final AdminAuthHelper auth;

    public AdminDeviceController(DeviceTrustService deviceTrustService, AdminAuthHelper auth) {
        this.deviceTrustService = deviceTrustService;
        this.auth = auth;
    }

    @GetMapping
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<List<DeviceRegistration>> list(@RequestParam UUID userId) {
        return ResponseEntity.ok(deviceTrustService.listForUser(userId));
    }

    @PostMapping("/{deviceId}/trust")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<DeviceRegistration> trust(
            @PathVariable UUID deviceId,
            @org.springframework.web.bind.annotation.RequestBody TrustRequest req) {
        UUID actor = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(deviceTrustService.trust(req.userId, deviceId, actor));
    }

    @DeleteMapping("/{deviceId}")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN')")
    public ResponseEntity<Map<String, Object>> revoke(
            @PathVariable UUID deviceId,
            @RequestParam UUID userId) {
        UUID actor = auth.currentUserId().orElseThrow();
        return ResponseEntity.ok(Map.of("revoked",
                deviceTrustService.revoke(userId, deviceId, actor)));
    }

    public static class TrustRequest {
        public UUID userId;
    }
}
