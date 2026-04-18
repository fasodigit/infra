package bf.gov.faso.impression.service.impl;

import bf.gov.faso.impression.client.EcCertificateRendererClient;
import bf.gov.faso.impression.grpc.DocumentSecurityGrpcClient;
import bf.gov.faso.impression.service.PdfGenerationService;
import com.lowagie.text.*;
import com.lowagie.text.pdf.*;
import freemarker.template.Configuration;
import freemarker.template.Template;
import org.apache.commons.codec.digest.DigestUtils;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import org.xhtmlrenderer.pdf.ITextRenderer;

import com.google.zxing.BarcodeFormat;
import com.google.zxing.EncodeHintType;
import com.google.zxing.WriterException;
import com.google.zxing.common.BitMatrix;
import com.google.zxing.qrcode.QRCodeWriter;

import java.awt.Color;
import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.StringReader;
import java.io.StringWriter;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import javax.imageio.ImageIO;

/**
 * Implementation of PDF generation service.
 */
@Service
public class PdfGenerationServiceImpl implements PdfGenerationService {

    private static final Logger log = LoggerFactory.getLogger(PdfGenerationServiceImpl.class);
    private static final int QR_CODE_SIZE = 120;

    private final Configuration freemarkerConfig;
    private final DocumentSecurityGrpcClient documentSecurityClient;
    private final String verificationBaseUrl;
    private final EcCertificateRendererClient rendererClient;

    @Autowired
    public PdfGenerationServiceImpl(
            Configuration freemarkerConfig,
            @Autowired(required = false) DocumentSecurityGrpcClient documentSecurityClient,
            @org.springframework.beans.factory.annotation.Value("${impression.verification.base-url:http://localhost:8105/api/v1/validation/verify}")
            String verificationBaseUrl,
            @Autowired(required = false) EcCertificateRendererClient rendererClient) {
        this.freemarkerConfig = freemarkerConfig;
        this.documentSecurityClient = documentSecurityClient;
        this.verificationBaseUrl = verificationBaseUrl;
        this.rendererClient = rendererClient;
    }

    @Override
    public byte[] generatePdf(String templateName, Map<String, Object> data, String tenantId) {
        // Resolve template name: documentType values (NAISSANCE, MARIAGE) → Handlebars template names (ACTE_NAISSANCE, ACTE_MARIAGE)
        String resolvedTemplate = resolveTemplateName(templateName);
        log.info("Generating PDF from template {} (resolved: {}) for tenant {}", templateName, resolvedTemplate, tenantId);

        try {
            byte[] pdfBytes;

            // Try Chromium renderer first (rich HTML + CSS3)
            if (rendererClient != null) {
                try {
                    pdfBytes = rendererClient.render(resolvedTemplate, data);
                    log.info("PDF rendered via ec-certificate-renderer for template {}", resolvedTemplate);
                    // QR code is already embedded in the Handlebars template
                    return pdfBytes;
                } catch (Exception e) {
                    log.warn("Chromium renderer failed, falling back to Flying Saucer: {}", e.getMessage());
                }
            }

            // Fallback: FreeMarker + Flying Saucer (CSS2 only)
            String htmlContent = processTemplate(resolvedTemplate, data);
            pdfBytes = renderHtmlToPdf(htmlContent);
            pdfBytes = addQrCodeFooter(pdfBytes, data);
            return pdfBytes;

        } catch (Exception e) {
            log.error("Failed to generate PDF from template {}", templateName, e);
            throw new RuntimeException("PDF generation failed: " + e.getMessage(), e);
        }
    }

    @Override
    public byte[] generatePdfWithWatermark(
            String templateName,
            Map<String, Object> data,
            String watermarkText,
            String tenantId) {

        byte[] pdfBytes = generatePdf(templateName, data, tenantId);
        return addWatermark(pdfBytes, watermarkText, null, tenantId);
    }

