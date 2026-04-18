package bf.gov.faso.impression.config;

import org.springframework.context.annotation.Configuration;

/**
 * gRPC client configuration for inter-service communication.
 *
 * KEPT clients (using @GrpcClient annotation or @Value-based config):
 * - VerificationGrpcClient (ec-verification-ms)
 * - DocumentSecurityGrpcClient (document-security-ms)
 *
 * Removed channels (replaced by DragonflyDB Streams + cache):
 * - demandeServiceChannel (DemandeDataGrpcClient, DemandeWorkflowGrpcClient)
 */
@Configuration
public class GrpcClientConfig {
    // All channel beans previously defined here have been removed.
    // KEPT gRPC clients use @GrpcClient annotation or @Value-based config
    // and do not depend on ManagedChannel beans defined here.
}
