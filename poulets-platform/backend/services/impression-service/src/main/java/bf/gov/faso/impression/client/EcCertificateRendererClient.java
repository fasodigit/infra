package bf.gov.faso.impression.client;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.http.MediaType;
import org.springframework.stereotype.Component;
import org.springframework.web.client.RestClient;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import java.util.Map;

@Component
public class EcCertificateRendererClient {

    private static final Logger log = LoggerFactory.getLogger(EcCertificateRendererClient.class);
    private static final String HMAC_ALGORITHM = "HmacSHA256";

    private final RestClient restClient;
    private final String authSecret;

    public EcCertificateRendererClient(
            @Value("${ec-certificate-renderer.url:http://localhost:8800}") String baseUrl,
            @Value("${ec-certificate-renderer.auth-secret:dev-renderer-secret}") String authSecret) {
        this.authSecret = authSecret;
        this.restClient = RestClient.builder()
                .baseUrl(baseUrl)
                .defaultHeader("Content-Type", MediaType.APPLICATION_JSON_VALUE)
                .build();
        log.info("EcCertificateRendererClient initialized — baseUrl={}", baseUrl);
    }

    public byte[] render(String templateName, Map<String, Object> data) {
        log.debug("Rendering certificate template '{}' with {} data fields", templateName, data.size());

        String authHeader = generateAuthHeader();

        try {
            byte[] pdfBytes = restClient.post()
                    .uri("/render/{templateName}", templateName)
                    .header("X-Internal-Auth", authHeader)
                    .body(data)
                    .retrieve()
                    .body(byte[].class);

            if (pdfBytes == null || pdfBytes.length == 0) {
                throw new CertificateRenderException(
                        "Empty PDF response from ec-certificate-renderer for template: " + templateName);
            }

            log.info("Certificate rendered: template={}, size={} bytes", templateName, pdfBytes.length);
            return pdfBytes;

        } catch (CertificateRenderException e) {
            throw e;
        } catch (Exception e) {
            log.error("Failed to render certificate template '{}': {}", templateName, e.getMessage());
            throw new CertificateRenderException(
                    "Certificate rendering failed for template " + templateName + ": " + e.getMessage(), e);
        }
    }

    private String generateAuthHeader() {
        try {
            String timestamp = String.valueOf(System.currentTimeMillis());
            Mac mac = Mac.getInstance(HMAC_ALGORITHM);
            mac.init(new SecretKeySpec(authSecret.getBytes(StandardCharsets.UTF_8), HMAC_ALGORITHM));
            byte[] hmacBytes = mac.doFinal(timestamp.getBytes(StandardCharsets.UTF_8));
            String hmacHex = HexFormat.of().formatHex(hmacBytes);
            return timestamp + ":" + hmacHex;
        } catch (Exception e) {
            throw new IllegalStateException("Failed to generate HMAC auth header", e);
        }
    }

    public boolean isHealthy() {
        try {
            String response = restClient.get()
                    .uri("/health")
                    .retrieve()
                    .body(String.class);
            return response != null && response.contains("UP");
        } catch (Exception e) {
            log.warn("EC certificate renderer health check failed: {}", e.getMessage());
            return false;
        }
    }

    public static class CertificateRenderException extends RuntimeException {
        public CertificateRenderException(String message) {
            super(message);
        }

        public CertificateRenderException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