    @Override
    public byte[] addWatermark(byte[] pdfBytes, String watermarkText, UUID documentId, String tenantId) {
        log.debug("Adding watermark to PDF: {}", watermarkText);

        try {
            PdfReader reader = new PdfReader(pdfBytes);
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            PdfStamper stamper = new PdfStamper(reader, out);

            // Get font for watermark
            BaseFont bf = BaseFont.createFont(BaseFont.HELVETICA, BaseFont.WINANSI, BaseFont.EMBEDDED);

            // Add watermark to each page
            int pages = reader.getNumberOfPages();
            for (int i = 1; i <= pages; i++) {
                Rectangle pageSize = reader.getPageSize(i);
                float width = pageSize.getWidth();
                float height = pageSize.getHeight();

                PdfContentByte over = stamper.getOverContent(i);
                over.saveState();

                // Set watermark properties
                PdfGState gs = new PdfGState();
                gs.setFillOpacity(0.2f);
                over.setGState(gs);
                over.setColorFill(Color.GRAY);

                // Add diagonal watermark
                over.beginText();
                over.setFontAndSize(bf, 40);
                over.setTextMatrix(30, 30);
                over.showTextAligned(
                    Element.ALIGN_CENTER,
                    watermarkText,
                    width / 2,
                    height / 2,
                    45 // Diagonal rotation
                );
                over.endText();

                // Add footer watermark
                over.setFontAndSize(bf, 10);
                over.showTextAligned(
                    Element.ALIGN_CENTER,
                    watermarkText,
                    width / 2,
                    30,
                    0
                );

                over.restoreState();
            }

            stamper.close();
            reader.close();

            log.debug("Watermark added successfully to {} pages", pages);
            return out.toByteArray();

        } catch (Exception e) {
            log.error("Failed to add watermark to PDF", e);
            throw new RuntimeException("Watermark addition failed: " + e.getMessage(), e);
        }
    }

    @Override
    public byte[] addWatermarkViaGrpc(byte[] pdfBytes, String watermarkText, UUID documentId, String tenantId) {
        log.info("Adding watermark via document-security-ms gRPC for document {}", documentId);

        if (documentSecurityClient != null) {
            try {
                return documentSecurityClient.addWatermark(pdfBytes, watermarkText, documentId, tenantId);
            } catch (Exception e) {
                log.warn("gRPC watermark failed, falling back to local: {}", e.getMessage());
            }
        }

        // Fallback to local implementation
        return addWatermark(pdfBytes, watermarkText, documentId, tenantId);
    }

    @Override
    public String calculateHash(byte[] pdfBytes) {
        return DigestUtils.sha256Hex(pdfBytes);
    }

    @Override
    public boolean verifyHash(byte[] pdfBytes, String expectedHash) {
        String actualHash = calculateHash(pdfBytes);
        return actualHash.equals(expectedHash);
    }

    @Override
    public List<String> getAvailableTemplates(String tenantId) {
        // TODO: Load from database or filesystem based on tenant
        List<String> templates = new ArrayList<>();
        templates.add("ACTE_NAISSANCE");
        templates.add("ACTE_MARIAGE");
        templates.add("ACTE_DECES");
        templates.add("PERMIS_PORT_ARMES");
        templates.add("ACTE_DIVERS");
        return templates;
    }

    private String processTemplate(String templateName, Map<String, Object> data) throws Exception {
        // Try to load template from Freemarker configuration
        Template template;
        try {
            template = freemarkerConfig.getTemplate(templateName + ".ftl");
        } catch (Exception e) {
            // Fallback to basic template
            log.warn("Template {} not found, using default", templateName);
            template = new Template(
                "default",
                new StringReader(getDefaultTemplate()),
                freemarkerConfig
            );
        }

        StringWriter writer = new StringWriter();
        template.process(data, writer);
        return writer.toString();
    }

    /**
     * Render XHTML+CSS to PDF using Flying Saucer.
     */
    private byte[] renderHtmlToPdf(String htmlContent) throws Exception {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        ITextRenderer renderer = new ITextRenderer();
        renderer.setDocumentFromString(htmlContent);
        renderer.layout();
        renderer.createPDF(out);
        renderer.finishPDF();
        log.debug("Flying Saucer rendered HTML to PDF ({} bytes)", out.size());
        return out.toByteArray();
    }

    /**
     * Add QR code verification to the last page of an existing PDF.
     * Utilise l'URL de verification HMAC-signee si disponible, sinon fallback sur l'ancien format.
     */
    private byte[] addQrCodeFooter(byte[] pdfBytes, Map<String, Object> data) {
        try {
            // Priorite au code de verification HMAC-signe provenant de validation-acte-service
            String signedVerificationUrl = data.containsKey("verificationUrl")
                    ? data.get("verificationUrl").toString() : null;

            String verificationUrl;
            if (signedVerificationUrl != null && !signedVerificationUrl.isEmpty()) {
                // URL signee HMAC — securite cryptographique
                verificationUrl = signedVerificationUrl;
                log.debug("Using HMAC-signed verification URL for QR code");
            } else {
                // Fallback: ancien format non signe (retrocompatibilite)
                String documentHash = data.getOrDefault("documentHash", "").toString();
                String documentId = data.getOrDefault("documentId", UUID.randomUUID().toString()).toString();
                verificationUrl = verificationBaseUrl + "?id=" + documentId
                        + (documentHash.isEmpty() ? "" : "&hash=" + documentHash);
                log.debug("Using legacy unsigned verification URL for QR code");
            }

            byte[] qrCodeBytes = generateQrCode(verificationUrl);
            com.lowagie.text.Image qrImage = com.lowagie.text.Image.getInstance(qrCodeBytes);
            qrImage.scaleToFit(QR_CODE_SIZE, QR_CODE_SIZE);

            PdfReader reader = new PdfReader(pdfBytes);
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            PdfStamper stamper = new PdfStamper(reader, out);

            int lastPage = reader.getNumberOfPages();
            PdfContentByte over = stamper.getOverContent(lastPage);

            // Place QR code at bottom-right of last page
            Rectangle pageSize = reader.getPageSize(lastPage);
            float qrX = pageSize.getWidth() - QR_CODE_SIZE - 40;
            float qrY = 30;
            qrImage.setAbsolutePosition(qrX, qrY);
            over.addImage(qrImage);

            stamper.close();
            reader.close();

            log.debug("QR code added to PDF via verification URL: {}", verificationUrl);
            return out.toByteArray();

        } catch (Exception e) {
            log.warn("Failed to add QR code to PDF: {}", e.getMessage());
            return pdfBytes;
        }
    }

