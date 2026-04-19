// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.auth.grpc;

import io.grpc.Context;
import io.grpc.Contexts;
import io.grpc.Metadata;
import io.grpc.ServerCall;
import io.grpc.ServerCallHandler;
import io.grpc.ServerInterceptor;
import io.grpc.Status;
import net.devh.boot.grpc.server.interceptor.GrpcGlobalServerInterceptor;
import org.bouncycastle.asn1.ASN1ObjectIdentifier;
import org.bouncycastle.asn1.x509.Extension;
import org.bouncycastle.asn1.x509.GeneralName;
import org.bouncycastle.asn1.x509.GeneralNames;
import org.bouncycastle.cert.X509CertificateHolder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import javax.net.ssl.SSLPeerUnverifiedException;
import javax.net.ssl.SSLSession;
import java.security.cert.Certificate;
import java.security.cert.X509Certificate;
import java.util.List;
import java.util.Set;

/**
 * gRPC server interceptor for East-West (service-to-service) authentication.
 *
 * <h2>Mode selection</h2>
 * <ul>
 *   <li><strong>SPIFFE mode</strong> ({@code spiffe.enabled=true}): extracts
 *       the peer SPIFFE ID from the mTLS certificate URI SAN and validates it
 *       against the {@code spiffe.authorized-peers} whitelist.  No bearer
 *       token is required or accepted.</li>
 *   <li><strong>Legacy mode</strong> ({@code spiffe.enabled=false}, default):
 *       validates the {@code Authorization: Bearer &lt;token&gt;} header
 *       against the {@code GRPC_SERVICE_TOKEN} env var.  This mode is kept
 *       for backward compat during progressive SPIFFE roll-out and will be
 *       removed once all callers present mTLS certificates.</li>
 * </ul>
 *
 * <h2>SPIFFE ID extraction</h2>
 * Uses BouncyCastle to parse the DER-encoded end-entity peer certificate and
 * extract the first URI Subject Alternative Name that starts with
 * {@code spiffe://}.  The OID 2.5.29.17 (subjectAltName) is used.
 *
 * <h2>Trust domain guard</h2>
 * Before the authorized-peers whitelist is consulted, every peer SPIFFE ID
 * is checked against the configured {@code spiffe.trust-domain}.  This
 * ensures that certificates from a foreign SPIFFE CA — even if otherwise
 * valid — are rejected outright.
 *
 * <h2>Failure modes</h2>
 * <ul>
 *   <li>No TLS session → {@code UNAUTHENTICATED} (SPIFFE mode only).</li>
 *   <li>Peer cert has no URI SAN → {@code PERMISSION_DENIED}.</li>
 *   <li>Peer SPIFFE ID not in whitelist → {@code PERMISSION_DENIED}.</li>
 *   <li>Missing/invalid Bearer token (legacy mode) → {@code UNAUTHENTICATED}
 *       / {@code PERMISSION_DENIED} as before.</li>
 * </ul>
 */
@Component
@GrpcGlobalServerInterceptor
public class GrpcAuthInterceptor implements ServerInterceptor {

    private static final Logger log = LoggerFactory.getLogger(GrpcAuthInterceptor.class);

    private static final Metadata.Key<String> AUTHORIZATION_KEY =
            Metadata.Key.of("authorization", Metadata.ASCII_STRING_MARSHALLER);

    /** OID for Subject Alternative Name (RFC 5280 §4.2.1.6). */
    private static final ASN1ObjectIdentifier OID_SAN = Extension.subjectAlternativeName;

    /**
     * gRPC Context key under which the validated peer SPIFFE ID is propagated.
     * Downstream can call {@code PEER_SPIFFE_ID.get()} to read the peer identity.
     */
    public static final Context.Key<String> PEER_SPIFFE_ID =
            Context.key("peer-spiffe-id");

    // ── SPIFFE mode fields ────────────────────────────────────────────────────

    private final boolean spiffeEnabled;
    private final String trustDomainPrefix;   // "spiffe://faso.gov.bf/"
    private final Set<String> authorizedPeers;

    // ── Legacy mode fields ────────────────────────────────────────────────────

    /** Allowed bearer tokens (comma-separated GRPC_SERVICE_TOKEN env var). */
    private final Set<String> allowedTokens;

    // ── Constructor ───────────────────────────────────────────────────────────

    public GrpcAuthInterceptor(
            @Value("${spiffe.enabled:false}") boolean spiffeEnabled,
            @Value("${spiffe.trust-domain:faso.gov.bf}") String trustDomain,
            @Value("${spiffe.authorized-peers:}") List<String> authorizedPeers,
            @Value("${GRPC_SERVICE_TOKEN:}") String serviceTokens) {

        this.spiffeEnabled = spiffeEnabled;
        this.trustDomainPrefix = "spiffe://" + trustDomain + "/";
        this.authorizedPeers = Set.copyOf(authorizedPeers);

        // Legacy bearer tokens — split on comma, ignore empty values.
        this.allowedTokens = serviceTokens.isBlank()
                ? Set.of()
                : Set.of(serviceTokens.split(","));

        if (spiffeEnabled) {
            log.info("GrpcAuthInterceptor: SPIFFE mode active. trust_domain={} authorized_peers={}",
                    trustDomain, authorizedPeers);
        } else {
            log.info("GrpcAuthInterceptor: legacy bearer-token mode active "
                    + "(set spiffe.enabled=true to activate mTLS)");
        }
    }

