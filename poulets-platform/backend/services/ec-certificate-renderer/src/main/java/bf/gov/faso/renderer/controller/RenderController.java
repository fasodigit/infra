package bf.gov.faso.renderer.controller;

import bf.gov.faso.renderer.service.PdfRenderService;
import bf.gov.faso.renderer.service.PdfRenderService.BatchRenderRequest;
import bf.gov.faso.renderer.service.PlaywrightMultiBrowserPool;
import bf.gov.faso.renderer.service.TemplateService;
import bf.gov.faso.renderer.util.RenderSemaphore;
import bf.gov.faso.renderer.util.ZipBuilder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import reactor.core.publisher.Mono;

import java.time.Instant;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/render")
public class RenderController {

    private static final Logger log = LoggerFactory.getLogger(RenderController.class);

    private static final MediaType APPLICATION_ZIP =
            MediaType.parseMediaType("application/zip");

    private final PdfRenderService      pdfRenderService;
    private final TemplateService       templateService;
    private final PlaywrightMultiBrowserPool browserPool;
    private final RenderSemaphore       semaphore;

    public RenderController(
            PdfRenderService pdfRenderService,
            TemplateService templateService,
            PlaywrightMultiBrowserPool browserPool,
            RenderSemaphore semaphore) {
        this.pdfRenderService = pdfRenderService;
        this.templateService  = templateService;
        this.browserPool      = browserPool;
        this.semaphore        = semaphore;
    }

    @PostMapping(
            value = "/{templateName}",
            consumes = MediaType.APPLICATION_JSON_VALUE,
            produces = MediaType.APPLICATION_PDF_VALUE
    )
    public Mono<ResponseEntity<byte[]>> render(
            @PathVariable String templateName,
            @RequestBody Map<String, Object> data) {

        String requestId = generateRequestId();
        log.info("[{}] Rendu unitaire demandé — template='{}'", requestId, templateName);

        ResponseEntity<byte[]> earlyError = validateRequest(templateName, requestId);
        if (earlyError != null) return Mono.just(earlyError);

        return pdfRenderService.renderPdf(templateName, data)
                .map(pdfBytes -> {
                    log.info("[{}] PDF {} rendu ({} octets)", requestId, templateName, pdfBytes.length);
                    return ResponseEntity.ok()
                            .headers(pdfHeaders(templateName, requestId, pdfBytes.length))
                            .<byte[]>body(pdfBytes);
                })
                .onErrorResume(ex -> {
                    log.error("[{}] Échec rendu PDF : {}", requestId, ex.getMessage(), ex);
                    return Mono.just(ResponseEntity
                            .status(HttpStatus.INTERNAL_SERVER_ERROR)
                            .contentType(MediaType.APPLICATION_JSON)
                            .body(errorBody("PDF rendering failed: " + ex.getMessage())));
                })
                .doFinally(signal -> semaphore.release());
    }

