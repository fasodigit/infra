// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.auth.service.admin;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.yubico.webauthn.AssertionRequest;
import com.yubico.webauthn.CredentialRepository;
import com.yubico.webauthn.FinishAssertionOptions;
import com.yubico.webauthn.FinishRegistrationOptions;
import com.yubico.webauthn.RegisteredCredential;
import com.yubico.webauthn.RelyingParty;
import com.yubico.webauthn.StartAssertionOptions;
import com.yubico.webauthn.StartRegistrationOptions;
import com.yubico.webauthn.data.ByteArray;
import com.yubico.webauthn.data.PublicKeyCredentialCreationOptions;
import com.yubico.webauthn.data.PublicKeyCredentialDescriptor;
import com.yubico.webauthn.data.RelyingPartyIdentity;
import com.yubico.webauthn.data.UserIdentity;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.security.SecureRandom;
import java.time.Duration;
import java.util.Collections;
import java.util.HashSet;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * WebAuthn / FIDO2 PassKey service.
 * <p>
 * Backed by {@code com.yubico:webauthn-server-core}. Pending challenges are
 * stored in KAYA at {@code auth:passkey:pending:{userId}} (TTL 600s); the
 * registered credentials are kept in-memory for Phase 4.b iter 1 (TODO
 * iteration 2: persist to a {@code passkey_credentials} table).
 * <p>
 * Relying-party id is taken from {@code admin.webauthn.rp-id} (defaults to
 * "faso.bf"). The browser must be served from the same eTLD+1 to register.
 */
@Service
public class WebAuthnService {

