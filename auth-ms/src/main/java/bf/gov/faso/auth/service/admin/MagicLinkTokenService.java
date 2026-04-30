// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import bf.gov.faso.auth.model.AuditAction;
import com.nimbusds.jose.JOSEException;
import com.nimbusds.jose.JWSAlgorithm;
import com.nimbusds.jose.JWSHeader;
import com.nimbusds.jose.JWSSigner;
import com.nimbusds.jose.JWSVerifier;
import com.nimbusds.jose.crypto.MACSigner;
import com.nimbusds.jose.crypto.MACVerifier;
import com.nimbusds.jwt.JWTClaimsSet;
import com.nimbusds.jwt.SignedJWT;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.Date;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;

/**
 * Magic-link channel-binding service (Phase 4.b.4 §6).
 *
 * <p>Issues short-lived HMAC-SHA256 signed JWTs used at two pivotal points of
 * the admin lifecycle:
 * <ul>
 *   <li><b>Onboarding (signup)</b> — SUPER-ADMIN invites a new admin. The
 *       target receives a magic link {@code .../auth/admin-onboard?token=…};
 *       clicking proves email ownership ("channel-binding"), the page then
 *       displays an 8-digit OTP that must be re-entered to also prove
 *       device-continuity.</li>
 *   <li><b>Self-recovery</b> — replaces the legacy 8-digit token previously
 *       e-mailed for self-initiated recovery flows ; admin-initiated recovery
 *       still uses the 8-digit code.</li>
 * </ul>
 *
 * <p>JTI single-use enforcement is backed by KAYA at
 * {@code auth:magic_link:jti:{jti}} TTL = JWT exp.
 *
 * <p>The HMAC secret is bound from Vault path
 * {@code faso/auth-ms/magic-link-hmac-key} via Spring Cloud Vault property
 * {@code magic-link-hmac-key.value}. A bootstrap fallback property
 * {@code admin.magic-link.hmac-key} is honoured for dev environments where
 * Vault is not yet seeded.
 */
@Service
public class MagicLinkTokenService {

    private static final Logger log = LoggerFactory.getLogger(MagicLinkTokenService.class);

    public static final String SCOPE_ONBOARD = "admin-onboard";
    public static final String SCOPE_RECOVERY = "admin-recovery-self";

    private static final String JTI_PREFIX = "auth:magic_link:jti:";
    private static final String JTI_REPLAY_MARKER = "REPLAYED";

    private final StringRedisTemplate redis;
    private final AdminAuditService auditService;
    private final byte[] hmacKey;

    @Value("${admin.magic-link.issuer:https://auth.faso.gov.bf}")
    private String issuer;

    @Value("${admin.magic-link.audience:faso-admin}")
    private String audience;

    public MagicLinkTokenService(StringRedisTemplate redis,
                                 AdminAuditService auditService,
                                 @Value("${magic-link-hmac-key.value:${admin.magic-link.hmac-key:}}")
                                 String hmacKeyHex) {
        this.redis = redis;
        this.auditService = auditService;
        this.hmacKey = resolveKey(hmacKeyHex);
    }

    /**
     * Issue a single-use signed JWT. Audit-logs {@link AuditAction#MAGIC_LINK_ISSUED}
     * with {@code scope, jti, exp}. Returns the compact-serialized token.
     */
    public IssuedLink issue(String scope, Map<String, Object> claims, Duration ttl) {
        if (scope == null || scope.isBlank()) throw new IllegalArgumentException("scope required");
        if (ttl == null || ttl.isZero() || ttl.isNegative()) {
            ttl = Duration.ofMinutes(30);
        }
        try {
            String jti = UUID.randomUUID().toString();
            Instant now = Instant.now();
            Instant exp = now.plus(ttl);

            JWTClaimsSet.Builder b = new JWTClaimsSet.Builder()
                    .issuer(issuer)
                    .audience(audience)
                    .subject(scope)
                    .jwtID(jti)
                    .issueTime(Date.from(now))
                    .expirationTime(Date.from(exp))
                    .claim("scope", scope);
            if (claims != null) {
                for (Map.Entry<String, Object> e : claims.entrySet()) {
                    if (!"jti".equals(e.getKey()) && !"exp".equals(e.getKey())
                            && !"iat".equals(e.getKey()) && !"iss".equals(e.getKey())
                            && !"aud".equals(e.getKey()) && !"sub".equals(e.getKey())) {
                        b.claim(e.getKey(), e.getValue());
                    }
                }
            }

            SignedJWT jwt = new SignedJWT(
                    new JWSHeader.Builder(JWSAlgorithm.HS256).type(com.nimbusds.jose.JOSEObjectType.JWT).build(),
                    b.build());
            JWSSigner signer = new MACSigner(hmacKey);
            jwt.sign(signer);
            String compact = jwt.serialize();

            // Reserve JTI now so two concurrent verifications race-safe.
            String key = JTI_PREFIX + jti;
            // value=ISSUED ; replaced by REPLAYED on first verify success/fail.
            redis.opsForValue().set(key, "ISSUED", ttl.plus(Duration.ofMinutes(1)));

            Map<String, Object> auditPayload = new LinkedHashMap<>();
            auditPayload.put("scope", scope);
            auditPayload.put("jti", jti);
            auditPayload.put("exp", exp.toString());
            auditService.log(AuditAction.MAGIC_LINK_ISSUED.key(), null, "magic_link:" + scope, null,
                    auditPayload, null);

            log.info("Magic-link issued scope={} jti={} ttl={}s", scope, jti, ttl.getSeconds());
            return new IssuedLink(compact, jti, exp);
        } catch (JOSEException e) {
            throw new IllegalStateException("magic-link sign failed", e);
        }
    }

