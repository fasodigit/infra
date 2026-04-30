// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.admin.DeviceTrustService;
import bf.gov.faso.auth.service.admin.RiskScoringService;
import bf.gov.faso.auth.service.admin.RiskScoringService.Decision;
import bf.gov.faso.auth.service.admin.RiskScoringService.LoginContext;
import bf.gov.faso.auth.service.admin.RiskScoringService.RiskAssessment;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.constraints.NotBlank;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.server.ResponseStatusException;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Phase 4.b.6 risk-based scoring entry point invoked by the BFF
 * {@code /api/admin/auth/login/risk} after Kratos validates the password and
 * BEFORE the MFA prompt. The BFF passes the resolved {@code email} (or
 * userId), the public client IP/UA/Accept-Language so we can rebuild the
 * device fingerprint server-side (no client-supplied fingerprint to avoid
 * spoofing).
 *
 * <p>Response shape:
 * <pre>
 *   200 OK   { decision: "ALLOW" | "STEP_UP", score, signals[] }
 *   403      { decision: "BLOCK", score, reason } — high-risk login refused
 *   404      { error: "user_not_found" }
 * </pre>
 *
 * <p>The endpoint is publicly reachable (it must precede the MFA gate) but
 * rate-limited by the BFF (cf. {@code admin-rate-limit.ts}). Audit + Redpanda
 * publication happen inside {@link RiskScoringService}.
 */
@RestController
@RequestMapping("/admin/auth/login")
public class AdminLoginRiskController {

    private static final Logger log = LoggerFactory.getLogger(AdminLoginRiskController.class);

    private final RiskScoringService riskScoringService;
    private final DeviceTrustService deviceTrustService;
    private final UserRepository userRepository;

    public AdminLoginRiskController(RiskScoringService riskScoringService,
                                    DeviceTrustService deviceTrustService,
                                    UserRepository userRepository) {
        this.riskScoringService = riskScoringService;
        this.deviceTrustService = deviceTrustService;
        this.userRepository = userRepository;
    }

    @PostMapping("/risk")
    public ResponseEntity<Map<String, Object>> assess(@RequestBody RiskRequest req,
                                                      HttpServletRequest http) {
        if (req == null || req.email == null || req.email.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "email required");
        }
        UUID userId = userRepository.findByEmail(req.email.trim().toLowerCase())
                .map(u -> u.getId())
                .orElseThrow(() -> new ResponseStatusException(HttpStatus.NOT_FOUND, "user_not_found"));

        // Re-derive fingerprint server-side (UA + IP/24 + Accept-Language).
        String ip = clientIp(http);
        String ua = headerOrEmpty(http, "User-Agent");
        String acceptLang = headerOrEmpty(http, "Accept-Language");
        String fingerprint = deviceTrustService.computeFingerprint(ua, ip, acceptLang);

        LoginContext ctx = new LoginContext(userId, ip, ua, fingerprint);
        RiskAssessment assessment = riskScoringService.score(ctx);

        Map<String, Object> body = new LinkedHashMap<>();
        body.put("score", assessment.score());
        body.put("decision", assessment.decision().name());
        body.put("signals", assessment.signals().stream()
                .map(s -> Map.of("name", s.name(), "delta", s.delta(),
                        "detail", s.detail() == null ? "" : s.detail()))
                .toList());

        if (assessment.decision() == Decision.BLOCK) {
            log.warn("LOGIN BLOCKED HIGH RISK user={} score={} ip={}",
                    userId, assessment.score(), ip);
            body.put("reason", "high_risk_login");
            // 403 — the BFF should map this to a generic refusal page (no
            // signals leaked to the client to avoid recon).
            return ResponseEntity.status(HttpStatus.FORBIDDEN).body(body);
        }

        // STEP_UP / ALLOW : 200 with full signal payload (consumed by BFF only).
        return ResponseEntity.ok(body);
    }

    // ── helpers ──────────────────────────────────────────────────────────

    private String clientIp(HttpServletRequest req) {
        String xff = req.getHeader("X-Forwarded-For");
        if (xff != null && !xff.isBlank()) {
            int comma = xff.indexOf(',');
            return (comma > 0 ? xff.substring(0, comma) : xff).trim();
        }
        String real = req.getHeader("X-Real-IP");
        if (real != null && !real.isBlank()) return real.trim();
        return req.getRemoteAddr();
    }

    private String headerOrEmpty(HttpServletRequest req, String name) {
        String v = req.getHeader(name);
        return v == null ? "" : v;
    }

    public static class RiskRequest {
        @NotBlank public String email;
        public List<String> hints; // free-form, currently ignored
    }
}
