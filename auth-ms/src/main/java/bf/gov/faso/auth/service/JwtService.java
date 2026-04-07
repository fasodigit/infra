package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.JwtSigningKey;
import bf.gov.faso.auth.model.User;
import bf.gov.faso.auth.repository.JwtSigningKeyRepository;
import com.nimbusds.jose.*;
import com.nimbusds.jose.crypto.ECDSASigner;
import com.nimbusds.jose.crypto.ECDSAVerifier;
import com.nimbusds.jose.jwk.Curve;
import com.nimbusds.jose.jwk.ECKey;
import com.nimbusds.jose.jwk.JWKSet;
import com.nimbusds.jwt.JWTClaimsSet;
import com.nimbusds.jwt.SignedJWT;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.io.StringReader;
import java.io.StringWriter;
import java.security.*;
import java.security.interfaces.ECPrivateKey;
import java.security.interfaces.ECPublicKey;
import java.security.spec.ECGenParameterSpec;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.*;
import java.util.stream.Collectors;

import org.bouncycastle.openssl.PEMParser;
import org.bouncycastle.openssl.jcajce.JcaPEMWriter;
import org.bouncycastle.openssl.jcajce.JcaPEMKeyConverter;
import org.bouncycastle.openssl.PEMKeyPair;
import org.bouncycastle.asn1.pkcs.PrivateKeyInfo;
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo;

/**
 * JWT service handling ES384 key generation, rotation, signing, and JWKS endpoint data.
 * <p>
 * Keys are persisted in PostgreSQL and rotated on a 90-day cycle.
 * The JWKS JSON is served by JwksController and consumed by ARMAGEDDON's jwt_authn filter.
 */
@Service
public class JwtService {

    private static final Logger log = LoggerFactory.getLogger(JwtService.class);

    private final JwtSigningKeyRepository keyRepository;

    @Value("${auth.jwt.rotation-days:90}")
    private int rotationDays;

    @Value("${auth.jwt.issuer:https://auth.faso.gov.bf}")
    private String issuer;

    @Value("${auth.jwt.audience:faso-digitalisation}")
    private String audience;

    @Value("${auth.jwt.access-token-ttl-minutes:15}")
    private int accessTokenTtlMinutes;

    @Value("${auth.jwt.refresh-token-ttl-days:7}")
    private int refreshTokenTtlDays;

    // Cached active signing key
    private volatile ECPrivateKey activePrivateKey;
    private volatile String activeKid;

    public JwtService(JwtSigningKeyRepository keyRepository) {
        this.keyRepository = keyRepository;
    }

    @PostConstruct
    public void init() {
        ensureActiveKeyExists();
    }

    /**
     * Ensure at least one active signing key exists. If not, generate one.
     */
    @Transactional
    public void ensureActiveKeyExists() {
        Optional<JwtSigningKey> activeKey = keyRepository.findFirstByActiveTrueOrderByCreatedAtDesc();
        if (activeKey.isEmpty()) {
            log.info("No active JWT signing key found. Generating initial ES384 keypair.");
            generateAndStoreNewKey();
        } else {
            loadActiveKey(activeKey.get());
        }
    }

    /**
     * Generate a new ES384 keypair, store it, and deactivate old keys.
     */
    @Transactional
    public JwtSigningKey rotateKeys() {
        log.info("Rotating JWT signing keys (ES384)");

        // Deactivate all existing active keys
        List<JwtSigningKey> activeKeys = keyRepository.findByActiveTrue();
        for (JwtSigningKey key : activeKeys) {
            key.setActive(false);
            key.setRevokedAt(Instant.now());
            keyRepository.save(key);
        }

        return generateAndStoreNewKey();
    }

