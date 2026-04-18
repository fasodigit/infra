package bf.gov.faso.impression.crypto;

import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import javax.crypto.Cipher;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.security.SecureRandom;
import java.util.Base64;

/**
 * Service de chiffrement AES-256-GCM pour les documents PDF generes.
 *
 * Format du fichier chiffre: [nonce 12B][ciphertext][GCM tag 16B]
 * La cle est chargee depuis la variable d'environnement EC_AES_GCM_KEY (Base64, 32 bytes).
 */
@Service
public class FileEncryptionService {

    private static final Logger log = LoggerFactory.getLogger(FileEncryptionService.class);

    private static final String ALGORITHM = "AES/GCM/NoPadding";
    private static final int GCM_NONCE_LENGTH = 12;
    private static final int GCM_TAG_LENGTH_BITS = 128;

    private final String base64Key;
    private final SecureRandom secureRandom = new SecureRandom();
    private SecretKeySpec secretKey;

    public FileEncryptionService(@Value("${encryption.key:}") String base64Key) {
        this.base64Key = base64Key;
    }

    @PostConstruct
    void init() {
        if (base64Key == null || base64Key.isBlank()) {
            log.warn("EC_AES_GCM_KEY not set — file encryption DISABLED (dev mode only)");
            return;
        }
        byte[] keyBytes = Base64.getDecoder().decode(base64Key);
        if (keyBytes.length != 32) {
            throw new IllegalStateException("EC_AES_GCM_KEY must be 32 bytes (256 bits), got " + keyBytes.length);
        }
        this.secretKey = new SecretKeySpec(keyBytes, "AES");
        log.info("FileEncryptionService initialized with AES-256-GCM");
    }

    /** Returns true if encryption key is configured and ready. */
    public boolean isEnabled() {
        return secretKey != null;
    }

    /**
     * Chiffre les donnees en AES-256-GCM.
     *
     * @param plaintext donnees en clair
     * @return nonce (12B) || ciphertext || GCM tag (16B)
     */
    public byte[] encrypt(byte[] plaintext) {
        if (!isEnabled()) {
            log.debug("Encryption disabled — returning plaintext");
            return plaintext;
        }
        try {
            byte[] nonce = new byte[GCM_NONCE_LENGTH];
            secureRandom.nextBytes(nonce);

            Cipher cipher = Cipher.getInstance(ALGORITHM);
            cipher.init(Cipher.ENCRYPT_MODE, secretKey, new GCMParameterSpec(GCM_TAG_LENGTH_BITS, nonce));

            byte[] ciphertextWithTag = cipher.doFinal(plaintext);

            byte[] result = new byte[GCM_NONCE_LENGTH + ciphertextWithTag.length];
            System.arraycopy(nonce, 0, result, 0, GCM_NONCE_LENGTH);
            System.arraycopy(ciphertextWithTag, 0, result, GCM_NONCE_LENGTH, ciphertextWithTag.length);
            return result;
        } catch (Exception e) {
            throw new IllegalStateException("Encryption failed", e);
        }
    }

    /**
     * Dechiffre les donnees chiffrees en AES-256-GCM.
     *
     * @param encrypted nonce (12B) || ciphertext || GCM tag (16B)
     * @return donnees en clair
     */
    public byte[] decrypt(byte[] encrypted) {
        if (!isEnabled()) {
            log.debug("Encryption disabled — returning data as-is");
            return encrypted;
        }
        try {
            if (encrypted.length < GCM_NONCE_LENGTH) {
                throw new IllegalArgumentException("Encrypted data too short");
            }

            byte[] nonce = new byte[GCM_NONCE_LENGTH];
            System.arraycopy(encrypted, 0, nonce, 0, GCM_NONCE_LENGTH);

            byte[] ciphertextWithTag = new byte[encrypted.length - GCM_NONCE_LENGTH];
            System.arraycopy(encrypted, GCM_NONCE_LENGTH, ciphertextWithTag, 0, ciphertextWithTag.length);

            Cipher cipher = Cipher.getInstance(ALGORITHM);
            cipher.init(Cipher.DECRYPT_MODE, secretKey, new GCMParameterSpec(GCM_TAG_LENGTH_BITS, nonce));

            return cipher.doFinal(ciphertextWithTag);
        } catch (Exception e) {
            throw new IllegalStateException("Decryption failed", e);
        }
    }
}
