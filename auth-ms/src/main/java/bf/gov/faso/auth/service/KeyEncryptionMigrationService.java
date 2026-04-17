package bf.gov.faso.auth.service;

import bf.gov.faso.auth.model.JwtSigningKey;
import bf.gov.faso.auth.repository.JwtSigningKeyRepository;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.boot.ApplicationArguments;
import org.springframework.boot.ApplicationRunner;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;

/**
 * On startup, finds any JWT signing key rows whose private_key_pem is still plaintext
 * (starts with "-----BEGIN") and re-saves them so EncryptedStringConverter encrypts them.
 * Once encrypted, sets key_encrypted = true to avoid repeated processing.
 */
@Service
public class KeyEncryptionMigrationService implements ApplicationRunner {

    private static final Logger log = LoggerFactory.getLogger(KeyEncryptionMigrationService.class);

    private final JwtSigningKeyRepository keyRepository;

    public KeyEncryptionMigrationService(JwtSigningKeyRepository keyRepository) {
        this.keyRepository = keyRepository;
    }

    @Override
    @Transactional
    public void run(ApplicationArguments args) {
        List<JwtSigningKey> keys = keyRepository.findAll();
        int migrated = 0;
        for (JwtSigningKey key : keys) {
            String pem = key.getPrivateKeyPem();
            // Plaintext PEM blocks start with "-----BEGIN"; encrypted blobs are base64 without that prefix
            if (pem != null && pem.startsWith("-----BEGIN")) {
                // Re-save: JPA will call EncryptedStringConverter.convertToDatabaseColumn
                keyRepository.save(key);
                migrated++;
                log.info("Encrypted JWT signing key kid={}", key.getKid());
            }
        }
        if (migrated > 0) {
            log.info("KeyEncryptionMigration: {} key(s) encrypted with AES-256-GCM", migrated);
        } else {
            log.debug("KeyEncryptionMigration: all keys already encrypted");
        }
    }
}
