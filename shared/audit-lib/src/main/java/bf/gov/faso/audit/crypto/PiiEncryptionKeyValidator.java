// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit.crypto;

import jakarta.annotation.PostConstruct;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Component;

import java.util.Base64;

/**
 * Boot-time validator for both PII encryption keys.
 *
 * <p>{@link PiiEncryptionConverter} and {@link BlindIndexConverter} are
 * instantiated lazily by JPA on first persist/read of an encrypted column,
 * which means missing or malformed keys would only surface at runtime —
 * long after Spring Boot has reported the application as healthy. This
 * validator runs during {@code ApplicationContext} refresh and fails fast
 * so the deployment pipeline can roll back before any traffic reaches a
 * degraded service.
 *
 * <p>Activation: any service that adds {@code audit-lib} as a dependency
 * and declares {@code faso.audit.pii-encryption.enabled=true} (default
 * {@code true}) gets the validator automatically.
 *
 * <p>The validator also enforces that the two keys differ — using the same
 * key for both AES and HMAC would weaken both primitives.
 */
@Component
@ConditionalOnProperty(prefix = "faso.audit.pii-encryption", name = "enabled", havingValue = "true", matchIfMissing = true)
public class PiiEncryptionKeyValidator {

    private static final int REQUIRED_KEY_BYTES = 32;
    private static final String ENC_VAR = "FASO_PII_ENCRYPTION_KEY";
    private static final String HMAC_VAR = "FASO_PII_BLIND_INDEX_KEY";

    @Value("${faso.audit.pii-encryption.key:#{environment.FASO_PII_ENCRYPTION_KEY}}")
    private String encKeyBase64;

    @Value("${faso.audit.pii-encryption.blind-index-key:#{environment.FASO_PII_BLIND_INDEX_KEY}}")
    private String hmacKeyBase64;

    @PostConstruct
    void validate() {
        byte[] encKey = decodeOrFail(encKeyBase64, ENC_VAR);
        byte[] hmacKey = decodeOrFail(hmacKeyBase64, HMAC_VAR);

        if (java.util.Arrays.equals(encKey, hmacKey)) {
            throw new IllegalStateException(
                    ENC_VAR + " and " + HMAC_VAR + " must be DIFFERENT keys. "
                    + "Reusing one weakens both AES-GCM and HMAC-SHA256.");
        }
    }

    private static byte[] decodeOrFail(String keyBase64, String envVar) {
        if (keyBase64 == null || keyBase64.isBlank()) {
            throw new IllegalStateException(
                    "Required " + envVar + " is missing. "
                    + "Generate with: openssl rand -base64 32. "
                    + "In production, source from Vault path faso/shared/pii-*.");
        }
        byte[] keyBytes;
        try {
            keyBytes = Base64.getDecoder().decode(keyBase64);
        } catch (IllegalArgumentException e) {
            throw new IllegalStateException(envVar + " is not valid base64", e);
        }
        if (keyBytes.length != REQUIRED_KEY_BYTES) {
            throw new IllegalStateException(
                    envVar + " must decode to exactly " + REQUIRED_KEY_BYTES
                    + " bytes (256 bits). Got " + keyBytes.length + " bytes.");
        }
        return keyBytes;
    }
}
