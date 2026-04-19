// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.shared.spiffe;

import io.spiffe.workloadapi.WorkloadApiClient;
import io.spiffe.workloadapi.DefaultWorkloadApiClient;
import io.spiffe.workloadapi.Watcher;
import io.spiffe.workloadapi.X509Source;
import io.spiffe.svid.x509.X509Svid;
import io.spiffe.bundle.x509bundle.X509Bundle;
import jakarta.annotation.PreDestroy;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.io.Closeable;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Manages the X.509 SVID lifecycle for FASO Java microservices.
 *
 * <p>Lifecycle:
 * <ol>
 *   <li>Connects to the SPIRE workload-API socket (default:
 *       {@code unix:/run/spire/sockets/agent.sock}).
 *   <li>Fetches the initial X.509-SVID synchronously; throws if unavailable
 *       so the Spring context fails fast rather than starting with null
 *       certificates.
 *   <li>Subscribes to the rotation stream; on every SVID renewal the
 *       {@link #currentSvid} reference is atomically swapped — existing
 *       gRPC sessions drain with the old cert; new handshakes pick up the
 *       new one automatically.
 * </ol>
 *
 * <p>Failure modes:
 * <ul>
 *   <li>Socket unreachable at startup → {@link IllegalStateException}; the
 *       process should restart (K8s restart-policy or systemd).
 *   <li>Rotation stream severed → the previous SVID stays active; the
 *       {@link io.spiffe.workloadapi.X509Source} reconnects automatically
 *       using the spiffe-java library's built-in retry.
 *   <li>SVID expires while disconnected → existing sessions drain; new
 *       handshakes fail with a TLS error until SPIRE delivers a fresh SVID.
 * </ul>
 *
 * <p>When {@code spiffe.enabled=false} this bean is not created
 * (see {@link SpiffeAutoConfiguration}).
 */
@Component
public class SpiffeX509SvidManager implements Closeable {

    private static final Logger log = LoggerFactory.getLogger(SpiffeX509SvidManager.class);

    private final X509Source x509Source;
    private final AtomicReference<X509Svid> currentSvid = new AtomicReference<>();

    public SpiffeX509SvidManager(SpiffeProperties props) throws Exception {
        log.info("Initialising SPIFFE X509Source from socket: {}", props.getEndpointSocket());

        // The spiffe-java X509Source reads from the Workload API and keeps
        // the SVID up-to-date via an internal streaming background thread.
        X509Source.X509SourceOptions options = X509Source.X509SourceOptions.builder()
                .spiffeSocketPath(props.getEndpointSocket())
                .build();
        this.x509Source = X509Source.newSource(options);

        // Fetch the initial SVID synchronously — fail fast if unavailable.
        X509Svid svid = x509Source.getX509Svid();
        if (svid == null) {
            throw new IllegalStateException(
                    "SPIRE workload API returned null SVID — is the agent running at "
                    + props.getEndpointSocket() + "?");
        }
        currentSvid.set(svid);
        log.info("Initial SVID obtained: {}", svid.getSpiffeId());

        // Register a watcher so we get notified on rotation.
        x509Source.watch(new Watcher<X509Svid>() {
            @Override
            public void onUpdate(X509Svid updated) {
                X509Svid previous = currentSvid.getAndSet(updated);
                log.info("SVID rotated: {} → {}",
                        previous != null ? previous.getSpiffeId() : "<none>",
                        updated.getSpiffeId());
            }

            @Override
            public void onError(Throwable cause) {
                log.error("SVID rotation stream error (previous SVID still active): {}",
                        cause.getMessage(), cause);
            }
        });
    }

    /**
     * Returns the current (possibly just-rotated) SVID.  Always non-null
     * after successful construction.
     */
    public X509Svid currentSvid() {
        return currentSvid.get();
    }

    /**
     * Returns the X509Bundle for this workload's trust domain.
     * Used by mTLS acceptors to validate peer certificate chains.
     */
    public X509Bundle trustBundle(String trustDomain) {
        return x509Source.getBundleForTrustDomain(
                io.spiffe.spiffeid.TrustDomain.parse(trustDomain));
    }

    @PreDestroy
    @Override
    public void close() {
        try {
            x509Source.close();
            log.info("SpiffeX509SvidManager closed");
        } catch (Exception e) {
            log.warn("Error closing X509Source: {}", e.getMessage());
        }
    }
}
