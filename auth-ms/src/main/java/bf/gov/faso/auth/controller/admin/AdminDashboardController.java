// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.repository.AdminRoleGrantRepository;
import bf.gov.faso.auth.repository.MfaStatusRepository;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.model.AdminRoleGrant;
import org.springframework.http.ResponseEntity;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Roll-up KPIs surface for the admin dashboard. Iter 1 returns counts;
 * iter 2 will add P50/P99 OTP issue latency, error rates, time-series.
 */
@RestController
@RequestMapping("/admin/dashboard")
public class AdminDashboardController {

    private final UserRepository userRepository;
    private final AdminRoleGrantRepository grantRepo;
    private final MfaStatusRepository mfaRepo;

    public AdminDashboardController(UserRepository userRepository,
                                    AdminRoleGrantRepository grantRepo,
                                    MfaStatusRepository mfaRepo) {
        this.userRepository = userRepository;
        this.grantRepo = grantRepo;
        this.mfaRepo = mfaRepo;
    }

    @GetMapping("/kpis")
    @PreAuthorize("hasAnyRole('SUPER_ADMIN','ADMIN','MANAGER')")
    public ResponseEntity<Map<String, Object>> kpis() {
        Map<String, Object> kpis = new LinkedHashMap<>();
        kpis.put("usersTotal", userRepository.count());
        kpis.put("pendingGrants", grantRepo.findByStatus(AdminRoleGrant.Status.PENDING).size());
        kpis.put("totpEnrolled", mfaRepo.countByTotpEnabledTrue());
        return ResponseEntity.ok(kpis);
    }
}