    /**
     * Generate a QR code as PNG bytes using ZXing.
     */
    private byte[] generateQrCode(String content) throws WriterException, java.io.IOException {
        QRCodeWriter qrCodeWriter = new QRCodeWriter();
        Map<EncodeHintType, Object> hints = Map.of(
                EncodeHintType.CHARACTER_SET, "UTF-8",
                EncodeHintType.MARGIN, 1
        );
        BitMatrix bitMatrix = qrCodeWriter.encode(content, BarcodeFormat.QR_CODE, QR_CODE_SIZE, QR_CODE_SIZE, hints);

        BufferedImage image = new BufferedImage(QR_CODE_SIZE, QR_CODE_SIZE, BufferedImage.TYPE_INT_RGB);
        for (int x = 0; x < QR_CODE_SIZE; x++) {
            for (int y = 0; y < QR_CODE_SIZE; y++) {
                image.setRGB(x, y, bitMatrix.get(x, y) ? 0xFF000000 : 0xFFFFFFFF);
            }
        }

        ByteArrayOutputStream pngOut = new ByteArrayOutputStream();
        ImageIO.write(image, "PNG", pngOut);
        return pngOut.toByteArray();
    }

    /**
     * Maps documentType values to Handlebars template names.
     * PrintJob stores types like "NAISSANCE", "MARIAGE" but templates are named
     * "ACTE_NAISSANCE", "ACTE_MARIAGE", etc.
     */
    private String resolveTemplateName(String documentType) {
        if (documentType == null) return "ACTE_NAISSANCE";
        return switch (documentType) {
            case "NAISSANCE", "ACTE_NAISSANCE" -> "ACTE_NAISSANCE";
            case "MARIAGE", "ACTE_MARIAGE" -> "ACTE_MARIAGE";
            case "DECES", "ACTE_DECES" -> "ACTE_DECES";
            case "ACTES_DIVERS", "ACTE_DIVERS", "DIVERS" -> "ACTE_DIVERS";
            case "PERMIS_ARMES", "PERMIS_PORT_ARMES" -> "PERMIS_PORT_ARMES";
            default -> documentType;
        };
    }

    private String getDefaultTemplate() {
        return """
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"
              "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
            <html xmlns="http://www.w3.org/1999/xhtml">
            <head>
                <style type="text/css">
                    body { font-family: Helvetica, Arial, sans-serif; font-size: 12px; padding: 40px; }
                    h1 { text-align: center; font-size: 18px; margin-bottom: 10px; }
                    h2 { text-align: center; font-size: 14px; font-style: italic; margin-bottom: 20px; }
                    .ref { text-align: right; font-size: 10px; margin-bottom: 20px; }
                    .content { margin-top: 30px; line-height: 1.6; }
                    .footer { margin-top: 40px; text-align: center; font-size: 8px; color: #666; border-top: 1px solid #ccc; padding-top: 10px; }
                    .border { border: 2px solid #006633; padding: 20px; }
                </style>
            </head>
            <body>
            <div class="border">
                <h1>BURKINA FASO</h1>
                <h2>Unite - Progres - Justice</h2>
                <h1>${documentType!'DOCUMENT OFFICIEL'}</h1>
                <div class="ref">Ref: ${documentReference!'N/A'}</div>
                <hr/>
                <div class="content">
                    <p>Ce document est genere automatiquement par la Plateforme Actes.</p>
                    <p>Date d'emission: ${printDate!'N/A'}</p>
                </div>
                <div class="footer">
                    <p>Document officiel - Plateforme Actes - Burkina Faso</p>
                    <p>ID: ${documentId!'N/A'}</p>
                </div>
            </div>
            </body>
            </html>
            """;
    }
}
