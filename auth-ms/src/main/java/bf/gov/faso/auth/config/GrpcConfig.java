package bf.gov.faso.auth.config;

import io.grpc.ServerInterceptor;
import io.grpc.Metadata;
import io.grpc.ServerCall;
import io.grpc.ServerCallHandler;
import io.grpc.ForwardingServerCallListener.SimpleForwardingServerCallListener;
import net.devh.boot.grpc.server.interceptor.GrpcGlobalServerInterceptor;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.context.annotation.Configuration;

/**
 * gRPC server configuration for Est-Ouest internal communications.
 * Server runs on port 9801 (configured in application.yml).
 */
@Configuration
public class GrpcConfig {

    private static final Logger log = LoggerFactory.getLogger(GrpcConfig.class);

    /**
     * Global logging interceptor for all gRPC calls.
     * Logs method name and duration for observability.
     */
    @GrpcGlobalServerInterceptor
    public ServerInterceptor loggingInterceptor() {
        return new ServerInterceptor() {
            @Override
            public <ReqT, RespT> ServerCall.Listener<ReqT> interceptCall(
                    ServerCall<ReqT, RespT> call,
                    Metadata headers,
                    ServerCallHandler<ReqT, RespT> next) {

                String methodName = call.getMethodDescriptor().getFullMethodName();
                long startTime = System.nanoTime();
                log.debug("gRPC call started: {}", methodName);

                ServerCall.Listener<ReqT> listener = next.startCall(call, headers);
                return new SimpleForwardingServerCallListener<>(listener) {
                    @Override
                    public void onComplete() {
                        long durationMs = (System.nanoTime() - startTime) / 1_000_000;
                        log.debug("gRPC call completed: {} ({}ms)", methodName, durationMs);
                        super.onComplete();
                    }
                };
            }
        };
    }
}
