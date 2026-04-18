package bf.gov.faso.renderer.grpc;

import bf.gov.actes.grpc.renderer.EcRendererServiceGrpc;
import bf.gov.actes.grpc.renderer.RenderPdfRequest;
import bf.gov.actes.grpc.renderer.RenderPdfResponse;
import bf.gov.faso.renderer.service.PdfRenderService;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.HashMap;
import java.util.Map;

@GrpcService
public class RendererGrpcService extends EcRendererServiceGrpc.EcRendererServiceImplBase {

    private static final Logger log = LoggerFactory.getLogger(RendererGrpcService.class);

    private final PdfRenderService pdfRenderService;

    public RendererGrpcService(PdfRenderService pdfRenderService) {
        this.pdfRenderService = pdfRenderService;
    }

    @Override
    public void renderPdf(RenderPdfRequest request, StreamObserver<RenderPdfResponse> responseObserver) {
        String templateName = request.getTemplateName();
        log.info("gRPC: RenderPdf request for template={}", templateName);

        try {
            // Convert proto map<string,string> to Map<String,Object> for template engine
            Map<String, Object> data = new HashMap<>(request.getDataMap());

            // PdfRenderService.renderPdf returns Mono<byte[]> — block for gRPC
            byte[] pdfBytes = pdfRenderService.renderPdf(templateName, data).block();

            if (pdfBytes == null || pdfBytes.length == 0) {
                responseObserver.onNext(RenderPdfResponse.newBuilder()
                        .setSuccess(false)
                        .setErrorMessage("Empty PDF generated")
                        .build());
            } else {
                responseObserver.onNext(RenderPdfResponse.newBuilder()
                        .setPdfContent(com.google.protobuf.ByteString.copyFrom(pdfBytes))
                        .setSuccess(true)
                        .setSizeBytes(pdfBytes.length)
                        .build());
                log.info("gRPC: PDF rendered successfully, template={}, size={} bytes",
                        templateName, pdfBytes.length);
            }
            responseObserver.onCompleted();

        } catch (Exception e) {
            log.error("gRPC: Failed to render PDF for template={}: {}", templateName, e.getMessage(), e);
            responseObserver.onNext(RenderPdfResponse.newBuilder()
                    .setSuccess(false)
                    .setErrorMessage(e.getMessage() != null ? e.getMessage() : "Unknown error")
                    .build());
            responseObserver.onCompleted();
        }
    }
}
