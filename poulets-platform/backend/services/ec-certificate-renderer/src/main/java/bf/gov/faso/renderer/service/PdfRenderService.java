package bf.gov.faso.renderer.service;

import bf.gov.faso.renderer.util.RenderSemaphore;
import com.google.zxing.BarcodeFormat;
import com.google.zxing.client.j2se.MatrixToImageWriter;
import com.google.zxing.common.BitMatrix;
import com.google.zxing.qrcode.QRCodeWriter;
import com.microsoft.playwright.Page;
import com.microsoft.playwright.options.Margin;
import com.microsoft.playwright.options.WaitUntilState;
import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.core.instrument.Timer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.stereotype.Service;
import reactor.core.publisher.Flux;
import reactor.core.publisher.Mono;
import reactor.core.scheduler.Scheduler;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

@Service
public class PdfRenderService {

    private static final Logger log = LoggerFactory.getLogger(PdfRenderService.class);

    private static final Page.PdfOptions PDF_OPTS = new Page.PdfOptions()
            .setFormat("A4")
            .setPrintBackground(true)
            .setPreferCSSPageSize(true)
            .setMargin(new Margin()
                    .setTop("8mm").setRight("8mm")
                    .setBottom("10mm").setLeft("8mm"));

    private final TemplateService            templateService;
    private final PlaywrightMultiBrowserPool browserPool;
    private final PdfCacheService            cacheService;
    private final RenderSemaphore            semaphore;
    private final Scheduler                  vtScheduler;
    private final MeterRegistry              meterRegistry;

    public PdfRenderService(
            TemplateService templateService,
            PlaywrightMultiBrowserPool browserPool,
            PdfCacheService cacheService,
            RenderSemaphore semaphore,
            @Qualifier("vtScheduler") Scheduler vtScheduler,
            MeterRegistry meterRegistry) {
        this.templateService = templateService;
        this.browserPool     = browserPool;
        this.cacheService    = cacheService;
        this.semaphore       = semaphore;
        this.vtScheduler     = vtScheduler;
        this.meterRegistry   = meterRegistry;
    }

    public Mono<byte[]> renderPdf(String templateName, Map<String, Object> data) {
        return Mono
                .fromCallable(() -> doRenderWithCache(templateName, data))
                .subscribeOn(vtScheduler)
                .doOnError(e -> meterRegistry.counter("renderer.pdf.failed",
                        "template", templateName).increment());
    }

    public Flux<BatchRenderResult> renderBatch(List<BatchRenderRequest> requests) {
        return Mono.fromCallable(() -> doRenderBatch(requests))
                   .subscribeOn(vtScheduler)
                   .flatMapMany(Flux::fromIterable);
    }

    private byte[] doRenderWithCache(String templateName, Map<String, Object> data)
            throws Exception {

        var cached = cacheService.get(templateName, data);
        if (cached.isPresent()) {
            meterRegistry.counter("renderer.pdf.cache.hit", "template", templateName).increment();
            return cached.get();
        }

        byte[] pdf = doRender(templateName, data);

        cacheService.put(templateName, data, pdf);
        meterRegistry.counter("renderer.pdf.cache.miss", "template", templateName).increment();

        return pdf;
    }

    @SuppressWarnings("unchecked")
    private byte[] doRender(String templateName, Map<String, Object> data) throws Exception {
        long start = System.currentTimeMillis();
        String tid = Thread.currentThread().getName();

        // QR Code
        Object qrRaw = data.get("qrCodeData");
        if (qrRaw != null) {
            String qrDataUrl = generateQrDataUrl(qrRaw.toString(), 150);
            data.put("qrCodeDataUrl", qrDataUrl);
        }

        // Handlebars → HTML
        String html = templateService.render(templateName, data);
        long htmlMs = System.currentTimeMillis() - start;

        // Playwright → PDF
        Page page = browserPool.acquire();
        try {
            long playwrightStart = System.currentTimeMillis();

            page.setContent(html, new Page.SetContentOptions()
                    .setTimeout(15_000)
                    .setWaitUntil(WaitUntilState.DOMCONTENTLOADED));

            // Fonts are inline data:URIs — no need for document.fonts.ready

            byte[] pdfBytes = page.pdf(PDF_OPTS);

            long totalMs = System.currentTimeMillis() - start;
            log.info("[{}] PDF {} généré — html={}ms, chromium={}ms, total={}ms, {} octets",
                    tid, templateName, htmlMs,
                    System.currentTimeMillis() - playwrightStart, totalMs, pdfBytes.length);

            meterRegistry.counter("renderer.pdf.generated", "template", templateName).increment();
            Timer.builder("renderer.pdf.duration")
                 .tag("template", templateName)
                 .register(meterRegistry)
                 .record(totalMs, TimeUnit.MILLISECONDS);

            return pdfBytes;

        } finally {
            browserPool.release(page);
        }
    }

    private List<BatchRenderResult> doRenderBatch(List<BatchRenderRequest> requests)
            throws InterruptedException {

        log.info("Batch rendu : {} documents en parallèle", requests.size());
        long batchStart = System.currentTimeMillis();

        try (var executor = Executors.newVirtualThreadPerTaskExecutor()) {

            List<Future<BatchRenderResult>> futures = new ArrayList<>(requests.size());

            for (int i = 0; i < requests.size(); i++) {
                final BatchRenderRequest req = requests.get(i);
                final int idx = i;

                futures.add(executor.submit(() -> {
                    try {
                        byte[] pdf = doRenderWithCache(req.templateName(), req.data());
                        return BatchRenderResult.success(idx, req.filename(), pdf);
                    } catch (Exception e) {
                        log.error("Batch[{}] erreur rendu {} : {}", idx, req.templateName(), e.getMessage());
                        return BatchRenderResult.failure(idx, req.filename(), e.getMessage());
                    }
                }));
            }

            List<BatchRenderResult> results = new ArrayList<>(requests.size());
            for (Future<BatchRenderResult> future : futures) {
                try {
                    results.add(future.get(30, TimeUnit.SECONDS));
                } catch (ExecutionException e) {
                    results.add(BatchRenderResult.failure(
                            results.size(), "unknown", e.getCause().getMessage()));
                } catch (TimeoutException e) {
                    results.add(BatchRenderResult.failure(
                            results.size(), "unknown", "Timeout après 30s"));
                }
            }

            log.info("Batch terminé : {}/{} succès en {} ms",
                    results.stream().filter(BatchRenderResult::success).count(),
                    results.size(),
                    System.currentTimeMillis() - batchStart);

            return results;
        }
    }

    private static String generateQrDataUrl(String content, int size) throws Exception {
        QRCodeWriter writer = new QRCodeWriter();
        BitMatrix matrix = writer.encode(content, BarcodeFormat.QR_CODE, size, size);
        BufferedImage image = MatrixToImageWriter.toBufferedImage(matrix);

        ByteArrayOutputStream baos = new ByteArrayOutputStream();
        ImageIO.write(image, "PNG", baos);

        return "data:image/png;base64,"
                + Base64.getEncoder().encodeToString(baos.toByteArray());
    }

    public record BatchRenderRequest(
            String templateName,
            Map<String, Object> data,
            String filename
    ) {}

    public record BatchRenderResult(
            int index,
            String filename,
            byte[] pdf,
            boolean success,
            String errorMessage
    ) {
        static BatchRenderResult success(int index, String filename, byte[] pdf) {
            return new BatchRenderResult(index, filename, pdf, true, null);
        }
        static BatchRenderResult failure(int index, String filename, String error) {
            return new BatchRenderResult(index, filename, null, false, error);
        }
    }
}