    @PostMapping(
            value = "/batch",
            consumes = MediaType.APPLICATION_JSON_VALUE,
            produces = "application/zip"
    )
    public Mono<ResponseEntity<byte[]>> renderBatch(
            @RequestBody List<BatchRenderRequest> requests) {

        String batchId = generateRequestId();

        if (requests == null || requests.isEmpty()) {
            return Mono.just(ResponseEntity
                    .badRequest()
                    .body(errorBody("La liste de requêtes est vide")));
        }

        if (requests.size() > 50) {
            return Mono.just(ResponseEntity
                    .status(HttpStatus.PAYLOAD_TOO_LARGE)
                    .body(errorBody("Maximum 50 documents par batch (reçu: " + requests.size() + ")")));
        }

        if (!browserPool.isHealthy()) {
            return Mono.just(ResponseEntity
                    .status(HttpStatus.SERVICE_UNAVAILABLE)
                    .body(errorBody("Renderer Playwright non disponible")));
        }

        log.info("[batch:{}] {} documents demandés", batchId, requests.size());

        return pdfRenderService.renderBatch(requests)
                .collectList()
                .map(results -> {
                    try {
                        byte[] zip = ZipBuilder.build(results);
                        long successes = results.stream()
                                .filter(PdfRenderService.BatchRenderResult::success)
                                .count();

                        log.info("[batch:{}] ZIP généré — {}/{} succès, {} octets",
                                batchId, successes, results.size(), zip.length);

                        HttpHeaders headers = new HttpHeaders();
                        headers.setContentType(APPLICATION_ZIP);
                        headers.setContentLength(zip.length);
                        headers.set(HttpHeaders.CONTENT_DISPOSITION,
                                "attachment; filename=\"batch-" + batchId + ".zip\"");
                        headers.setCacheControl("no-store");
                        headers.set("X-Batch-Id", batchId);
                        headers.set("X-Batch-Count", String.valueOf(results.size()));
                        headers.set("X-Batch-Success", String.valueOf(successes));

                        return ResponseEntity.ok()
                                .headers(headers)
                                .<byte[]>body(zip);

                    } catch (Exception e) {
                        log.error("[batch:{}] Erreur construction ZIP : {}", batchId, e.getMessage(), e);
                        return ResponseEntity
                                .status(HttpStatus.INTERNAL_SERVER_ERROR)
                                .<byte[]>body(errorBody("Erreur construction ZIP: " + e.getMessage()));
                    }
                })
                .onErrorResume(ex -> {
                    log.error("[batch:{}] Erreur batch : {}", batchId, ex.getMessage(), ex);
                    return Mono.just(ResponseEntity
                            .status(HttpStatus.INTERNAL_SERVER_ERROR)
                            .<byte[]>body(errorBody("Batch rendering failed: " + ex.getMessage())));
                });
    }

    private ResponseEntity<byte[]> validateRequest(String templateName, String requestId) {
        if (!templateService.hasTemplate(templateName)) {
            log.warn("[{}] Template '{}' introuvable. Disponibles: {}",
                    requestId, templateName, templateService.availableTemplates());
            return ResponseEntity
                    .status(HttpStatus.NOT_FOUND)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(errorBody("Template '%s' not found. Available: %s"
                            .formatted(templateName, templateService.availableTemplates())));
        }

        if (!browserPool.isHealthy()) {
            log.error("[{}] Pool Playwright non disponible", requestId);
            return ResponseEntity
                    .status(HttpStatus.SERVICE_UNAVAILABLE)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(errorBody("PDF renderer not available"));
        }

        if (!semaphore.tryAcquire()) {
            log.warn("[{}] Sémaphore saturé ({}/{} actifs)",
                    requestId, semaphore.getActiveTasks(), semaphore.getMaxConcurrent());
            return ResponseEntity
                    .status(HttpStatus.SERVICE_UNAVAILABLE)
                    .contentType(MediaType.APPLICATION_JSON)
                    .header("Retry-After", "2")
                    .body(errorBody("Too many concurrent renders (%d/%d) — retry in 2s"
                            .formatted(semaphore.getActiveTasks(), semaphore.getMaxConcurrent())));
        }

        return null;
    }

    private static HttpHeaders pdfHeaders(String templateName, String requestId, long contentLength) {
        HttpHeaders headers = new HttpHeaders();
        headers.setContentType(MediaType.APPLICATION_PDF);
        headers.setContentLength(contentLength);
        headers.set(HttpHeaders.CONTENT_DISPOSITION,
                "inline; filename=\"%s-%s.pdf\"".formatted(templateName, requestId));
        headers.setCacheControl("no-store");
        headers.set("X-Request-Id", requestId);
        return headers;
    }

    private static String generateRequestId() {
        return Instant.now().toEpochMilli() + "-"
                + UUID.randomUUID().toString().substring(0, 8);
    }

    private static byte[] errorBody(String message) {
        return ("{\"error\":\"%s\"}".formatted(message.replace("\"", "'")))
                .getBytes(java.nio.charset.StandardCharsets.UTF_8);
    }
}
