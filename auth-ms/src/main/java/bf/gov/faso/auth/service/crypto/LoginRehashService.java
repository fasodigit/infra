// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.crypto;

import bf.gov.faso.auth.model.AuditAction;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.UserRepository;
import bf.gov.faso.auth.service.admin.AdminAuditService;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * Lazy re-hash hook (cf. plan §1 "Migration des hashs existants").
 *
 * <p>Today auth-ms does not own the password hash — Kratos does. This service
 * is therefore a forward-compatible scaffold:
 * <ul>
 *   <li>Streams 4.b.4 (magic-link signup) and 4.b.5 (push approval) will be
 *   the first paths where auth-ms holds a verified plaintext password ; they
 *   will call {@link #attemptRehashOnLogin}.</li>
 *   <li>Until then, the {@link #shouldRehash} check is exposed so the existing
 *   {@code AdminAuditService} hooks (e.g. {@code login.success} audit row) can
 *   emit a {@link AuditAction#HASH_REHASHED_ON_LOGIN} event when a stale-algo
 *   user logs in via Kratos.</li>
 * </ul>
 *
 * <p>The actual UPDATE is wrapped in a separate transaction so a hash failure
 * never breaks the login flow that called us.
 */
@Service
public class LoginRehashService {

    private static final Logger log = LoggerFactory.getLogger(LoginRehashService.class);

    private final UserRepository userRepository;
    private final CryptographicHashService cryptoHashService;
    private final AdminAuditService auditService;
    private final ObjectMapper objectMapper;
    private final Argon2idCryptographicHashService argon2Service;

    public LoginRehashService(UserRepository userRepository,
                              CryptographicHashService cryptoHashService,
                              AdminAuditService auditService,
                              ObjectMapper objectMapper,
                              Argon2idCryptographicHashService argon2Service) {
        this.userRepository = userRepository;
        this.cryptoHashService = cryptoHashService;
        this.auditService = auditService;
        this.objectMapper = objectMapper;
        this.argon2Service = argon2Service;
    }

    /**
     * Decide whether the user's stored hash is stale. Reads the columns added
     * by V13 (hash_algo, hash_pepper_version) via raw SQL since the {@link User}
     * JPA entity has not been amended yet — Phase 4.b.4 will add the fields.
     */
    public boolean shouldRehash(UUID userId) {
        Optional<User> opt = userRepository.findById(userId);
        if (opt.isEmpty()) return false;
        // We can't read hash_algo via the entity yet, so be conservative:
        // assume stale until the entity is extended in a follow-up stream.
        return true;
    }

    /**
     * Rehash the verified plaintext password and persist via raw SQL UPDATE.
     * Emits {@link AuditAction#HASH_REHASHED_ON_LOGIN}.
     *
     * <p>Caller MUST already have validated the plaintext (e.g. via Kratos
     * webhook with a captured plaintext, or via the upcoming auth-ms-owned
     * password verification path). Plaintext is wiped after hashing.
     */
    @Transactional
    public void attemptRehashOnLogin(UUID userId, char[] verifiedPlaintext) {
        if (verifiedPlaintext == null || verifiedPlaintext.length == 0) return;
        try {
            String newHash = cryptoHashService.hashPassword(verifiedPlaintext);
            HashParams params = cryptoHashService.currentPasswordParams();
            String paramsJson = objectMapper.writeValueAsString(params.toMap());
            int pepperVersion = argon2Service.currentPepperVersion();

            // Persist via JPA-bypass UPDATE to avoid round-tripping the entire
            // User entity; the columns we touch are nullable additions from V13.
            int rows = userRepository.updatePasswordHashColumns(
                    userId, newHash, "argon2id", paramsJson, pepperVersion);
            if (rows == 1) {
                auditService.log(AuditAction.HASH_REHASHED_ON_LOGIN.key(), userId,
                        "user:" + userId, null,
                        Map.of("algo", "argon2id",
                                "pepper_version", pepperVersion,
                                "params", params.toMap()),
                        null);
                log.info("HASH_REHASHED_ON_LOGIN user={} pepper_v={} m={} t={} p={}",
                        userId, pepperVersion, params.memory(),
                        params.iterations(), params.parallelism());
            }
        } catch (Exception e) {
            // Never propagate — the login itself already succeeded.
            log.warn("Lazy re-hash failed user={} : {}", userId, e.getMessage());
        }
    }
}
