package bf.gov.faso.impression.controller;

import bf.gov.faso.impression.entity.PrintJob;
import bf.gov.faso.impression.repository.PrintJobRepository;
import bf.gov.faso.impression.service.QrVerificationService;
import bf.gov.faso.impression.service.QrVerificationService.VerificationResult;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Public verification endpoint for QR code scanning.
 * Returns a styled HTML page showing document authenticity.
 *
 * Deux modes de verification:
 * - Legacy: /verify?id={documentId}&hash={documentHash} (retrocompatibilite)
 * - Signe: /verify-qr?code={base64_hmac_signed_code} (securite cryptographique)
 *
 * Rate limiting: 60 requetes/minute par IP sur les deux endpoints.
 */
@RestController
@RequestMapping("/api/v1/verification")
public class VerificationController {

    private static final Logger log = LoggerFactory.getLogger(VerificationController.class);
    private static final DateTimeFormatter DATE_FMT = DateTimeFormatter.ofPattern("dd/MM/yyyy 'a' HH:mm")
            .withZone(ZoneId.of("Africa/Ouagadougou"));

    private static final int MAX_REQUESTS_PER_MINUTE = 60;

    private final PrintJobRepository printJobRepository;
    private final QrVerificationService qrVerificationService;

    /** Rate limiting: IP -> (count, windowStart) */
    private final ConcurrentHashMap<String, long[]> rateLimitMap = new ConcurrentHashMap<>();

    public VerificationController(
            PrintJobRepository printJobRepository,
            QrVerificationService qrVerificationService) {
        this.printJobRepository = printJobRepository;
        this.qrVerificationService = qrVerificationService;
    }

    /**
     * Endpoint legacy: verification par documentId et hash.
     * Maintenu pour retrocompatibilite avec les documents existants.
     */
    @GetMapping(value = "/verify", produces = MediaType.TEXT_HTML_VALUE)
    public ResponseEntity<String> verifyDocument(
            @RequestParam(name = "id", required = false) String documentIdStr,
            @RequestParam(name = "hash", required = false) String hash,
            @RequestHeader(value = "X-Forwarded-For", required = false) String forwardedFor,
            @RequestHeader(value = "X-Real-IP", required = false) String realIp) {

        // Rate limiting
        String clientIp = resolveClientIp(forwardedFor, realIp);
        if (isRateLimited(clientIp)) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(renderPage(null, false,
                            "Trop de requetes. Veuillez reessayer dans une minute."));
        }

        log.info("Public verification request (legacy): id={}", documentIdStr);

        if (documentIdStr == null || documentIdStr.isBlank()) {
            return ResponseEntity.ok(renderPage(null, false, "Lien de verification invalide. Aucun identifiant fourni."));
        }

        UUID documentId;
        try {
            documentId = UUID.fromString(documentIdStr);
        } catch (IllegalArgumentException e) {
            return ResponseEntity.ok(renderPage(null, false, "Identifiant de document invalide."));
        }

        List<PrintJob> jobs = printJobRepository.findByDocumentId(documentId);
        if (jobs.isEmpty()) {
            return ResponseEntity.ok(renderPage(null, false,
                    "Document non trouve dans le registre officiel. Ce document pourrait etre un faux."));
        }

        // Use the most recent WORM-locked job
        PrintJob job = jobs.stream()
                .filter(PrintJob::isWormLocked)
                .findFirst()
                .orElse(jobs.getFirst());

        // Verify hash if provided
        boolean hashMatch = true;
        if (hash != null && !hash.isBlank() && job.getDocumentHash() != null) {
            hashMatch = hash.equalsIgnoreCase(job.getDocumentHash());
        }

        if (!hashMatch) {
            return ResponseEntity.ok(renderPage(job, false,
                    "L'empreinte numerique ne correspond pas. Ce document a pu etre modifie ou falsifie."));
        }