    // ── Interceptor ───────────────────────────────────────────────────────────

    @Override
    public <ReqT, RespT> ServerCall.Listener<ReqT> interceptCall(
            ServerCall<ReqT, RespT> call,
            Metadata headers,
            ServerCallHandler<ReqT, RespT> next) {

        return spiffeEnabled
                ? interceptSpiffe(call, headers, next)
                : interceptLegacy(call, headers, next);
    }

    // ── SPIFFE path ───────────────────────────────────────────────────────────

    private <ReqT, RespT> ServerCall.Listener<ReqT> interceptSpiffe(
            ServerCall<ReqT, RespT> call,
            Metadata headers,
            ServerCallHandler<ReqT, RespT> next) {

        String method = call.getMethodDescriptor().getFullMethodName();

        SSLSession sslSession = call.getAttributes()
                .get(io.grpc.Grpc.TRANSPORT_ATTR_SSL_SESSION);

        if (sslSession == null) {
            log.warn("SPIFFE: rejected — no TLS session (mTLS required): {}", method);
            call.close(
                    Status.UNAUTHENTICATED.withDescription("mTLS required; no TLS session"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        Certificate[] peerCerts;
        try {
            peerCerts = sslSession.getPeerCertificates();
        } catch (SSLPeerUnverifiedException e) {
            log.warn("SPIFFE: rejected — peer not verified ({}): {}", e.getMessage(), method);
            call.close(
                    Status.UNAUTHENTICATED.withDescription("Peer certificate unavailable"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        if (peerCerts == null || peerCerts.length == 0) {
            log.warn("SPIFFE: rejected — empty peer cert chain: {}", method);
            call.close(
                    Status.UNAUTHENTICATED.withDescription("No peer certificate"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        String spiffeId = extractSpiffeId((X509Certificate) peerCerts[0]);
        if (spiffeId == null) {
            log.warn("SPIFFE: rejected — no URI SAN in peer cert: {}", method);
            call.close(
                    Status.PERMISSION_DENIED.withDescription("Peer cert has no SPIFFE URI SAN"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        if (!spiffeId.startsWith(trustDomainPrefix)) {
            log.warn("SPIFFE: rejected — foreign trust domain. got={} expected_prefix={} method={}",
                    spiffeId, trustDomainPrefix, method);
            call.close(
                    Status.PERMISSION_DENIED.withDescription("Foreign SPIFFE trust domain"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        if (!authorizedPeers.contains(spiffeId)) {
            log.warn("SPIFFE: rejected — peer not in authorized_peers. peer={} method={}",
                    spiffeId, method);
            call.close(
                    Status.PERMISSION_DENIED.withDescription("SPIFFE ID not in authorized_peers"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        log.debug("SPIFFE: authorized. peer={} method={}", spiffeId, method);
        Context ctx = Context.current().withValue(PEER_SPIFFE_ID, spiffeId);
        return Contexts.interceptCall(ctx, call, headers, next);
    }

    // ── Legacy bearer-token path ──────────────────────────────────────────────

    private <ReqT, RespT> ServerCall.Listener<ReqT> interceptLegacy(
            ServerCall<ReqT, RespT> call,
            Metadata headers,
            ServerCallHandler<ReqT, RespT> next) {

        String authHeader = headers.get(AUTHORIZATION_KEY);
        if (authHeader == null || !authHeader.startsWith("Bearer ")) {
            log.warn("Legacy: rejected — missing Bearer token: {}",
                    call.getMethodDescriptor().getFullMethodName());
            call.close(
                    Status.UNAUTHENTICATED.withDescription("Missing Bearer token"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        String token = authHeader.substring("Bearer ".length()).trim();
        if (!allowedTokens.contains(token)) {
            log.warn("Legacy: rejected — invalid service token: {}",
                    call.getMethodDescriptor().getFullMethodName());
            call.close(
                    Status.PERMISSION_DENIED.withDescription("Invalid service token"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        return Contexts.interceptCall(Context.current(), call, headers, next);
    }

    // ── SPIFFE ID extraction (BouncyCastle) ───────────────────────────────────

    /**
     * Extract the first SPIFFE URI SAN from the end-entity certificate.
     * Returns {@code null} if none found or parsing fails.
     */
    static String extractSpiffeId(X509Certificate cert) {
        try {
            X509CertificateHolder holder = new X509CertificateHolder(cert.getEncoded());
            org.bouncycastle.asn1.x509.Extensions exts = holder.getExtensions();
            if (exts == null) return null;

            org.bouncycastle.asn1.x509.Extension sanExt = exts.getExtension(OID_SAN);
            if (sanExt == null) return null;

            GeneralNames gns = GeneralNames.getInstance(sanExt.getParsedValue());
            for (GeneralName gn : gns.getNames()) {
                if (gn.getTagNo() == GeneralName.uniformResourceIdentifier) {
                    String uri = gn.getName().toString();
                    if (uri.startsWith("spiffe://")) {
                        return uri;
                    }
                }
            }
        } catch (Exception e) {
            log.debug("Failed to parse X.509 URI SAN: {}", e.getMessage());
        }
        return null;
    }
}
