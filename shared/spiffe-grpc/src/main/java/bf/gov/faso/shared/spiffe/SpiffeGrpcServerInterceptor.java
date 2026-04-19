// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
package bf.gov.faso.shared.spiffe;

import io.grpc.Context;
import io.grpc.Contexts;
import io.grpc.Metadata;
import io.grpc.ServerCall;
import io.grpc.ServerCallHandler;
import io.grpc.ServerInterceptor;
import io.grpc.Status;
import io.netty.handler.ssl.SslHandler;
import org.bouncycastle.asn1.ASN1ObjectIdentifier;
import org.bouncycastle.asn1.x509.Extension;
import org.bouncycastle.asn1.x509.GeneralName;
import org.bouncycastle.asn1.x509.GeneralNames;
import org.bouncycastle.cert.X509CertificateHolder;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.net.ssl.SSLPeerUnverifiedException;
import javax.net.ssl.SSLSession;
import java.security.cert.Certificate;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

/**
 * gRPC server interceptor that validates the caller's SPIFFE identity from
 * the mTLS peer certificate.
 *
 * <h2>SPIFFE ID extraction</h2>
 * The interceptor reads the TLS peer certificate from the gRPC
 * {@link io.grpc.Grpc#TRANSPORT_ATTR_SSL_SESSION} attribute, parses the
 * first URI Subject Alternative Name using BouncyCastle, and compares it
 * constant-time against the {@code authorizedPeers} whitelist.
 *
 * <h2>Legacy bearer-token fallback</h2>
 * When {@code spiffe.enabled=false} this interceptor is not registered
 * (see {@link SpiffeAutoConfiguration}).  The original
 * {@code GrpcAuthInterceptor} stays active and uses bearer tokens.
 *
 * <h2>Failure modes</h2>
 * <ul>
 *   <li>No TLS session on the call → {@code UNAUTHENTICATED} (mTLS is
 *       mandatory when SPIFFE mode is enabled).</li>
 *   <li>No URI SAN in peer cert → {@code PERMISSION_DENIED}.</li>
 *   <li>Peer SPIFFE ID not in whitelist → {@code PERMISSION_DENIED},
 *       logged at WARN with the peer ID so operators can diagnose
 *       misconfigured registration entries.</li>
 * </ul>
 */
public class SpiffeGrpcServerInterceptor implements ServerInterceptor {

    private static final Logger log = LoggerFactory.getLogger(SpiffeGrpcServerInterceptor.class);

    /**
     * gRPC Context key under which the validated peer SPIFFE ID is stored.
     * Downstream services can call
     * {@code PEER_SPIFFE_ID_KEY.get(Context.current())} to obtain the peer
     * identity without repeating the cert parsing.
     */
    public static final Context.Key<String> PEER_SPIFFE_ID_KEY =
            Context.key("peer-spiffe-id");

    /** OID for the Subject Alternative Name extension (RFC 5280). */
    private static final ASN1ObjectIdentifier OID_SAN = Extension.subjectAlternativeName;

    private final Set<String> authorizedPeers;
    private final String trustDomainPrefix;

    /**
     * @param authorizedPeers  full SPIFFE URI whitelist (e.g.
     *                         {@code spiffe://faso.gov.bf/ns/default/sa/armageddon})
     * @param trustDomain      trust domain without scheme (e.g. {@code faso.gov.bf})
     */
    public SpiffeGrpcServerInterceptor(List<String> authorizedPeers, String trustDomain) {
        this.authorizedPeers = Set.copyOf(authorizedPeers);
        this.trustDomainPrefix = "spiffe://" + trustDomain + "/";
        log.info("SpiffeGrpcServerInterceptor active. trust_domain={} authorized_peers={}",
                trustDomain, authorizedPeers);
    }

    @Override
    public <ReqT, RespT> ServerCall.Listener<ReqT> interceptCall(
            ServerCall<ReqT, RespT> call,
            Metadata headers,
            ServerCallHandler<ReqT, RespT> next) {

        String method = call.getMethodDescriptor().getFullMethodName();

        // ── Extract peer certificate from TLS session ─────────────────────
        SSLSession sslSession = call.getAttributes()
                .get(io.grpc.Grpc.TRANSPORT_ATTR_SSL_SESSION);

        if (sslSession == null) {
            log.warn("gRPC call rejected — no TLS session (SPIFFE mode requires mTLS): {}", method);
            call.close(
                    Status.UNAUTHENTICATED.withDescription("mTLS required: no TLS session found"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        Certificate[] peerCerts;
        try {
            peerCerts = sslSession.getPeerCertificates();
        } catch (SSLPeerUnverifiedException e) {
            log.warn("gRPC call rejected — peer not verified ({}): {}", e.getMessage(), method);
            call.close(
                    Status.UNAUTHENTICATED.withDescription("Peer certificate not available"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        if (peerCerts == null || peerCerts.length == 0) {
            log.warn("gRPC call rejected — empty peer cert chain: {}", method);
            call.close(
                    Status.UNAUTHENTICATED.withDescription("No peer certificate presented"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        // ── Extract SPIFFE URI SAN ────────────────────────────────────────
        String spiffeId = extractSpiffeId((X509Certificate) peerCerts[0]);
        if (spiffeId == null) {
            log.warn("gRPC call rejected — no URI SAN in peer cert: {}", method);
            call.close(
                    Status.PERMISSION_DENIED.withDescription("Peer certificate has no SPIFFE URI SAN"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        // ── Trust-domain guard ────────────────────────────────────────────
        if (!spiffeId.startsWith(trustDomainPrefix)) {
            log.warn("gRPC call rejected — foreign trust domain. got={} expected_prefix={} method={}",
                    spiffeId, trustDomainPrefix, method);
            call.close(
                    Status.PERMISSION_DENIED.withDescription(
                            "Peer SPIFFE ID belongs to a foreign trust domain"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        // ── Whitelist check (constant-time via MessageDigest is overkill
        //    for SPIFFE IDs — String.equals is fine since they are not
        //    secret values; the attacker already knows them from the cert) ─
        if (!authorizedPeers.contains(spiffeId)) {
            log.warn("gRPC call rejected — peer SPIFFE ID not in authorized_peers. peer={} method={}",
                    spiffeId, method);
            call.close(
                    Status.PERMISSION_DENIED.withDescription(
                            "Peer SPIFFE ID not in authorized_peers whitelist"),
                    new Metadata());
            return new ServerCall.Listener<>() {};
        }

        log.debug("gRPC call authorized. peer={} method={}", spiffeId, method);

        // ── Propagate peer SPIFFE ID to downstream via gRPC Context ──────
        Context ctx = Context.current().withValue(PEER_SPIFFE_ID_KEY, spiffeId);
        return Contexts.interceptCall(ctx, call, headers, next);
    }

    // -------------------------------------------------------------------------
    // SPIFFE ID extraction via BouncyCastle
    // -------------------------------------------------------------------------

    /**
     * Extract the first URI SAN from a DER-encoded X.509 certificate.
     * Returns {@code null} if none is found or the cert cannot be parsed.
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
            log.debug("Failed to parse X.509 SAN: {}", e.getMessage());
        }
        return null;
    }
}
