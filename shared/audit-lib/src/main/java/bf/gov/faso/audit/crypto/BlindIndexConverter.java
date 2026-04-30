// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit.crypto;

import jakarta.persistence.AttributeConverter;
import jakarta.persistence.Converter;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.security.NoSuchAlgorithmException;
import java.security.InvalidKeyException;
import java.util.Base64;
import java.util.HexFormat;

/**
 * JPA {@link AttributeConverter} that produces a deterministic HMAC-SHA256
 * "blind index" of a PII attribute, enabling exact-match search over
 * AES-GCM-encrypted columns.
 *
 * <p><strong>Threat model & design</strong>:
 * <ul>
 *   <li>{@link PiiEncryptionConverter} stores PII with random-IV AES-256-GCM
 *       — strong confidentiality but the same plaintext yields different
 *       ciphertexts, so {@code findByEmail("alice@x")} cannot be served by
 *       a B-tree index.</li>
 *   <li>Blind index = deterministic keyed hash. Same plaintext → same hash,
 *       so the DB can index it. Compromising the DB does NOT reveal
 *       plaintext (HMAC is one-way + the key is in Vault).</li>
 *   <li>The HMAC key MUST be DIFFERENT from the AES PII key — stolen DB
 *       + leaked HMAC key still doesn't decrypt PII.</li>
 *   <li>Plaintext is normalised (lowercased + trimmed) before hashing so
 *       lookups behave like ILIKE-equality on the original attribute.</li>
 * </ul>
 *
 * <p><strong>Usage pattern</strong> (entity):
 * <pre>
 * {@literal @}Convert(converter = PiiEncryptionConverter.class)
 * private String email;
 *
 * {@literal @}Convert(converter = BlindIndexConverter.class)
 * {@literal @}Column(name = "email_hash", length = 64)
 * private String emailHash; // setter copies from email setter
 * </pre>
 *
 * <p><strong>Repository</strong>:
 * <pre>
 * Optional&lt;User&gt; findByEmailHash(String emailHash);
 * </pre>
 *
 * <p>Key provisioning: env var {@code FASO_PII_BLIND_INDEX_KEY} (32 bytes
 * base64). In production, sourced from Vault path
 * {@code faso/shared/pii-blind-index-key}. Validation occurs at boot via
 * {@link PiiEncryptionKeyValidator}.
 *
 * @see PiiEncryptionConverter
 * @see PiiEncryptionKeyValidator
 */
@Converter
public class BlindIndexConverter implements AttributeConverter<String, String> {

    private static final String HMAC_ALGORITHM = "HmacSHA256";
    private static final String ENV_VAR = "FASO_PII_BLIND_INDEX_KEY";

    private final SecretKeySpec macKey;

    public BlindIndexConverter() {
        String keyBase64 = System.getenv(ENV_VAR);
        if (keyBase64 == null || keyBase64.isBlank()) {
            throw new IllegalStateException(
                    ENV_VAR + " environment variable is required. "
                    + "Generate with: openssl rand -base64 32. "
                    + "Must be DIFFERENT from FASO_PII_ENCRYPTION_KEY.");
        }
        byte[] keyBytes = Base64.getDecoder().decode(keyBase64);
        if (keyBytes.length != 32) {
            throw new IllegalStateException(
                    ENV_VAR + " must decode to 32 bytes (256 bits)");
        }
        this.macKey = new SecretKeySpec(keyBytes, HMAC_ALGORITHM);
    }

    @Override
    public String convertToDatabaseColumn(String attribute) {
        if (attribute == null) {
            return null;
        }
        String normalized = attribute.trim().toLowerCase();
        try {
            Mac mac = Mac.getInstance(HMAC_ALGORITHM);
            mac.init(macKey);
            byte[] hash = mac.doFinal(normalized.getBytes(StandardCharsets.UTF_8));
            // Hex (length 64) is more index-friendly than base64 (length 44)
            // and avoids any case-sensitivity surprises on collation.
            return HexFormat.of().formatHex(hash);
        } catch (NoSuchAlgorithmException | InvalidKeyException e) {
            throw new IllegalStateException("Blind index computation failed", e);
        }
    }

    /**
     * One-way: blind index columns are queried by their hash, not decrypted.
     * The original plaintext is recovered from the paired
     * {@link PiiEncryptionConverter} column instead.
     */
    @Override
    public String convertToEntityAttribute(String dbData) {
        return dbData;
    }
}
