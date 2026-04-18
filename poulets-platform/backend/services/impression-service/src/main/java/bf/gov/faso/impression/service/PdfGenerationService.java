package bf.gov.faso.impression.service;

import java.util.Map;
import java.util.UUID;

/**
 * Service interface for PDF generation operations.
 *
 * Generates PDF documents from templates with data binding and watermarking.
 */
public interface PdfGenerationService {

    /**
     * Generate a PDF from a template.
     *
     * @param templateName  The template name
     * @param data          The data to bind to the template
     * @param tenantId      The tenant ID
     * @return The generated PDF bytes
     */
    byte[] generatePdf(String templateName, Map<String, Object> data, String tenantId);

    /**
     * Generate a PDF from a template with watermark.
     *
     * @param templateName   The template name
     * @param data           The data to bind to the template
     * @param watermarkText  The watermark text
     * @param tenantId       The tenant ID
     * @return The generated PDF bytes with watermark
     */
    byte[] generatePdfWithWatermark(
        String templateName,
        Map<String, Object> data,
        String watermarkText,
        String tenantId
    );

    /**
     * Add a watermark to an existing PDF.
     *
     * @param pdfBytes       The original PDF bytes
     * @param watermarkText  The watermark text
     * @param documentId     The document ID (for logging)
     * @param tenantId       The tenant ID
     * @return The PDF bytes with watermark
     */
    byte[] addWatermark(byte[] pdfBytes, String watermarkText, UUID documentId, String tenantId);

    /**
     * Add a watermark using the document-security-ms gRPC service.
     *
     * @param pdfBytes       The original PDF bytes
     * @param watermarkText  The watermark text
     * @param documentId     The document ID
     * @param tenantId       The tenant ID
     * @return The PDF bytes with watermark
     */
    byte[] addWatermarkViaGrpc(byte[] pdfBytes, String watermarkText, UUID documentId, String tenantId);

    /**
     * Calculate the SHA-256 hash of a PDF.
     *
     * @param pdfBytes  The PDF bytes
     * @return The SHA-256 hash as hex string
     */
    String calculateHash(byte[] pdfBytes);

    /**
     * Verify the hash of a PDF.
     *
     * @param pdfBytes      The PDF bytes
     * @param expectedHash  The expected hash
     * @return True if the hash matches
     */
    boolean verifyHash(byte[] pdfBytes, String expectedHash);

    /**
     * Get the list of available templates.
     *
     * @param tenantId  The tenant ID
     * @return List of template names
     */
    java.util.List<String> getAvailableTemplates(String tenantId);
}
