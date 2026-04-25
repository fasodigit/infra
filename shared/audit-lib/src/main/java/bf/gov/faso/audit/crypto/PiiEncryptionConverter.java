// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
package bf.gov.faso.audit.crypto;

import jakarta.persistence.AttributeConverter;
import jakarta.persistence.Converter;

import javax.crypto.Cipher;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Base64;

/**
 * JPA {@link AttributeConverter} that transparently encrypts/decrypts PII
 * columns using AES-256-GCM.
 *
 * <p>Wire format stored in database (base64):
 * {@code [ 12-byte IV | ciphertext | 16-byte GCM auth-tag ]}
 *
 * <p><strong>Key provisioning:</strong> The 32-byte AES key must be supplied
 * via the {@code FASO_PII_ENCRYPTION_KEY} environment variable as base64.
 * In production, inject from Vault ({@code faso/shared/pii-encryption-key}).
 *
 * <p>Usage on entity fields:
 * <pre>
 * {@literal @}Convert(converter = PiiEncryptionConverter.class)
 * private String email;
 * </pre>
 *
 * @see bf.gov.faso.auth.persistence.EncryptedStringConverter — similar converter for JWT keys
 */
@Converter
public class PiiEncryptionConverter implements AttributeConverter<String, String> {

    private static final String ALGORITHM = "AES/GCM/NoPadding";
    private static final int GCM_TAG_LENGTH = 128; // bits
    private static final int GCM_IV_LENGTH = 12;   // bytes

    private final SecretKey secretKey;
    private final SecureRandom secureRandom = new SecureRandom();

    public PiiEncryptionConverter() {
        String keyBase64 = System.getenv("FASO_PII_ENCRYPTION_KEY");
        if (keyBase64 == null || keyBase64.isBlank()) {
            throw new IllegalStateException(
                    "FASO_PII_ENCRYPTION_KEY environment variable is required. "
                    + "Generate with: openssl rand -base64 32");
        }
        byte[] keyBytes = Base64.getDecoder().decode(keyBase64);
        if (keyBytes.length != 32) {
            throw new IllegalStateException(
                    "FASO_PII_ENCRYPTION_KEY must be exactly 32 bytes (256 bits) base64-encoded");
        }
        this.secretKey = new SecretKeySpec(keyBytes, "AES");
    }

    @Override
    public String convertToDatabaseColumn(String attribute) {
        if (attribute == null) {
            return null;
        }
        try {
            byte[] iv = new byte[GCM_IV_LENGTH];
            secureRandom.nextBytes(iv);

            Cipher cipher = Cipher.getInstance(ALGORITHM);
            cipher.init(Cipher.ENCRYPT_MODE, secretKey, new GCMParameterSpec(GCM_TAG_LENGTH, iv));
            byte[] encrypted = cipher.doFinal(attribute.getBytes(StandardCharsets.UTF_8));

            byte[] combined = ByteBuffer.allocate(iv.length + encrypted.length)
                    .put(iv)
                    .put(encrypted)
                    .array();
            return Base64.getEncoder().encodeToString(combined);
        } catch (Exception e) {
            throw new RuntimeException("PII encryption failed", e);
        }
    }

    @Override
    public String convertToEntityAttribute(String dbData) {
        if (dbData == null) {
            return null;
        }
        try {
            byte[] combined = Base64.getDecoder().decode(dbData);
            ByteBuffer buffer = ByteBuffer.wrap(combined);

            byte[] iv = new byte[GCM_IV_LENGTH];
            buffer.get(iv);

            byte[] encrypted = new byte[buffer.remaining()];
            buffer.get(encrypted);

            Cipher cipher = Cipher.getInstance(ALGORITHM);
            cipher.init(Cipher.DECRYPT_MODE, secretKey, new GCMParameterSpec(GCM_TAG_LENGTH, iv));
            return new String(cipher.doFinal(encrypted), StandardCharsets.UTF_8);
        } catch (Exception e) {
            throw new RuntimeException("PII decryption failed", e);
        }
    }
}
