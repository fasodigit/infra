package bf.gov.faso.impression.service;

import com.google.zxing.BarcodeFormat;
import com.google.zxing.WriterException;
import com.google.zxing.client.j2se.MatrixToImageWriter;
import com.google.zxing.common.BitMatrix;
import com.google.zxing.qrcode.QRCodeWriter;
import org.apache.pdfbox.pdmodel.PDDocument;
import org.apache.pdfbox.pdmodel.PDPage;
import org.apache.pdfbox.pdmodel.PDPageContentStream;
import org.apache.pdfbox.pdmodel.common.PDRectangle;
import org.apache.pdfbox.pdmodel.font.PDType1Font;
import org.apache.pdfbox.pdmodel.font.Standard14Fonts;
import org.apache.pdfbox.pdmodel.graphics.image.PDImageXObject;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.LocalDate;
import java.time.format.DateTimeFormatter;
import java.util.UUID;

@Service
public class PdfGeneratorService {

    private static final Logger log = LoggerFactory.getLogger(PdfGeneratorService.class);

    @Value("${impression.storage.base-path:/home/DOCS-PDF-ETAT-CIVIL}")
    private String basePath;

    @Value("${impression.verification.base-url:https://etat-civil.gov.bf/verify}")
    private String verificationBaseUrl;

    /**
     * Génère un PDF/A avec QR code de vérification.
     */
    public GeneratedPdfResult generatePdfWithQrCode(
            String numeroActe,
            String typeDocument,
            String nomBeneficiaire,
            String prenomBeneficiaire,
            String dateNaissance,
            String lieuNaissance,
            String tenantId,
            UUID demandeId
    ) throws IOException {

        String verificationCode = generateVerificationCode();
        String qrUrl = verificationBaseUrl + "/" + verificationCode;

        // Create PDF document
        try (PDDocument document = new PDDocument()) {
            PDPage page = new PDPage(PDRectangle.A4);
            document.addPage(page);

            try (PDPageContentStream contentStream = new PDPageContentStream(document, page)) {
                // Header
                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA_BOLD), 16);
                contentStream.newLineAtOffset(50, 780);
                contentStream.showText("BURKINA FASO");
                contentStream.endText();

                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA), 12);
                contentStream.newLineAtOffset(50, 760);
                contentStream.showText("Ministère de l'Administration Territoriale");
                contentStream.endText();

                // Document title
                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA_BOLD), 18);
                contentStream.newLineAtOffset(150, 700);
                contentStream.showText(getDocumentTitle(typeDocument));
                contentStream.endText();

                // Document number
                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA), 12);
                contentStream.newLineAtOffset(50, 660);
                contentStream.showText("N° " + numeroActe);
                contentStream.endText();

                // Beneficiary info
                int yPos = 600;
                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA), 12);
                contentStream.newLineAtOffset(50, yPos);
                contentStream.showText("Nom: " + nomBeneficiaire);
                contentStream.endText();

                yPos -= 25;
                contentStream.beginText();
                contentStream.newLineAtOffset(50, yPos);
                contentStream.showText("Prénom(s): " + prenomBeneficiaire);
                contentStream.endText();

                if (dateNaissance != null) {
                    yPos -= 25;
                    contentStream.beginText();
                    contentStream.newLineAtOffset(50, yPos);
                    contentStream.showText("Date de naissance: " + dateNaissance);
                    contentStream.endText();
                }

                if (lieuNaissance != null) {
                    yPos -= 25;
                    contentStream.beginText();
                    contentStream.newLineAtOffset(50, yPos);
                    contentStream.showText("Lieu de naissance: " + lieuNaissance);
                    contentStream.endText();
                }

                // Add QR Code
                byte[] qrCodeImage = generateQrCode(qrUrl, 150, 150);
                PDImageXObject qrImage = PDImageXObject.createFromByteArray(document, qrCodeImage, "qrcode");
                contentStream.drawImage(qrImage, 400, 100, 150, 150);

                // QR code label
                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA), 8);
                contentStream.newLineAtOffset(400, 85);
                contentStream.showText("Scanner pour vérifier");
                contentStream.endText();

                // Footer with verification code
                contentStream.beginText();
                contentStream.setFont(new PDType1Font(Standard14Fonts.FontName.HELVETICA), 10);
                contentStream.newLineAtOffset(50, 50);
                contentStream.showText("Code de vérification: " + verificationCode);
                contentStream.endText();
            }

            // Save to file
            Path filePath = getStoragePath(tenantId, numeroActe);
            Files.createDirectories(filePath.getParent());
            document.save(filePath.toFile());

            log.info("PDF generated: {}", filePath);

            return new GeneratedPdfResult(
                filePath.toString(),
                verificationCode,
                qrUrl,
                Files.size(filePath)
            );
        }
    }

    private byte[] generateQrCode(String content, int width, int height) throws IOException {
        try {
            QRCodeWriter qrCodeWriter = new QRCodeWriter();
            BitMatrix bitMatrix = qrCodeWriter.encode(content, BarcodeFormat.QR_CODE, width, height);
            BufferedImage image = MatrixToImageWriter.toBufferedImage(bitMatrix);

            ByteArrayOutputStream baos = new ByteArrayOutputStream();
            ImageIO.write(image, "PNG", baos);
            return baos.toByteArray();
        } catch (WriterException e) {
            throw new IOException("Failed to generate QR code", e);
        }
    }

    private Path getStoragePath(String tenantId, String numeroActe) {
        LocalDate now = LocalDate.now();
        String year = String.valueOf(now.getYear());
        String month = String.format("%02d", now.getMonthValue());
        String filename = numeroActe.replace("/", "-") + ".pdf";

        return Paths.get(basePath, tenantId, year, month, filename);
    }

    private String generateVerificationCode() {
        return UUID.randomUUID().toString().substring(0, 8).toUpperCase();
    }

    private String getDocumentTitle(String typeDocument) {
        return switch (typeDocument) {
            case "ACTE_NAISSANCE" -> "EXTRAIT D'ACTE DE NAISSANCE";
            case "ACTE_MARIAGE" -> "EXTRAIT D'ACTE DE MARIAGE";
            case "ACTE_DECES" -> "EXTRAIT D'ACTE DE DÉCÈS";
            default -> "ACTE D'ÉTAT CIVIL";
        };
    }

    public record GeneratedPdfResult(
        String filePath,
        String verificationCode,
        String qrUrl,
        long fileSize
    ) {}
}
