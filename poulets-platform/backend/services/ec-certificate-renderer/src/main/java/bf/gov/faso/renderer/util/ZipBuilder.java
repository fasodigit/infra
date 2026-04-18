package bf.gov.faso.renderer.util;

import bf.gov.faso.renderer.service.PdfRenderService.BatchRenderResult;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

public final class ZipBuilder {

    private static final Logger log = LoggerFactory.getLogger(ZipBuilder.class);

    private ZipBuilder() {}

    public static byte[] build(List<BatchRenderResult> results) throws IOException {
        ByteArrayOutputStream baos = new ByteArrayOutputStream(estimateSize(results));

        try (ZipOutputStream zip = new ZipOutputStream(baos, StandardCharsets.UTF_8)) {
            zip.setLevel(1);

            StringBuilder errors = new StringBuilder();
            int successCount = 0;

            for (BatchRenderResult result : results) {
                if (result.success() && result.pdf() != null) {
                    String entryName = sanitizeFilename(result.filename(), result.index());
                    ZipEntry entry = new ZipEntry(entryName);
                    entry.setSize(result.pdf().length);
                    zip.putNextEntry(entry);
                    zip.write(result.pdf());
                    zip.closeEntry();
                    successCount++;
                } else {
                    errors.append("ERREUR [").append(result.index()).append("] ")
                          .append(result.filename()).append(" : ")
                          .append(result.errorMessage())
                          .append(System.lineSeparator());
                }
            }

            if (!errors.isEmpty()) {
                ZipEntry errEntry = new ZipEntry("ERRORS.txt");
                byte[] errBytes = errors.toString().getBytes(StandardCharsets.UTF_8);
                errEntry.setSize(errBytes.length);
                zip.putNextEntry(errEntry);
                zip.write(errBytes);
                zip.closeEntry();
            }

            log.info("ZIP built — {}/{} PDFs, {} bytes estimated",
                    successCount, results.size(), baos.size());
        }

        return baos.toByteArray();
    }

    private static String sanitizeFilename(String filename, int index) {
        if (filename == null || filename.isBlank()) {
            return String.format("document-%04d.pdf", index);
        }
        String safe = filename.replaceAll("[^a-zA-Z0-9._\\-]", "_");
        if (!safe.toLowerCase().endsWith(".pdf")) {
            safe += ".pdf";
        }
        return safe;
    }

    private static int estimateSize(List<BatchRenderResult> results) {
        return results.size() * 200 * 1024 + 4096;
    }
}