    private static final Logger log = LoggerFactory.getLogger(WebAuthnService.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();
    private static final String PENDING_KEY = "auth:passkey:pending:";
    private static final SecureRandom RNG = new SecureRandom();

    @Value("${admin.webauthn.rp-id:faso.bf}")
    private String rpId;

    @Value("${admin.webauthn.rp-name:FASO DIGITALISATION}")
    private String rpName;

    private final StringRedisTemplate redis;
    private final InMemoryCredentialRepository credentialRepository = new InMemoryCredentialRepository();
    private RelyingParty relyingParty;

    private final AdminAuditService auditService;

    public WebAuthnService(StringRedisTemplate redis, AdminAuditService auditService) {
        this.redis = redis;
        this.auditService = auditService;
    }

    @PostConstruct
    void init() {
        RelyingPartyIdentity rpIdentity = RelyingPartyIdentity.builder()
                .id(rpId)
                .name(rpName)
                .build();
        this.relyingParty = RelyingParty.builder()
                .identity(rpIdentity)
                .credentialRepository(credentialRepository)
                .origins(Set.of("https://" + rpId, "https://admin." + rpId))
                .build();
        log.info("WebAuthn relying party initialised rpId={} rpName={}", rpId, rpName);
    }

    /**
     * Begin registration — return the JSON-serialised
     * {@link PublicKeyCredentialCreationOptions} the browser will hand to
     * {@code navigator.credentials.create()}.
     */
    public String registerBegin(UUID userId, String userName, String displayName) {
        ByteArray userHandle = uuidToByteArray(userId);
        UserIdentity user = UserIdentity.builder()
                .name(userName)
                .displayName(displayName == null ? userName : displayName)
                .id(userHandle)
                .build();

        PublicKeyCredentialCreationOptions options = relyingParty.startRegistration(
                StartRegistrationOptions.builder()
                        .user(user)
                        .build());

        try {
            String json = options.toCredentialsCreateJson();
            redis.opsForValue().set(PENDING_KEY + userId, json, Duration.ofMinutes(10));
            auditService.log("passkey.register.begin", userId, "user:" + userId, null,
                    Map.of("rpId", rpId), null);
            return json;
        } catch (JsonProcessingException e) {
            throw new IllegalStateException("failed to serialize creation options", e);
        }
    }

    /**
     * Finish registration. {@code clientResponseJson} is the
     * PublicKeyCredential the browser produced in {@code create()}.
     * Iteration 1 acknowledges receipt and increments the in-memory
     * credential map. Iteration 2 will persist to DB and run the full
     * {@link RelyingParty#finishRegistration} ceremony with attestation
     * verification.
     */
    public boolean registerFinish(UUID userId, String clientResponseJson) {
        // TODO Phase 4.b iteration 2 — full attestation verification + DB persistence.
        try {
            String pending = redis.opsForValue().get(PENDING_KEY + userId);
            if (pending == null) return false;

            // Iter 1: just record we accepted a credential.
            String credId = "cred-" + UUID.randomUUID();
            ByteArray cid = new ByteArray(credId.getBytes());
            RegisteredCredential rc = RegisteredCredential.builder()
                    .credentialId(cid)
                    .userHandle(uuidToByteArray(userId))
                    .publicKeyCose(new ByteArray(new byte[0]))
                    .signatureCount(0)
                    .build();
            credentialRepository.add(userId, rc);

            redis.delete(PENDING_KEY + userId);
            auditService.log("passkey.register.finish", userId, "user:" + userId, null,
                    Map.of("credentialId", credId), null);
            return true;
        } catch (Exception e) {
            log.error("Passkey register finish error: {}", e.getMessage());
            return false;
        }
    }

    public String authenticateBegin(UUID userId) {
        AssertionRequest request = relyingParty.startAssertion(
                StartAssertionOptions.builder()
                        .username(userId.toString())
                        .build());
        try {
            String json = request.toCredentialsGetJson();
            redis.opsForValue().set(PENDING_KEY + userId + ":auth", json, Duration.ofMinutes(10));
            return json;
        } catch (JsonProcessingException e) {
            throw new IllegalStateException("failed to serialize assertion request", e);
        }
    }

    public boolean authenticateFinish(UUID userId, String clientAssertionJson) {
        // TODO Phase 4.b iteration 2 — full assertion verification.
        String pending = redis.opsForValue().get(PENDING_KEY + userId + ":auth");
        if (pending == null) return false;
        redis.delete(PENDING_KEY + userId + ":auth");
        auditService.log("passkey.authenticate", userId, "user:" + userId, null,
                Map.of("result", "accepted-iter1"), null);
        return true;
    }

    public int countCredentials(UUID userId) {
        return credentialRepository.countForUser(userId);
    }

    public boolean revoke(UUID userId, String credentialId, UUID actorId) {
        boolean removed = credentialRepository.remove(userId, credentialId);
        if (removed) {
            auditService.log("passkey.revoked", actorId, "user:" + userId, null,
                    Map.of("credentialId", credentialId), null);
        }
        return removed;
    }

    private static ByteArray uuidToByteArray(UUID userId) {
        byte[] buf = new byte[16];
        long msb = userId.getMostSignificantBits();
        long lsb = userId.getLeastSignificantBits();
        for (int i = 0; i < 8; i++) buf[i] = (byte) (msb >>> (8 * (7 - i)));
        for (int i = 8; i < 16; i++) buf[i] = (byte) (lsb >>> (8 * (15 - i)));
        return new ByteArray(buf);
    }

    /**
     * In-memory CredentialRepository — replace with a JPA-backed impl in
     * iteration 2. Persistent storage will use a {@code passkey_credentials}
     * table created by a future Flyway migration.
     */
    static class InMemoryCredentialRepository implements CredentialRepository {

        private final Map<UUID, Set<RegisteredCredential>> byUser = new ConcurrentHashMap<>();
        private final Map<ByteArray, UUID> byUserHandle = new ConcurrentHashMap<>();

        void add(UUID userId, RegisteredCredential cred) {
            byUser.computeIfAbsent(userId, k -> ConcurrentHashMap.newKeySet()).add(cred);
            byUserHandle.put(cred.getUserHandle(), userId);
        }

        boolean remove(UUID userId, String credentialId) {
            Set<RegisteredCredential> set = byUser.get(userId);
            if (set == null) return false;
            return set.removeIf(c -> new String(c.getCredentialId().getBytes()).equals(credentialId));
        }

        int countForUser(UUID userId) {
            Set<RegisteredCredential> set = byUser.get(userId);
            return set == null ? 0 : set.size();
        }

        @Override
        public Set<PublicKeyCredentialDescriptor> getCredentialIdsForUsername(String username) {
            try {
                UUID userId = UUID.fromString(username);
                Set<RegisteredCredential> set = byUser.getOrDefault(userId, Collections.emptySet());
                Set<PublicKeyCredentialDescriptor> result = new HashSet<>();
                for (RegisteredCredential c : set) {
                    result.add(PublicKeyCredentialDescriptor.builder().id(c.getCredentialId()).build());
                }
                return result;
            } catch (IllegalArgumentException e) {
                return Collections.emptySet();
            }
        }

        @Override
        public Optional<ByteArray> getUserHandleForUsername(String username) {
            try {
                UUID userId = UUID.fromString(username);
                return Optional.of(uuidToByteArray(userId));
            } catch (IllegalArgumentException e) {
                return Optional.empty();
            }
        }

        @Override
        public Optional<String> getUsernameForUserHandle(ByteArray userHandle) {
            UUID userId = byUserHandle.get(userHandle);
            return Optional.ofNullable(userId).map(UUID::toString);
        }

        @Override
        public Optional<RegisteredCredential> lookup(ByteArray credentialId, ByteArray userHandle) {
            UUID userId = byUserHandle.get(userHandle);
            if (userId == null) return Optional.empty();
            return byUser.getOrDefault(userId, Collections.emptySet()).stream()
                    .filter(c -> c.getCredentialId().equals(credentialId))
                    .findFirst();
        }

        @Override
        public Set<RegisteredCredential> lookupAll(ByteArray credentialId) {
            Set<RegisteredCredential> all = new HashSet<>();
            for (Set<RegisteredCredential> s : byUser.values()) {
                for (RegisteredCredential c : s) {
                    if (c.getCredentialId().equals(credentialId)) all.add(c);
                }
            }
            return all;
        }
    }
}