    /**
     * Scheduled check: auto-rotate if the active key is past its expiration.
     * Runs daily at 02:00 UTC.
     */
    @Scheduled(cron = "0 0 2 * * *")
    @Transactional
    public void scheduledKeyRotationCheck() {
        Optional<JwtSigningKey> activeKey = keyRepository.findFirstByActiveTrueOrderByCreatedAtDesc();
        if (activeKey.isPresent() && activeKey.get().isExpired()) {
            log.warn("Active JWT signing key has expired (kid={}). Triggering automatic rotation.",
                    activeKey.get().getKid());
            rotateKeys();
        }
    }

    /**
     * Sign an access token JWT for the given user.
     */
    public String signAccessToken(User user) {
        return signToken(user, accessTokenTtlMinutes * 60L);
    }

    /**
     * Sign a refresh token JWT for the given user.
     */
    public String signRefreshToken(User user) {
        return signToken(user, refreshTokenTtlDays * 24L * 60 * 60);
    }

    /**
     * Verify and parse a JWT, returning the claims if valid.
     */
    public Optional<JWTClaimsSet> verifyToken(String token) {
        try {
            SignedJWT signedJwt = SignedJWT.parse(token);
            String kid = signedJwt.getHeader().getKeyID();

            // Look up the signing key by KID
            Optional<JwtSigningKey> signingKey = keyRepository.findByKid(kid);
            if (signingKey.isEmpty()) {
                log.warn("JWT verification failed: unknown kid={}", kid);
                return Optional.empty();
            }

            ECPublicKey publicKey = loadPublicKey(signingKey.get().getPublicKeyPem());
            JWSVerifier verifier = new ECDSAVerifier(publicKey);

            if (!signedJwt.verify(verifier)) {
                log.warn("JWT signature verification failed for kid={}", kid);
                return Optional.empty();
            }

            JWTClaimsSet claims = signedJwt.getJWTClaimsSet();

            // Check expiration
            if (claims.getExpirationTime() != null
                    && claims.getExpirationTime().toInstant().isBefore(Instant.now())) {
                log.debug("JWT expired for sub={}", claims.getSubject());
                return Optional.empty();
            }

            return Optional.of(claims);
        } catch (Exception e) {
            log.error("JWT verification error: {}", e.getMessage());
            return Optional.empty();
        }
    }

    /**
     * Build the JWKS JSON for the /.well-known/jwks.json endpoint.
     * Returns all active (non-revoked) public keys so ARMAGEDDON can validate tokens
     * signed with current or recently rotated keys.
     */
    public Map<String, Object> buildJwks() {
        List<JwtSigningKey> activeKeys = keyRepository.findByActiveTrue();
        List<Map<String, Object>> keys = new ArrayList<>();

        for (JwtSigningKey signingKey : activeKeys) {
            try {
                ECPublicKey publicKey = loadPublicKey(signingKey.getPublicKeyPem());
                ECKey jwk = new ECKey.Builder(Curve.P_384, publicKey)
                        .keyID(signingKey.getKid())
                        .keyUse(com.nimbusds.jose.jwk.KeyUse.SIGNATURE)
                        .algorithm(JWSAlgorithm.ES384)
                        .build();
                keys.add(jwk.toJSONObject());
            } catch (Exception e) {
                log.error("Failed to build JWK for kid={}: {}", signingKey.getKid(), e.getMessage());
            }
        }

        Map<String, Object> jwks = new LinkedHashMap<>();
        jwks.put("keys", keys);
        return jwks;
    }

    // ---- Internal ----

