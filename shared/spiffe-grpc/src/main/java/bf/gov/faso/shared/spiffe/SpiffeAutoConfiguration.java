// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.shared.spiffe;

import org.springframework.boot.autoconfigure.AutoConfiguration;
import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;

/**
 * Spring Boot auto-configuration for the SPIFFE/SPIRE mTLS integration.
 *
 * <p>Activated when {@code spiffe.enabled=true} in {@code application.yml}.
 * When {@code spiffe.enabled=false} (the default) none of these beans are
 * created and the service falls back to the legacy bearer-token interceptor.
 *
 * <p>Wire-up in the consuming service:
 * <pre>{@code
 * // application.yml:
 * spiffe:
 *   enabled: true
 *   endpoint-socket: unix:/run/spire/sockets/agent.sock
 *   trust-domain: faso.gov.bf
 *   authorized-peers:
 *     - spiffe://faso.gov.bf/ns/default/sa/armageddon
 * }</pre>
 *
 * <p>The consuming service must:
 * <ol>
 *   <li>Remove or disable the legacy {@code GrpcAuthInterceptor} when
 *       {@code spiffe.enabled=true}.
 *   <li>Configure grpc-spring-boot to use the SVID from
 *       {@link SpiffeX509SvidManager} for TLS:
 *       set {@code grpc.server.security.certificateChain} and
 *       {@code grpc.server.security.privateKey} to point to the SVID
 *       files written by SPIRE (or integrate via a custom
 *       {@code io.netty.handler.ssl.SslContext} factory).
 * </ol>
 */
@AutoConfiguration
@EnableConfigurationProperties(SpiffeProperties.class)
@ConditionalOnProperty(prefix = "spiffe", name = "enabled", havingValue = "true")
public class SpiffeAutoConfiguration {

    /**
     * Creates the SVID manager bean.  Fails fast (throws) if the SPIRE agent
     * socket is unreachable — this is intentional: a service that cannot
     * obtain an SVID should not start.
     */
    @Bean
    public SpiffeX509SvidManager spiffeX509SvidManager(SpiffeProperties props) throws Exception {
        return new SpiffeX509SvidManager(props);
    }

    /**
     * Registers the server interceptor that validates the mTLS peer
     * certificate SPIFFE ID on every incoming gRPC call.
     */
    @Bean
    public SpiffeGrpcServerInterceptor spiffeGrpcServerInterceptor(SpiffeProperties props) {
        return new SpiffeGrpcServerInterceptor(
                props.getAuthorizedPeers(),
                props.getTrustDomain());
    }
}