    /**
     * Verify the signature, expiration, audience, scope, and JTI single-use.
     * On success the JTI is flipped to {@value #JTI_REPLAY_MARKER} so any
     * subsequent verify hits {@link AuditAction#MAGIC_LINK_REPLAYED}.
     */
    public VerifiedLink verify(String token, String expectedScope) {
        if (token == null || token.isBlank()) throw new IllegalArgumentException("token required");
        try {
            SignedJWT jwt = SignedJWT.parse(token);
            JWSVerifier verifier = new MACVerifier(hmacKey);
            if (!jwt.verify(verifier)) {
                throw new IllegalArgumentException("magic-link signature invalid");
            }
            JWTClaimsSet claims = jwt.getJWTClaimsSet();
            String jti = claims.getJWTID();
            if (jti == null) throw new IllegalArgumentException("magic-link jti missing");

            Date exp = claims.getExpirationTime();
            if (exp == null || exp.toInstant().isBefore(Instant.now())) {
                throw new IllegalStateException("magic-link expired");
            }
            if (claims.getAudience() == null || !claims.getAudience().contains(audience)) {
                throw new IllegalArgumentException("magic-link audience mismatch");
            }
            String scope = (String) claims.getClaim("scope");
            if (expectedScope != null && !expectedScope.equals(scope)) {
                throw new IllegalArgumentException("magic-link scope mismatch");
            }

            String key = JTI_PREFIX + jti;
            String prior = redis.opsForValue().get(key);
            if (prior == null) {
                // unknown JTI — token was issued elsewhere or redis flushed
                throw new IllegalStateException("magic-link unknown or expired");
            }
            if (JTI_REPLAY_MARKER.equals(prior)) {
                Map<String, Object> rp = new LinkedHashMap<>();
                rp.put("scope", scope);
                rp.put("jti", jti);
                auditService.log(AuditAction.MAGIC_LINK_REPLAYED.key(), null,
                        "magic_link:" + scope, null, rp, null);
                throw new IllegalStateException("magic-link already used");
            }

            // Atomic flip; even if 2 verifications race, the first wins.
            redis.opsForValue().set(key, JTI_REPLAY_MARKER,
                    Duration.between(Instant.now(), exp.toInstant()).plus(Duration.ofMinutes(1)));

            Map<String, Object> ap = new LinkedHashMap<>();
            ap.put("scope", scope);
            ap.put("jti", jti);
            auditService.log(AuditAction.MAGIC_LINK_VERIFIED.key(), null,
                    "magic_link:" + scope, null, ap, null);

            log.info("Magic-link verified scope={} jti={}", scope, jti);
            return new VerifiedLink(jti, scope, claims.getClaims());
        } catch (java.text.ParseException e) {
            throw new IllegalArgumentException("magic-link malformed", e);
        } catch (JOSEException e) {
            throw new IllegalStateException("magic-link verify failed", e);
        }
    }

    private static byte[] resolveKey(String hex) {
        if (hex == null || hex.isBlank()) {
            // Dev fallback — generate an in-memory ephemeral key. Restarts
            // invalidate all in-flight magic-links.
            byte[] random = new byte[32];
            new java.security.SecureRandom().nextBytes(random);
            log.warn("magic-link HMAC key NOT bound from Vault — using ephemeral in-memory key. "
                    + "Seed faso/auth-ms/magic-link-hmac-key for production.");
            return random;
        }
        try {
            byte[] key = java.util.HexFormat.of().parseHex(hex.trim());
            // MACSigner requires >= 256 bits for HS256.
            if (key.length < 32) {
                byte[] padded = new byte[32];
                System.arraycopy(key, 0, padded, 0, key.length);
                return padded;
            }
            return key;
        } catch (IllegalArgumentException ex) {
            // Treat as raw UTF-8 string (fallback when operator pasted ascii).
            byte[] raw = hex.getBytes(StandardCharsets.UTF_8);
            if (raw.length < 32) {
                byte[] padded = new byte[32];
                System.arraycopy(raw, 0, padded, 0, raw.length);
                return padded;
            }
            return raw;
        }
    }

    // ── DTOs ────────────────────────────────────────────────────────────────

    public record IssuedLink(String token, String jti, Instant expiresAt) {}

    public record VerifiedLink(String jti, String scope, Map<String, Object> claims) {}
}
