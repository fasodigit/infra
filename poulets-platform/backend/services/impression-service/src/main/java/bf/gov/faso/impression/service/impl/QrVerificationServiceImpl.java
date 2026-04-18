package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.repository.PrintJobRepository;
import bf.gov.faso.impression.service.QrVerificationService;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.security.InvalidKeyException;
import java.security.NoSuchAlgorithmException;
import java.util.Base64;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

/**
 * Implementation du service de verification des QR codes HMAC-signes.
 *
 * Utilise le meme algorithme HMAC-SHA256 que QrCodeServiceImpl
 * dans validation-acte-service pour verifier l'authenticite.
 */
@Service
public class QrVerificationServiceImpl implements QrVerificationService {

    private static final Logger log = LoggerFactory.getLogger(QrVerificationServiceImpl.class);
    private static final String HMAC_ALGORITHM = "HmacSHA256";
    private static final String SEPARATOR = ":";
    private static final String DEFAULT_SECRET = "change-me-in-production";

    private final PrintJobRepository printJobRepository;
    private final String hmacSecret;
    private final String activeProfile;

    public QrVerificationServiceImpl(
            PrintJobRepository printJobRepository,
            @Value("${verification.hmac-secret:change-me-in-production}") String hmacSecret,
            @Value("${spring.profiles.active:dev}") String activeProfile) {
        this.printJobRepository = printJobRepository;
        this.hmacSecret = hmacSecret;
        this.activeProfile = activeProfile;
    }

    @PostConstruct
    void validateConfig() {
        if (!"dev".equals(activeProfile) && !"test".equals(activeProfile)
                && DEFAULT_SECRET.equals(hmacSecret)) {
            throw new IllegalStateException(
                    "QR_HMAC_SECRET must be configured in production! "
                    + "Set the environment variable QR_HMAC_SECRET with a secure key.");
        }
        log.info("QR verification service initialized (profile={})", activeProfile);
    }

    @Override
    public VerificationResult verifyQrCode(String encodedCode) {
        if (encodedCode == null || encodedCode.isBlank()) {
            log.warn("Verification attempt with empty code");
            return VerificationResult.invalid("CODE_VIDE");
        }

        try {
            // 1. Decoder le Base64 URL-safe
            String decoded = new String(
                    Base64.getUrlDecoder().decode(encodedCode),
                    StandardCharsets.UTF_8);

            // 2. Extraire les composants: demandeId:tenantId:timestamp:hmac
            String[] parts = decoded.split(SEPARATOR);
            if (parts.length < 4) {
                log.warn("Code de verification invalide: format incorrect ({} parties)", parts.length);
                return VerificationResult.invalid("FORMAT_INVALIDE");
            }

            String demandeIdStr = parts[0];
            String tenantId = parts[1];
            String timestampStr = parts[2];
            String providedHmac = parts[3];

            // 3. Recalculer le HMAC-SHA256
            String payload = demandeIdStr + SEPARATOR + tenantId + SEPARATOR + timestampStr;
            String calculatedHmac = calculateHmac(payload);

            // 4. Comparaison en temps constant pour eviter les timing attacks
            if (!constantTimeEquals(providedHmac, calculatedHmac)) {
                log.warn("Verification HMAC echouee pour demande={}", demandeIdStr);
                return VerificationResult.invalid("SIGNATURE_INVALIDE");
            }

            // 5. Extraire les donnees verifiees
            UUID demandeId;
            try {
                demandeId = UUID.fromString(demandeIdStr);
            } catch (IllegalArgumentException e) {
                log.warn("UUID invalide dans le code de verification: {}", demandeIdStr);
                return VerificationResult.invalid("IDENTIFIANT_INVALIDE");
            }

            long timestamp;
            try {
                timestamp = Long.parseLong(timestampStr);
            } catch (NumberFormatException e) {
                log.warn("Timestamp invalide dans le code de verification: {}", timestampStr);
                return VerificationResult.invalid("TIMESTAMP_INVALIDE");
            }

            // 6. Rechercher le PrintJob associe
            Optional<PrintJob> printJob = findByQrCode(encodedCode);
            if (printJob.isEmpty()) {
                // Fallback: recherche par demandeId (le code est valide meme si pas encore imprime)
                List<PrintJob> jobs = printJobRepository.findByDemandeIdAndTenantId(demandeId, tenantId);
                if (!jobs.isEmpty()) {
                    PrintJob job = jobs.stream()
                            .filter(PrintJob::isWormLocked)
                            .findFirst()
                            .orElse(jobs.getFirst());
                    return VerificationResult.authentic(demandeId, tenantId, timestamp, job);
                }
                // Le code HMAC est valide mais aucun job d'impression n'existe
                log.info("Code HMAC valide mais aucun job d'impression pour demande={}", demandeId);
                return VerificationResult.authentic(demandeId, tenantId, timestamp, null);
            }

            log.info("Verification reussie pour demande={}, tenant={}", demandeId, tenantId);
            return VerificationResult.authentic(demandeId, tenantId, timestamp, printJob.get());

        } catch (IllegalArgumentException e) {
            log.warn("Erreur de decodage Base64: {}", e.getMessage());
            return VerificationResult.invalid("ENCODAGE_INVALIDE");
        } catch (Exception e) {
            log.error("Erreur inattendue lors de la verification du QR code", e);
            return VerificationResult.invalid("ERREUR_INTERNE");
        }
    }

    @Override
    public Optional<PrintJob> findByQrCode(String qrVerificationCode) {
        return printJobRepository.findByQrVerificationCode(qrVerificationCode);
    }

    /**
     * Calcule le HMAC-SHA256 du payload.
     * Algorithme identique a QrCodeServiceImpl dans validation-acte-service.
     */
    private String calculateHmac(String payload) {
        try {
            Mac mac = Mac.getInstance(HMAC_ALGORITHM);
            SecretKeySpec secretKey = new SecretKeySpec(
                    hmacSecret.getBytes(StandardCharsets.UTF_8),
                    HMAC_ALGORITHM);
            mac.init(secretKey);
            byte[] hmacBytes = mac.doFinal(payload.getBytes(StandardCharsets.UTF_8));
            return Base64.getUrlEncoder().withoutPadding().encodeToString(hmacBytes);
        } catch (NoSuchAlgorithmException | InvalidKeyException e) {
            throw new RuntimeException("Failed to calculate HMAC", e);
        }
    }

    /**
     * Comparaison en temps constant pour eviter les timing attacks.
     * Algorithme identique a QrCodeServiceImpl dans validation-acte-service.
     */
    private boolean constantTimeEquals(String a, String b) {
        if (a == null || b == null) {
            return false;
        }
        if (a.length() != b.length()) {
            return false;
        }
        int result = 0;
        for (int i = 0; i < a.length(); i++) {
            result |= a.charAt(i) ^ b.charAt(i);
        }
        return result == 0;
    }
}
