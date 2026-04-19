// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.shared.spiffe;

import org.springframework.boot.context.properties.ConfigurationProperties;

import java.util.ArrayList;
import java.util.List;

/**
 * Configuration properties for the SPIFFE/SPIRE workload-identity integration.
 *
 * <p>Bound from {@code application.yml} under the {@code spiffe} prefix:
 *
 * <pre>{@code
 * spiffe:
 *   enabled: true
 *   endpoint-socket: unix:/run/spire/sockets/agent.sock
 *   trust-domain: faso.gov.bf
 *   authorized-peers:
 *     - spiffe://faso.gov.bf/ns/default/sa/armageddon
 *     - spiffe://faso.gov.bf/ns/default/sa/kaya
 * }</pre>
 *
 * <p>When {@code enabled = false} (the default) the SPIFFE interceptor is a
 * no-op and callers must set {@code spiffe.bearer-token-env} to name the env
 * var carrying the shared service token used for legacy bearer auth.
 */
@ConfigurationProperties(prefix = "spiffe")
public class SpiffeProperties {

    /**
     * Enable SPIFFE mTLS peer validation.
     * Default: {@code false} — legacy bearer-token mode is active.
     */
    private boolean enabled = false;

    /**
     * SPIRE workload-API socket URI consumed by the Java workload-API client.
     * Matches the {@code SPIFFE_ENDPOINT_SOCKET} env-var convention.
     * Default: {@code unix:/run/spire/sockets/agent.sock}
     */
    private String endpointSocket = "unix:/run/spire/sockets/agent.sock";

    /**
     * SPIFFE trust domain (without the {@code spiffe://} prefix).
     * Peer cert URI SANs not belonging to this domain are rejected
     * unconditionally before the authorized-peers whitelist is consulted.
     */
    private String trustDomain = "faso.gov.bf";

    /**
     * Exhaustive whitelist of SPIFFE IDs this service accepts connections from.
     * Each entry must be a full SPIFFE URI, e.g.
     * {@code spiffe://faso.gov.bf/ns/default/sa/armageddon}.
     * An empty list is fail-closed: no peer is accepted.
     */
    private List<String> authorizedPeers = new ArrayList<>();

    /**
     * Name of the env var carrying the legacy shared service token.
     * Only consulted when {@code enabled = false}.
     * Default: {@code GRPC_SERVICE_TOKEN}
     */
    private String bearerTokenEnv = "GRPC_SERVICE_TOKEN";

    // -------------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------------

    public boolean isEnabled() { return enabled; }
    public void setEnabled(boolean enabled) { this.enabled = enabled; }

    public String getEndpointSocket() { return endpointSocket; }
    public void setEndpointSocket(String endpointSocket) { this.endpointSocket = endpointSocket; }

    public String getTrustDomain() { return trustDomain; }
    public void setTrustDomain(String trustDomain) { this.trustDomain = trustDomain; }

    public List<String> getAuthorizedPeers() { return authorizedPeers; }
    public void setAuthorizedPeers(List<String> authorizedPeers) { this.authorizedPeers = authorizedPeers; }

    public String getBearerTokenEnv() { return bearerTokenEnv; }
    public void setBearerTokenEnv(String bearerTokenEnv) { this.bearerTokenEnv = bearerTokenEnv; }
}
