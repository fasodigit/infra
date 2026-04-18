package bf.gov.faso.impression.service;

import bf.gov.faso.impression.entity.PrintJob;

import java.util.Optional;
import java.util.UUID;

/**
 * Service de verification des QR codes HMAC-signes.
 *
 * Valide l'authenticite cryptographique des codes de verification
 * generes par validation-acte-service (HMAC-SHA256).
 */
public interface QrVerificationService {

    /**
     * Resultat de la verification d'un code QR.
     */
    record VerificationResult(
        boolean valid,
        String status,
        UUID demandeId,
        String tenantId,
        long timestamp,
        PrintJob printJob
    ) {
        public static VerificationResult invalid(String status) {
            return new VerificationResult(false, status, null, null, 0, null);
        }

        public static VerificationResult authentic(UUID demandeId, String tenantId, long timestamp, PrintJob job) {
            return new VerificationResult(true, "AUTHENTIQUE", demandeId, tenantId, timestamp, job);
        }
    }

    /**
     * Verifie un code QR HMAC-signe.
     *
     * 1. Decode le Base64 URL-safe
     * 2. Extrait demandeId, tenantId, timestamp, hmac
     * 3. Recalcule le HMAC-SHA256 avec le secret partage
     * 4. Comparaison en temps constant
     * 5. Si valide: recherche le PrintJob associe
     *
     * @param encodedCode code Base64 URL-safe encode
     * @return resultat de la verification
     */
    VerificationResult verifyQrCode(String encodedCode);

    /**
     * Recherche un PrintJob par code de verification QR.
     *
     * @param qrVerificationCode le code HMAC-signe
     * @return le PrintJob si trouve
     */
    Optional<PrintJob> findByQrCode(String qrVerificationCode);
}
