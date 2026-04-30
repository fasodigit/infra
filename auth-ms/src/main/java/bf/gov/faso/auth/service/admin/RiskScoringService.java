// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.infra.kafka.AdminEventProducer;
import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.service.BruteForceService;
import jakarta.annotation.PostConstruct;
import jakarta.persistence.EntityManager;
import jakarta.persistence.PersistenceContext;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.slf4j.MDC;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.web.reactive.function.client.WebClient;

import java.net.URI;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Risk-based scoring MVP — Phase 4.b.6.
 *
 * <p>Computes a 0-100 score from three signals at every login attempt (after
 * password verify, before MFA prompt) and emits a decision in
 * {@link Decision#ALLOW}, {@link Decision#STEP_UP}, {@link Decision#BLOCK}.
 *
 * <h3>Signals (MVP)</h3>
 * <ol>
 *   <li><b>Device fingerprint match</b> — KAYA {@code dev:{userId}:{fp}}.
 *       Trusted device ⇒ {@code -30} (risk reduction).</li>
 *   <li><b>Geo IP distance</b> — MaxMind GeoLite2 + last login row in
 *       {@code login_history}. Same city &lt; 50 km ⇒ {@code 0}; same
 *       country (50-1000 km) ⇒ {@code +10}; different country ⇒ {@code +20};
 *       Tor exit listed ⇒ {@code +40}.</li>
 *   <li><b>Recent brute-force</b> — reuses {@link BruteForceService}
 *       sliding 15 min counter. Any failure in window ⇒ {@code +30}.</li>
 * </ol>
 *
 * <h3>Decision (read from {@code admin_settings}, hot-reloadable)</h3>
 * <pre>
 *   score &lt;  risk.score_threshold_step_up (30)  → ALLOW
 *   score &lt;  risk.score_threshold_block   (80)  → STEP_UP
 *   score &gt;= risk.score_threshold_block         → BLOCK
 * </pre>
 *
 * <p>Persists the assessment in {@code login_history} (in a REQUIRES_NEW
 * transaction so the audit row survives a rollback of the surrounding login
 * flow), publishes {@code auth.risk.assessed} to Redpanda, and records the
 * outcome ID in KAYA {@code auth:risk:{userId}} (sliding 30 d set, used by
 * the future Phase 5 batch analytics).
 *
 * <h3>Tor exit list</h3>
 * <p>Refreshed daily (cron {@code admin.risk.tor-list-refresh-cron}) into KAYA
 * SET {@code auth:tor:exit_list} from
 * {@code https://check.torproject.org/torbulkexitlist}. Empty list ⇒ signal
 * skipped (fail-open).
 */
@Service
public class RiskScoringService {

    private static final Logger log = LoggerFactory.getLogger(RiskScoringService.class);

    private static final String KAYA_DEV_PREFIX = "dev:";
    private static final String KAYA_RISK_PREFIX = "auth:risk:";
    private static final String KAYA_TOR_KEY = "auth:tor:exit_list";
    private static final Duration RISK_SET_TTL = Duration.ofDays(30);
    private static final Duration TOR_LIST_TTL = Duration.ofHours(36);
    private static final Duration BRUTEFORCE_WINDOW = Duration.ofMinutes(15);

    private final StringRedisTemplate redis;
    private final BruteForceService bruteForceService;
    private final GeoIpResolver geoIpResolver;
    private final AdminSettingsService settingsService;
    private final AdminAuditService auditService;
    private final AdminEventProducer eventProducer;

    @PersistenceContext
    private EntityManager em;

    @Value("${admin.risk.enabled:true}")
    private boolean enabled;

    public RiskScoringService(StringRedisTemplate redis,
                              BruteForceService bruteForceService,
                              GeoIpResolver geoIpResolver,
                              AdminSettingsService settingsService,
                              AdminAuditService auditService,
                              AdminEventProducer eventProducer) {
        this.redis = redis;
        this.bruteForceService = bruteForceService;
        this.geoIpResolver = geoIpResolver;
        this.settingsService = settingsService;
        this.auditService = auditService;
        this.eventProducer = eventProducer;
    }

    @PostConstruct
    void boot() {
        if (!enabled) log.warn("RiskScoringService disabled (admin.risk.enabled=false) — all logins ALLOW.");
    }

    // ── Public API ──────────────────────────────────────────────────────────

    /** Inputs required to score a login (built by the calling controller). */
    public record LoginContext(
            UUID userId,
            String ipAddress,
            String userAgent,
            String deviceFingerprint
    ) {}

    /** Outcome of {@link #score(LoginContext)}. */
    public record RiskAssessment(
            int score,
            Decision decision,
            List<Signal> signals,
            UUID loginHistoryId,
            String country
    ) {}

    /** Single contributing signal (auditable). */
    public record Signal(String name, int delta, String detail) {
        public Map<String, Object> toMap() {
            return Map.of("name", name, "delta", delta, "detail", detail == null ? "" : detail);
        }
    }

    public enum Decision { ALLOW, STEP_UP, BLOCK }

    /**
     * Score & decide. ALWAYS persists a {@code login_history} row — even when
     * the service is disabled (decision = ALLOW with empty signals) so the
     * audit trail stays linear.
     */
    @Transactional
    public RiskAssessment score(LoginContext ctx) {
        List<Signal> signals = new ArrayList<>();
        int total = 0;
        String country = null;

        if (enabled) {
            Signal s1 = deviceMatchScore(ctx.userId(), ctx.deviceFingerprint());
            if (s1 != null) { signals.add(s1); total += s1.delta(); }

            GeoSignalOutcome geo = geoDistanceScore(ctx.userId(), ctx.ipAddress());
            if (geo.signal() != null) { signals.add(geo.signal()); total += geo.signal().delta(); }
            country = geo.country();

            Signal s3 = bruteforceRecentScore(ctx.userId());
            if (s3 != null) { signals.add(s3); total += s3.delta(); }
        }

        // Clamp 0-100.
        int score = Math.max(0, Math.min(100, total));
        Decision decision = decide(score);

        UUID loginHistoryId = persistLoginHistory(ctx, score, decision, country);

        // Publish & audit (best-effort).
        try {
            List<Map<String, Object>> sigPayload = signals.stream()
                    .map(Signal::toMap).toList();
            eventProducer.publishRiskAssessed(ctx.userId(), score, decision.name(),
                    ctx.ipAddress(), country, sigPayload,
                    loginHistoryId == null ? null : loginHistoryId.toString());

            if (decision == Decision.BLOCK) {
                eventProducer.publishRiskBlocked(ctx.userId(), score,
                        ctx.ipAddress(), country, signals.toString());
                auditService.log(AuditAction.LOGIN_BLOCKED_HIGH_RISK.key(), ctx.userId(),
                        "user:" + ctx.userId(), null,
                        Map.of("score", score, "country", country == null ? "" : country,
                                "ip", ctx.ipAddress() == null ? "" : ctx.ipAddress(),
                                "signals", sigPayload),
                        ctx.ipAddress());
            } else if (decision == Decision.STEP_UP) {
                auditService.log(AuditAction.LOGIN_STEP_UP_REQUIRED.key(), ctx.userId(),
                        "user:" + ctx.userId(), null,
                        Map.of("score", score, "signals", sigPayload),
                        ctx.ipAddress());
            }

            // Always log the assessment (low-frequency at INFO).
            auditService.log(AuditAction.LOGIN_RISK_ASSESSED.key(), ctx.userId(),
                    "user:" + ctx.userId(), null,
                    Map.of("score", score, "decision", decision.name(),
                            "signals", sigPayload),
                    ctx.ipAddress());

            // Append to KAYA sliding window 30d (members = login_history.id).
            if (loginHistoryId != null) {
                String setKey = KAYA_RISK_PREFIX + ctx.userId();
                redis.opsForSet().add(setKey, loginHistoryId.toString());
                redis.expire(setKey, RISK_SET_TTL);
            }
        } catch (Exception e) {
            log.warn("Risk side-effects (publish/audit/KAYA) failed: {}", e.getMessage());
        }

        log.info("Risk assess user={} score={} decision={} signals={}",
                ctx.userId(), score, decision, signals);
        return new RiskAssessment(score, decision, signals, loginHistoryId, country);
    }

    // ── Signal 1: device fingerprint match ──────────────────────────────────

    public Signal deviceMatchScore(UUID userId, String fingerprint) {
        if (userId == null || fingerprint == null || fingerprint.isBlank()) return null;
        try {
            Boolean trusted = redis.hasKey(KAYA_DEV_PREFIX + userId + ":" + fingerprint);
            if (Boolean.TRUE.equals(trusted)) {
                return new Signal("device.trusted", -30, "fingerprint match in KAYA");
            }
            return new Signal("device.unknown", 0, "fingerprint not trusted");
        } catch (Exception e) {
            log.warn("device-match signal failed: {}", e.getMessage());
            return null;
        }
    }

    // ── Signal 2: geo IP distance ──────────────────────────────────────────

    /** Bundle the signal with the resolved country (used in payloads). */
    record GeoSignalOutcome(Signal signal, String country) {}

    public GeoSignalOutcome geoDistanceScore(UUID userId, String currentIp) {
        if (userId == null || currentIp == null || currentIp.isBlank()) {
            return new GeoSignalOutcome(null, null);
        }

        // Tor exit list takes precedence (worst-case +40).
        try {
            Boolean inTor = redis.opsForSet().isMember(KAYA_TOR_KEY, currentIp);
            if (Boolean.TRUE.equals(inTor)) {
                return new GeoSignalOutcome(
                        new Signal("geo.tor", 40, "ip listed in Tor exit list"),
                        null);
            }
        } catch (Exception e) {
            log.debug("Tor SISMEMBER failed: {}", e.getMessage());
        }

        Optional<GeoIpResolver.GeoLocation> currentGeo = geoIpResolver.resolve(currentIp);
        if (currentGeo.isEmpty()) {
            // Fail-open: no geo signal contribution.
            return new GeoSignalOutcome(null, null);
        }
        String country = currentGeo.get().country();

        // Pull last login row (one-shot native query — index hot path).
        try {
            @SuppressWarnings("unchecked")
            List<Object[]> rows = em.createNativeQuery(
                    "SELECT ip_country_iso2, ip_lat, ip_lon FROM login_history " +
                            "WHERE user_id = :uid AND risk_decision <> 'BLOCK' " +
                            "ORDER BY occurred_at DESC LIMIT 1")
                    .setParameter("uid", userId)
                    .getResultList();
            if (rows.isEmpty()) {
                // First-ever login — neutral.
                return new GeoSignalOutcome(
                        new Signal("geo.first_login", 0,
                                country == null ? "no prior history" : "first login from " + country),
                        country);
            }
            Object[] row = rows.get(0);
            String prevCountry = (String) row[0];
            Double prevLat = row[1] == null ? null : ((Number) row[1]).doubleValue();
            Double prevLon = row[2] == null ? null : ((Number) row[2]).doubleValue();
            GeoIpResolver.GeoLocation prev = new GeoIpResolver.GeoLocation(
                    prevCountry, null, prevLat, prevLon);

            double distKm = currentGeo.get().distanceKmTo(prev);
            if (Double.isNaN(distKm)) {
                // No coords on prior row — fall back on country comparison.
                if (prevCountry != null && country != null
                        && !prevCountry.equalsIgnoreCase(country)) {
                    return new GeoSignalOutcome(
                            new Signal("geo.country_changed", 20,
                                    prevCountry + " → " + country),
                            country);
                }
                return new GeoSignalOutcome(
                        new Signal("geo.unknown_distance", 0, "no coords on prior login"),
                        country);
            }

            int delta;
            String detail;
            if (distKm < 50) { delta = 0; detail = "<50 km"; }
            else if (distKm < 1000) { delta = 10; detail = String.format("%.0f km", distKm); }
            else { delta = 20; detail = String.format("%.0f km (country change)", distKm); }
            return new GeoSignalOutcome(
                    new Signal("geo.distance", delta, detail), country);
        } catch (Exception e) {
            log.warn("geo-distance signal failed: {}", e.getMessage());
            return new GeoSignalOutcome(null, country);
        }
    }

    // ── Signal 3: recent brute-force ───────────────────────────────────────

    public Signal bruteforceRecentScore(UUID userId) {
        if (userId == null) return null;
        try {
            int recent = bruteForceService.getRecentFailures(userId, BRUTEFORCE_WINDOW);
            if (recent > 0) {
                return new Signal("bruteforce.recent", 30,
                        recent + " failed attempts in last " + BRUTEFORCE_WINDOW.toMinutes() + " min");
            }
            return new Signal("bruteforce.clean", 0, "no recent failures");
        } catch (Exception e) {
            log.warn("bruteforce signal failed: {}", e.getMessage());
            return null;
        }
    }

    // ── Decision tree ──────────────────────────────────────────────────────

    public Decision decide(int score) {
        int stepUp = settingsService.getInt("risk.score_threshold_step_up", 30);
        int block = settingsService.getInt("risk.score_threshold_block", 80);
        if (score >= block) return Decision.BLOCK;
        if (score >= stepUp) return Decision.STEP_UP;
        return Decision.ALLOW;
    }

    // ── Persistence ────────────────────────────────────────────────────────

    /**
     * Persists in REQUIRES_NEW so an outer rollback (e.g. login flow refusing
     * the user post-decision) does NOT erase the audit footprint.
     */
    @Transactional(propagation = Propagation.REQUIRES_NEW)
    UUID persistLoginHistory(LoginContext ctx, int score, Decision decision, String country) {
        try {
            UUID id = UUID.randomUUID();
            Optional<GeoIpResolver.GeoLocation> geo = geoIpResolver.resolve(ctx.ipAddress());
            String city = geo.map(GeoIpResolver.GeoLocation::city).orElse(null);
            Double lat = geo.map(GeoIpResolver.GeoLocation::lat).orElse(null);
            Double lon = geo.map(GeoIpResolver.GeoLocation::lon).orElse(null);
            String resolvedCountry = country != null ? country
                    : geo.map(GeoIpResolver.GeoLocation::country).orElse(null);
            String traceId = MDC.get("traceId");

            em.createNativeQuery(
                    "INSERT INTO login_history (id, user_id, ip_address, ip_country_iso2, " +
                            "ip_city, ip_lat, ip_lon, user_agent, device_fingerprint, " +
                            "risk_score, risk_decision, trace_id, occurred_at) " +
                            "VALUES (:id, :uid, :ip, :country, :city, :lat, :lon, :ua, :fp, " +
                            ":score, :decision, :traceId, :ts)")
                    .setParameter("id", id)
                    .setParameter("uid", ctx.userId())
                    .setParameter("ip", ctx.ipAddress() == null ? "0.0.0.0" : ctx.ipAddress())
                    .setParameter("country", resolvedCountry)
                    .setParameter("city", city)
                    .setParameter("lat", lat)
                    .setParameter("lon", lon)
                    .setParameter("ua", ctx.userAgent())
                    .setParameter("fp", ctx.deviceFingerprint())
                    .setParameter("score", score)
                    .setParameter("decision", decision.name())
                    .setParameter("traceId", traceId)
                    .setParameter("ts", Instant.now())
                    .executeUpdate();
            return id;
        } catch (Exception e) {
            log.error("Failed to persist login_history for user={}: {}", ctx.userId(), e.getMessage());
            return null;
        }
    }

    // ── Tor exit list refresh ──────────────────────────────────────────────

    /**
     * Refreshes the Tor exit list every day at 04:00 UTC by default (cron is
     * configurable via {@code admin.risk.tor-list-refresh-cron}). Failures are
     * logged and the previous list is retained — fail-open.
     */
    @Scheduled(cron = "${admin.risk.tor-list-refresh-cron:0 0 4 * * *}")
    public void refreshTorExitList() {
        if (!enabled) return;
        log.info("Refreshing Tor exit list...");
        try {
            String body = WebClient.create()
                    .get()
                    .uri(URI.create("https://check.torproject.org/torbulkexitlist"))
                    .retrieve()
                    .bodyToMono(String.class)
                    .timeout(Duration.ofSeconds(30))
                    .block();
            if (body == null || body.isBlank()) {
                log.warn("Empty Tor exit list response — keeping previous KAYA set.");
                return;
            }
            List<String> ips = Arrays.stream(body.split("\\R"))
                    .map(String::trim)
                    .filter(s -> !s.isEmpty() && !s.startsWith("#"))
                    .toList();
            if (ips.isEmpty()) {
                log.warn("Parsed 0 IPs from Tor list — fail-open.");
                return;
            }
            // Atomic-ish refresh: write to a temp key then RENAME.
            String tmp = KAYA_TOR_KEY + ":refresh";
            redis.delete(tmp);
            redis.opsForSet().add(tmp, ips.toArray(String[]::new));
            redis.rename(tmp, KAYA_TOR_KEY);
            redis.expire(KAYA_TOR_KEY, TOR_LIST_TTL);
            log.info("Tor exit list refreshed — {} IPs in KAYA {}", ips.size(), KAYA_TOR_KEY);
        } catch (Exception e) {
            log.warn("Tor exit list refresh failed: {} — keeping previous list (fail-open).",
                    e.getMessage());
        }
    }
}