    private String signToken(User user, long ttlSeconds) {
        if (activePrivateKey == null || activeKid == null) {
            throw new IllegalStateException("No active JWT signing key available");
        }

        try {
            String jti = UUID.randomUUID().toString();
            Instant now = Instant.now();
            Instant exp = now.plusSeconds(ttlSeconds);

            List<String> roles = user.getRoles().stream()
                    .map(r -> r.getName())
                    .collect(Collectors.toList());

            JWTClaimsSet claims = new JWTClaimsSet.Builder()
                    .issuer(issuer)
                    .subject(user.getId().toString())
                    .audience(audience)
                    .jwtID(jti)
                    .issueTime(Date.from(now))
                    .expirationTime(Date.from(exp))
                    .claim("email", user.getEmail())
                    .claim("roles", roles)
                    .claim("department", user.getDepartment())
                    .build();

            JWSHeader header = new JWSHeader.Builder(JWSAlgorithm.ES384)
                    .keyID(activeKid)
                    .type(JOSEObjectType.JWT)
                    .build();

            SignedJWT signedJwt = new SignedJWT(header, claims);
            signedJwt.sign(new ECDSASigner(activePrivateKey));

            return signedJwt.serialize();
        } catch (JOSEException e) {
            throw new RuntimeException("Failed to sign JWT", e);
        }
    }

    private JwtSigningKey generateAndStoreNewKey() {
        try {
            KeyPairGenerator keyGen = KeyPairGenerator.getInstance("EC");
            keyGen.initialize(new ECGenParameterSpec("secp384r1"));
            KeyPair keyPair = keyGen.generateKeyPair();

            ECPublicKey publicKey = (ECPublicKey) keyPair.getPublic();
            ECPrivateKey privateKey = (ECPrivateKey) keyPair.getPrivate();

            String kid = "faso-" + UUID.randomUUID().toString().substring(0, 8);
            Instant expiresAt = Instant.now().plus(rotationDays, ChronoUnit.DAYS);

            JwtSigningKey signingKey = new JwtSigningKey();
            signingKey.setKid(kid);
            signingKey.setAlgorithm("ES384");
            signingKey.setPublicKeyPem(encodePem(publicKey));
            signingKey.setPrivateKeyPem(encodePem(privateKey));
            signingKey.setActive(true);
            signingKey.setExpiresAt(expiresAt);

            JwtSigningKey saved = keyRepository.save(signingKey);
            loadActiveKey(saved);

            log.info("Generated new ES384 keypair: kid={}, expires={}", kid, expiresAt);
            return saved;
        } catch (Exception e) {
            throw new RuntimeException("Failed to generate ES384 keypair", e);
        }
    }

    private void loadActiveKey(JwtSigningKey signingKey) {
        try {
            this.activePrivateKey = loadPrivateKey(signingKey.getPrivateKeyPem());
            this.activeKid = signingKey.getKid();
            log.info("Loaded active signing key: kid={}", activeKid);
        } catch (Exception e) {
            throw new RuntimeException("Failed to load active signing key kid=" + signingKey.getKid(), e);
        }
    }

    private String encodePem(Key key) throws Exception {
        StringWriter sw = new StringWriter();
        try (JcaPEMWriter writer = new JcaPEMWriter(sw)) {
            writer.writeObject(key);
        }
        return sw.toString();
    }

    private ECPublicKey loadPublicKey(String pem) throws Exception {
        try (PEMParser parser = new PEMParser(new StringReader(pem))) {
            Object obj = parser.readObject();
            JcaPEMKeyConverter converter = new JcaPEMKeyConverter();
            if (obj instanceof SubjectPublicKeyInfo) {
                return (ECPublicKey) converter.getPublicKey((SubjectPublicKeyInfo) obj);
            }
            throw new IllegalArgumentException("Not a valid public key PEM");
        }
    }

    private ECPrivateKey loadPrivateKey(String pem) throws Exception {
        try (PEMParser parser = new PEMParser(new StringReader(pem))) {
            Object obj = parser.readObject();
            JcaPEMKeyConverter converter = new JcaPEMKeyConverter();
            if (obj instanceof PEMKeyPair) {
                return (ECPrivateKey) converter.getKeyPair((PEMKeyPair) obj).getPrivate();
            }
            if (obj instanceof PrivateKeyInfo) {
                return (ECPrivateKey) converter.getPrivateKey((PrivateKeyInfo) obj);
            }
            throw new IllegalArgumentException("Not a valid private key PEM");
        }
    }

    public String getActiveKid() {
        return activeKid;
    }
}
