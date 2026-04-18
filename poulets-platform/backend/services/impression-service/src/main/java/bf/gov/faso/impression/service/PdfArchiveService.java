package bf.gov.faso.impression.service;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.LocalDate;
import java.time.format.DateTimeFormatter;
import java.util.UUID;

/**
 * Saves generated secure PDFs (with QR code + watermark) to the DOCS-ETAT-CIVIL archive directory.
 * Structure: {basePath}/{demandeId}/{YYYY-MM-DD}/{documentType}/{reference}__{documentId}.pdf
 */
@Service
public class PdfArchiveService {

    private static final Logger log = LoggerFactory.getLogger(PdfArchiveService.class);
    private static final DateTimeFormatter DATE_FMT = DateTimeFormatter.ofPattern("yyyy-MM-dd");

    private final Path basePath;
    private final boolean enabled;

    public PdfArchiveService(
            @Value("${impression.archive.base-path:}") String basePath,
            @Value("${impression.archive.enabled:false}") boolean enabled) {
        this.basePath = basePath.isBlank() ? null : Path.of(basePath);
        this.enabled = enabled && this.basePath != null;
        if (this.enabled) {
            log.info("PDF archive enabled: {}", this.basePath);
        }
    }

    /**
     * Saves the plaintext (unencrypted) PDF with QR code and watermark to the archive directory.
     *
     * @param pdfBytes      plaintext PDF bytes (after watermark, before encryption)
     * @param demandeId     UUIDv7 of the demande (top-level directory)
     * @param documentType  e.g. NAISSANCE, MARIAGE, DECES, PERMIS_ARMES, ACTES_DIVERS
     * @param documentId    unique document UUID
     * @param reference     document reference string (used in filename)
     * @return the saved file path, or null if archiving is disabled/failed
     */
    public Path archive(byte[] pdfBytes, UUID demandeId, String documentType, UUID documentId, String reference) {
        if (!enabled || pdfBytes == null || pdfBytes.length == 0) {
            return null;
        }

        try {
            String dateDir = LocalDate.now().format(DATE_FMT);
            String safeType = sanitize(documentType != null ? documentType : "DIVERS");
            String safeRef = sanitize(reference != null ? reference : documentId.toString());
            String fileName = safeRef + "__" + documentId + ".pdf";

            // Structure: {basePath}/{demandeId}/{date}/{type}/
            Path dir = basePath
                    .resolve(demandeId.toString())
                    .resolve(dateDir)
                    .resolve(safeType);
            Files.createDirectories(dir);

            Path target = dir.resolve(fileName);
            Files.write(target, pdfBytes,
                    StandardOpenOption.CREATE,
                    StandardOpenOption.TRUNCATE_EXISTING,
                    StandardOpenOption.WRITE);

            log.info("PDF archived: {} ({} bytes)", target, pdfBytes.length);
            return target;

        } catch (IOException e) {
            log.error("Failed to archive PDF for document {}: {}", documentId, e.getMessage());
            return null;
        }
    }

    private static String sanitize(String name) {
        return name.replaceAll("[^a-zA-Z0-9_\\-.]", "_");
    }
}
