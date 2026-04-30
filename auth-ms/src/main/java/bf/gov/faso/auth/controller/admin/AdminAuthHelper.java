// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.controller.admin;

import bf.gov.faso.auth.security.JwtAuthenticatedPrincipal;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Duration;
import java.util.HexFormat;
import java.util.Optional;
import java.util.UUID;

/**
 * Shared utilities for admin controllers — JWT principal access &
 * idempotency-key handling backed by KAYA (TTL 24h).
 */
@Component
public class AdminAuthHelper {

    private static final String IDEMPOTENCY_PREFIX = "auth:idempotency:";
    private static final Duration IDEMPOTENCY_TTL = Duration.ofHours(24);

    private final StringRedisTemplate redis;

    public AdminAuthHelper(StringRedisTemplate redis) {
        this.redis = redis;
    }

    public Optional<JwtAuthenticatedPrincipal> currentPrincipal() {
        Authentication auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || !(auth.getPrincipal() instanceof JwtAuthenticatedPrincipal p)) {
            return Optional.empty();
        }
        return Optional.of(p);
    }

    public Optional<UUID> currentUserId() {
        return currentPrincipal().map(p -> {
            try { return UUID.fromString(p.getUserId()); } catch (Exception e) { return null; }
        });
    }

    /**
     * Returns true if this Idempotency-Key has not been seen yet (fresh
     * request); marks the key as seen for {@link #IDEMPOTENCY_TTL}.
     * Keys are SHA-256-hashed before storage to avoid leaking caller-chosen
     * tokens through KAYA listings.
     */
    public boolean acquireIdempotency(String idempotencyKey) {
        if (idempotencyKey == null || idempotencyKey.isBlank()) return true; // optional header
        String hash = sha256(idempotencyKey);
        Boolean acquired = redis.opsForValue().setIfAbsent(
                IDEMPOTENCY_PREFIX + hash, "1", IDEMPOTENCY_TTL);
        return Boolean.TRUE.equals(acquired);
    }

    private String sha256(String s) {
        try {
            byte[] d = MessageDigest.getInstance("SHA-256")
                    .digest(s.getBytes(StandardCharsets.UTF_8));
            return HexFormat.of().formatHex(d);
        } catch (Exception e) {
            return s; // fallback — never silently drop the guard
        }
    }
}