        return ResponseEntity.ok(renderPage(job, true, null));
    }

    /**
     * Endpoint signe: verification par code HMAC-SHA256.
     * Le code est genere par validation-acte-service et integre dans le QR code du PDF.
     *
     * Format du code: Base64(demandeId:tenantId:timestamp:hmac)
     */
    @GetMapping(value = "/verify-qr", produces = MediaType.TEXT_HTML_VALUE)
    public ResponseEntity<String> verifyQrCode(
            @RequestParam(name = "code", required = false) String code,
            @RequestHeader(value = "X-Forwarded-For", required = false) String forwardedFor,
            @RequestHeader(value = "X-Real-IP", required = false) String realIp) {

        // Rate limiting
        String clientIp = resolveClientIp(forwardedFor, realIp);
        if (isRateLimited(clientIp)) {
            return ResponseEntity.status(HttpStatus.TOO_MANY_REQUESTS)
                    .body(renderPage(null, false,
                            "Trop de requetes. Veuillez reessayer dans une minute."));
        }

        log.info("Public verification request (HMAC-signed QR) from IP={}", clientIp);

        if (code == null || code.isBlank()) {
            return ResponseEntity.ok(renderPage(null, false,
                    "Code de verification manquant. Scannez le QR code du document."));
        }

        VerificationResult result = qrVerificationService.verifyQrCode(code);

        if (!result.valid()) {
            String errorMessage = switch (result.status()) {
                case "SIGNATURE_INVALIDE" ->
                    "DOCUMENT FALSIFIE — La signature cryptographique ne correspond pas. "
                    + "Ce document n'a pas ete emis par la Plateforme Actes.";
                case "FORMAT_INVALIDE", "ENCODAGE_INVALIDE" ->
                    "Code de verification invalide. Le QR code est endommage ou corrompu.";
                case "CODE_VIDE" ->
                    "Code de verification manquant. Scannez le QR code du document.";
                default ->
                    "Verification impossible. Veuillez reessayer ou contacter le support.";
            };

            log.warn("QR verification failed: status={}, ip={}", result.status(), clientIp);
            return ResponseEntity.ok(renderPage(null, false, errorMessage));
        }

        // HMAC valide — le document est authentique
        PrintJob job = result.printJob();
        if (job == null) {
            // Code valide mais document pas encore imprime
            return ResponseEntity.ok(renderPage(null, true,
                    "Ce document est authentique mais n'a pas encore ete imprime. "
                    + "Veuillez contacter le service d'etat civil."));
        }

        return ResponseEntity.ok(renderPage(job, true, null));
    }

    // --- Rate limiting ---

    /**
     * Verifie si l'IP est rate-limitee (60 req/min).
     * Nettoyage automatique des fenetres expirees.
     */
    private boolean isRateLimited(String ip) {
        long now = System.currentTimeMillis();
        long windowMs = 60_000L;

        long[] bucket = rateLimitMap.compute(ip, (key, existing) -> {
            if (existing == null || (now - existing[1]) > windowMs) {
                // Nouvelle fenetre
                return new long[]{1, now};
            }
            existing[0]++;
            return existing;
        });

        if (bucket[0] > MAX_REQUESTS_PER_MINUTE) {
            log.warn("Rate limit exceeded for IP={} ({} requests)", ip, bucket[0]);
            return true;
        }
        return false;
    }

    private String resolveClientIp(String forwardedFor, String realIp) {
        if (forwardedFor != null && !forwardedFor.isBlank()) {
            return forwardedFor.split(",")[0].trim();
        }
        if (realIp != null && !realIp.isBlank()) {
            return realIp;
        }
        return "unknown";
    }

    // --- HTML rendering ---

    private String renderPage(PrintJob job, boolean authentic, String errorMessage) {
        String statusColor = authentic ? "#009639" : "#EF2B2D";
        String statusIcon = authentic ? "&#10003;" : "&#10007;";
        String statusTitle = authentic ? "DOCUMENT AUTHENTIQUE" : "VERIFICATION ECHOUEE";
        String statusSubtitle = authentic
                ? "Ce document est enregistre dans le registre officiel de l'Etat Civil."
                : errorMessage;

        Map<String, Object> meta = job != null && job.getMetadata() != null ? job.getMetadata() : Map.of();

        String nom = strVal(meta, "nom", "");
        String prenoms = strVal(meta, "prenoms", "");
        String dateNaissance = strVal(meta, "dateNaissance", "");
        String lieuNaissance = strVal(meta, "lieuNaissance", "");

        String documentType = job != null ? formatDocumentType(job.getDocumentType()) : "";
        String documentRef = job != null && job.getDocumentReference() != null ? job.getDocumentReference() : "";
        String numero = strVal(meta, "numero", strVal(meta, "numeroActe", ""));
        String printedDate = job != null && job.getPrintedAt() != null ? DATE_FMT.format(job.getPrintedAt()) : "";
        String docHash = job != null && job.getDocumentHash() != null ? job.getDocumentHash() : "";
        String blockHash = job != null && job.getBlockchainHash() != null ? job.getBlockchainHash() : "";
        boolean wormLocked = job != null && job.isWormLocked();
        boolean hmacVerified = job != null && job.getQrVerificationCode() != null;

        StringBuilder sb = new StringBuilder();
        sb.append("""
            <!DOCTYPE html>
            <html lang="fr">
            <head>
                <meta charset="UTF-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
                <title>Verification - Plateforme Actes</title>
                <style>
                    * { margin: 0; padding: 0; box-sizing: border-box; }
                    body {
                        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                        background: #f5f5f5;
                        color: #333;
                        min-height: 100vh;
                    }
                    .flag-bar {
                        height: 6px;
                        background: linear-gradient(to right, #EF2B2D 33.33%, #FCD116 33.33%, #FCD116 66.66%, #009639 66.66%);
                    }
                    .header {
                        background: #1a1a2e;
                        color: white;
                        padding: 20px;
                        text-align: center;
                    }
                    .header h1 { font-size: 18px; font-weight: 600; letter-spacing: 1px; }
                    .header p { font-size: 12px; opacity: 0.7; margin-top: 4px; font-style: italic; }
                    .container { max-width: 600px; margin: 0 auto; padding: 24px 16px; }
                    .status-card {
                        background: white;
                        border-radius: 12px;
                        box-shadow: 0 2px 12px rgba(0,0,0,0.08);
                        overflow: hidden;
                        margin-bottom: 20px;
                    }
                    .status-banner {
                        padding: 32px 24px;
                        text-align: center;
                        color: white;
                    }
                    .status-icon {
                        width: 64px; height: 64px;
                        border-radius: 50%;
                        background: rgba(255,255,255,0.2);
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        font-size: 32px;
                        margin-bottom: 16px;
                    }
                    .status-title { font-size: 20px; font-weight: 700; letter-spacing: 1px; }
                    .status-subtitle {
                        font-size: 14px;
                        margin-top: 8px;
                        opacity: 0.9;
                        line-height: 1.5;
                        max-width: 400px;
                        margin-left: auto;
                        margin-right: auto;
                    }
                    .details { padding: 24px; }
                    .detail-section {
                        margin-bottom: 20px;
                        padding-bottom: 20px;
                        border-bottom: 1px solid #eee;
                    }
                    .detail-section:last-child { border-bottom: none; margin-bottom: 0; padding-bottom: 0; }
                    .section-title {
                        font-size: 11px;
                        font-weight: 700;
                        text-transform: uppercase;
                        letter-spacing: 1.5px;
                        color: #888;
                        margin-bottom: 12px;
                    }
                    .detail-row {
                        display: flex;
                        justify-content: space-between;
                        padding: 6px 0;
                    }
                    .detail-label { color: #666; font-size: 14px; }
                    .detail-value { font-weight: 600; font-size: 14px; text-align: right; max-width: 60%; }
                    .badge {
                        display: inline-block;
                        padding: 3px 10px;
                        border-radius: 20px;
                        font-size: 11px;
                        font-weight: 700;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                    }
                    .badge-green { background: #e8f5e9; color: #2e7d32; }
                    .badge-red { background: #fce4ec; color: #c62828; }
                    .badge-blue { background: #e3f2fd; color: #1565c0; }
                    .security-list { list-style: none; }
                    .security-list li {
                        padding: 8px 0;
                        font-size: 13px;
                        display: flex;
                        align-items: center;
                        gap: 10px;
                    }
                    .check { color: #2e7d32; font-weight: bold; font-size: 16px; }
                    .cross { color: #c62828; font-weight: bold; font-size: 16px; }
                    .hash-value {
                        font-family: 'Courier New', monospace;
                        font-size: 10px;
                        color: #666;
                        word-break: break-all;
                        background: #f8f8f8;
                        padding: 8px;
                        border-radius: 6px;
                        margin-top: 4px;
                    }
                    .footer {
                        text-align: center;
                        padding: 20px;
                        font-size: 11px;
                        color: #999;
                    }
                    .footer a { color: #1565c0; text-decoration: none; }
                </style>
            </head>
            <body>
                <div class="flag-bar"></div>
                <div class="header">
                    <h1>PLATEFORME ACTES - BURKINA FASO</h1>
                    <p>Systeme de verification des documents officiels</p>
                </div>
                <div class="container">
                    <div class="status-card">
            """);

        // Status banner
        sb.append("<div class=\"status-banner\" style=\"background:").append(statusColor).append("\">");
        sb.append("<div class=\"status-icon\">").append(statusIcon).append("</div>");
        sb.append("<div class=\"status-title\">").append(statusTitle).append("</div>");
        sb.append("<div class=\"status-subtitle\">").append(statusSubtitle).append("</div>");
        sb.append("</div>");

        if (job != null) {
            sb.append("<div class=\"details\">");

            // Document info section
            if (!documentType.isEmpty() || !documentRef.isEmpty()) {
                sb.append("<div class=\"detail-section\">");
                sb.append("<div class=\"section-title\">Informations du document</div>");
                if (!documentType.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">Type</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(documentType)).append("</span></div>");
                }
                if (!documentRef.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">Reference</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(documentRef)).append("</span></div>");
                }
                if (!numero.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">N&#176; Acte</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(numero)).append("</span></div>");
                }
                if (!printedDate.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">Imprime le</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(printedDate)).append("</span></div>");
                }
                sb.append("</div>");
            }

            // Beneficiary section
            if (!nom.isEmpty() || !prenoms.isEmpty()) {
                sb.append("<div class=\"detail-section\">");
                sb.append("<div class=\"section-title\">Beneficiaire</div>");
                if (!nom.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">Nom</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(nom)).append("</span></div>");
                }
                if (!prenoms.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">Prenom(s)</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(prenoms)).append("</span></div>");
                }
                if (!dateNaissance.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">Ne(e) le</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(dateNaissance)).append("</span></div>");
                }
                if (!lieuNaissance.isEmpty()) {
                    sb.append("<div class=\"detail-row\"><span class=\"detail-label\">A</span>");
                    sb.append("<span class=\"detail-value\">").append(esc(lieuNaissance)).append("</span></div>");
                }
                sb.append("</div>");
            }

            // Security section
            sb.append("<div class=\"detail-section\">");
            sb.append("<div class=\"section-title\">Securite du document</div>");
            sb.append("<ul class=\"security-list\">");

            sb.append("<li>");
            sb.append(hmacVerified ? "<span class=\"check\">&#10003;</span>" : "<span class=\"cross\">&#10007;</span>");
            sb.append(" Signature HMAC-SHA256 verifiee</li>");

            sb.append("<li>");
            sb.append(wormLocked ? "<span class=\"check\">&#10003;</span>" : "<span class=\"cross\">&#10007;</span>");
            sb.append(" Archivage WORM (document inalterable)</li>");

            sb.append("<li>");
            sb.append(!docHash.isEmpty() ? "<span class=\"check\">&#10003;</span>" : "<span class=\"cross\">&#10007;</span>");
            sb.append(" Empreinte numerique SHA-256</li>");

            sb.append("<li>");
            sb.append(!blockHash.isEmpty() ? "<span class=\"check\">&#10003;</span>" : "<span class=\"cross\">&#10007;</span>");
            sb.append(" Trace blockchain enregistree</li>");

            sb.append("<li><span class=\"check\">&#10003;</span> QR Code de verification cryptographique</li>");
            sb.append("<li><span class=\"check\">&#10003;</span> Filigrane de securite officiel</li>");
            sb.append("</ul>");

            if (!docHash.isEmpty()) {
                sb.append("<div style=\"margin-top:12px\">");
                sb.append("<span class=\"detail-label\">Empreinte SHA-256</span>");
                sb.append("<div class=\"hash-value\">").append(esc(docHash)).append("</div>");
                sb.append("</div>");
            }
            if (!blockHash.isEmpty()) {
                sb.append("<div style=\"margin-top:8px\">");
                sb.append("<span class=\"detail-label\">Hash blockchain</span>");
                sb.append("<div class=\"hash-value\">").append(esc(blockHash)).append("</div>");
                sb.append("</div>");
            }

            sb.append("</div>"); // security section
            sb.append("</div>"); // details
        }

        sb.append("""
                    </div>
                    <div class="footer">
                        <p>Republique du Burkina Faso &mdash; Unite - Progres - Justice</p>
                        <p style="margin-top:6px">Plateforme Actes &mdash; Systeme de gestion des actes d'etat civil</p>
                        <p style="margin-top:6px">
                            <a href="https://actes.gov.bf">actes.gov.bf</a> &bull;
                            Contact: support@actes.gov.bf
                        </p>
                    </div>
                </div>
            </body>
            </html>
            """);

        return sb.toString();
    }

    private static String formatDocumentType(String type) {
        if (type == null) return "";
        return switch (type) {
            case "ACTE_NAISSANCE" -> "Extrait d'Acte de Naissance";
            case "ACTE_MARIAGE" -> "Acte de Mariage";
            case "ACTE_DECES" -> "Acte de Deces";
            case "ACTE_DIVERS" -> "Acte Divers";
            case "PERMIS_PORT_ARMES" -> "Permis de Port d'Armes";
            default -> type.replace('_', ' ');
        };
    }

    private static String strVal(Map<String, Object> map, String key, String def) {
        Object v = map.get(key);
        return v != null ? v.toString() : def;
    }

    private static String esc(String s) {
        if (s == null) return "";
        return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;");
    }
}
