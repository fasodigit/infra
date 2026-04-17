package bf.gov.faso.auth.grpc;

import io.grpc.Context;
import io.grpc.Contexts;
import io.grpc.Metadata;
import io.grpc.ServerCall;
import io.grpc.ServerCallHandler;
import io.grpc.ServerInterceptor;
import io.grpc.Status;
import net.devh.boot.grpc.server.interceptor.GrpcGlobalServerInterceptor;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.util.Set;

/**
 * gRPC interceptor that validates caller identity via a shared service token
 * (Bearer) in the Authorization metadata key. In production, replace with mTLS
 * CN whitelist validation once certificates are provisioned.
 */
@Component
@GrpcGlobalServerInterceptor
public class GrpcAuthInterceptor implements ServerInterceptor {

    private static final Logger log = LoggerFactory.getLogger(GrpcAuthInterceptor.class);

    private static final Metadata.Key<String> AUTHORIZATION_KEY =
            Metadata.Key.of("authorization", Metadata.ASCII_STRING_MARSHALLER);

    /** Allowed service tokens — injected from env GRPC_SERVICE_TOKEN (comma-separated). */
    private final Set<String> allowedTokens;

    public GrpcAuthInterceptor(
            @Value("${GRPC_SERVICE_TOKEN}") String serviceTokens) {
        this.allowedTokens = Set.of(serviceTokens.split(","));
    }

    @Override
    public <ReqT, RespT> ServerCall.Listener<ReqT> interceptCall(
            ServerCall<ReqT, RespT> call,
            Metadata headers,
            ServerCallHandler<ReqT, RespT> next) {

        String authHeader = headers.get(AUTHORIZATION_KEY);
        if (authHeader == null || !authHeader.startsWith("Bearer ")) {
            log.warn("gRPC call rejected — missing Authorization header: {}",
                    call.getMethodDescriptor().getFullMethodName());
            call.close(Status.UNAUTHENTICATED.withDescription("Missing Bearer token"), new Metadata());
            return new ServerCall.Listener<>() {};
        }

        String token = authHeader.substring("Bearer ".length()).trim();
        if (!allowedTokens.contains(token)) {
            log.warn("gRPC call rejected — invalid service token for: {}",
                    call.getMethodDescriptor().getFullMethodName());
            call.close(Status.PERMISSION_DENIED.withDescription("Invalid service token"), new Metadata());
            return new ServerCall.Listener<>() {};
        }

        return Contexts.interceptCall(Context.current(), call, headers, next);
    }
}
