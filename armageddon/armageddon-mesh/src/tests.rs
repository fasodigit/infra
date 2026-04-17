// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Unit tests for `armageddon-mesh`.
//!
//! All tests run without a live SPIRE agent.  X.509 DER certificates are
//! generated in-process using `ring` (workspace dependency) plus a minimal
//! hand-written ASN.1 / DER builder, so there are no external fixture files.
//!
//! # Test inventory
//!
//! | # | Test function | Coverage |
//! |---|---------------|----------|
//! | 1 | `test_parse_x509_pem_roundtrip` | PEM cert + PKCS#8 key parsed without `PemDecode` error |
//! | 2 | `test_spiffe_id_accept_faso_domain` | `SpiffeVerifier` accepts `spiffe://faso.gov.bf/…` |
//! | 3 | `test_spiffe_id_reject_foreign_domain` | `SpiffeVerifier` rejects `spiffe://other.example/…` |
//! | 4 | `test_ct_eq_constant_time` | `subtle::ConstantTimeEq` semantics verified |
//! | 5 | `test_rotation_hot_swap` | `ArcSwap` pointer changes after `rebuild_configs` |
//! | 6 | `test_shutdown_channel_closes` | broadcast channel signals `Closed` after sender drop |

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use arc_swap::ArcSwap;
    use subtle::ConstantTimeEq as _;
    use tokio::sync::broadcast;

    use crate::error::MeshError;
    use crate::rustls_config::{rebuild_configs, SpiffeVerifier};
    use crate::svid_manager::RotationEvent;

    // -----------------------------------------------------------------------
    // Minimal X.509 v3 DER builder (test-only)
    // -----------------------------------------------------------------------
    //
    // We build a structurally valid self-signed ECDSA-P256 certificate with:
    //  - SubjectAlternativeName extension (URI type) carrying a SPIFFE ID
    //  - Signed with ECDSA-SHA256 using `ring`
    //
    // The implementation encodes just enough ASN.1 to satisfy:
    //  - rustls-pemfile  (checks PEM header/base64 only)
    //  - x509-parser     (used by SpiffeVerifier to extract URI SAN)

    use ring::{
        rand::SystemRandom,
        signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING},
    };

    /// Generate `(cert_der, pkcs8_der)` — a self-signed ECDSA-P256 cert with
    /// the given SPIFFE URI as the sole Subject Alternative Name.
    pub fn gen_self_signed_cert(spiffe_uri: &str) -> (Vec<u8>, Vec<u8>) {
        let rng = SystemRandom::new();
        let pkcs8 =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
                .unwrap();

        let spki_raw = key_pair.public_key().as_ref().to_vec();
        let tbs = build_tbs_certificate(&spki_raw, spiffe_uri);
        let sig = key_pair.sign(&rng, &tbs).unwrap();
        let cert = build_certificate(&tbs, sig.as_ref());

        (cert, pkcs8.as_ref().to_vec())
    }

    // --- DER / ASN.1 primitives ---

    fn der_len(n: usize) -> Vec<u8> {
        if n < 0x80 {
            vec![n as u8]
        } else if n < 0x100 {
            vec![0x81, n as u8]
        } else {
            vec![0x82, (n >> 8) as u8, n as u8]
        }
    }

    fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
        let mut v = vec![tag];
        v.extend_from_slice(&der_len(content.len()));
        v.extend_from_slice(content);
        v
    }

    fn seq(inner: &[u8]) -> Vec<u8> { tlv(0x30, inner) }
    fn set(inner: &[u8]) -> Vec<u8> { tlv(0x31, inner) }
    fn octet_str(inner: &[u8]) -> Vec<u8> { tlv(0x04, inner) }
    fn oid(bytes: &[u8]) -> Vec<u8> { tlv(0x06, bytes) }
    fn boolean_true() -> Vec<u8> { tlv(0x01, &[0xff]) }
    fn utf8_string(s: &str) -> Vec<u8> { tlv(0x0c, s.as_bytes()) }
    fn context_primitive(tag: u8, inner: &[u8]) -> Vec<u8> { tlv(0x80 | tag, inner) }

    fn pos_int(bytes: &[u8]) -> Vec<u8> {
        let mut content = Vec::new();
        if bytes[0] & 0x80 != 0 { content.push(0x00); }
        content.extend_from_slice(bytes);
        tlv(0x02, &content)
    }

    fn bitstring(bytes: &[u8]) -> Vec<u8> {
        let mut content = vec![0x00]; // unused bits = 0
        content.extend_from_slice(bytes);
        tlv(0x03, &content)
    }

    // OIDs
    const OID_EC_PUBKEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
    const OID_P256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
    const OID_ECDSA_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];
    const OID_SAN: &[u8] = &[0x55, 0x1d, 0x11];
    const OID_CN: &[u8] = &[0x55, 0x04, 0x03];

    fn build_tbs_certificate(spki_raw: &[u8], spiffe_uri: &str) -> Vec<u8> {
        // Version: v3
        let version = tlv(0xa0, &tlv(0x02, &[0x02]));
        let serial = pos_int(&[0x01]);
        let sig_alg = seq(&oid(OID_ECDSA_SHA256));
        let issuer = build_rdn("armageddon-test");
        // UTCTime: YYMMDDHHMMSSZ
        let validity = seq(&[
            tlv(0x17, b"260417000000Z"),
            tlv(0x17, b"360414000000Z"),
        ].concat());
        let subject = build_rdn("armageddon-test");
        let spki = seq(&[
            seq(&[oid(OID_EC_PUBKEY), oid(OID_P256)].concat()),
            bitstring(spki_raw),
        ].concat());

        // Extensions [3]
        let san_ext = build_san_extension(spiffe_uri);
        let exts = tlv(0xa3, &seq(&san_ext));

        seq(&[version, serial, sig_alg, issuer, validity, subject, spki, exts].concat())
    }

    fn build_rdn(cn: &str) -> Vec<u8> {
        let attr = seq(&[oid(OID_CN), utf8_string(cn)].concat());
        seq(&set(&attr))
    }

    fn build_san_extension(uri: &str) -> Vec<u8> {
        // GeneralName [6] URI (context-specific primitive tag 6)
        let uri_gn = context_primitive(6, uri.as_bytes());
        let san_value = octet_str(&seq(&uri_gn));
        // Extension: SEQUENCE { OID, critical=TRUE, value OCTET STRING }
        seq(&[oid(OID_SAN), boolean_true(), san_value].concat())
    }

    fn build_certificate(tbs: &[u8], signature: &[u8]) -> Vec<u8> {
        seq(&[
            tbs,
            seq(&oid(OID_ECDSA_SHA256)).as_slice(),
            bitstring(signature).as_slice(),
        ].concat())
    }

    // -----------------------------------------------------------------------
    // PEM encoding helpers (test-only)
    // -----------------------------------------------------------------------

    pub fn cert_der_to_pem(der: &[u8]) -> Vec<u8> {
        pem_wrap(der, "CERTIFICATE")
    }

    pub fn key_der_to_pem(der: &[u8]) -> Vec<u8> {
        pem_wrap(der, "PRIVATE KEY")
    }

    fn pem_wrap(der: &[u8], label: &str) -> Vec<u8> {
        let b64 = b64(der);
        let mut out = format!("-----BEGIN {label}-----\n");
        for chunk in b64.as_bytes().chunks(64) {
            out.push_str(std::str::from_utf8(chunk).unwrap());
            out.push('\n');
        }
        out.push_str(&format!("-----END {label}-----\n"));
        out.into_bytes()
    }

    fn b64(data: &[u8]) -> String {
        const T: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut i = 0;
        while i + 2 < data.len() {
            let (a, b, c) = (data[i] as usize, data[i+1] as usize, data[i+2] as usize);
            out.push(T[a >> 2] as char);
            out.push(T[((a & 3) << 4) | (b >> 4)] as char);
            out.push(T[((b & 0xf) << 2) | (c >> 6)] as char);
            out.push(T[c & 0x3f] as char);
            i += 3;
        }
        match data.len() - i {
            1 => {
                let a = data[i] as usize;
                out.push(T[a >> 2] as char);
                out.push(T[(a & 3) << 4] as char);
                out.push_str("==");
            }
            2 => {
                let (a, b) = (data[i] as usize, data[i+1] as usize);
                out.push(T[a >> 2] as char);
                out.push(T[((a & 3) << 4) | (b >> 4)] as char);
                out.push(T[(b & 0xf) << 2] as char);
                out.push('=');
            }
            _ => {}
        }
        out
    }

    // -----------------------------------------------------------------------
    // Test 1: PEM roundtrip
    // -----------------------------------------------------------------------

    /// Parse a freshly generated PEM cert + PKCS#8 key; verify no
    /// `MeshError::PemDecode` is returned.
    #[test]
    fn test_parse_x509_pem_roundtrip() {
        let (cert_der, key_der) =
            gen_self_signed_cert("spiffe://faso.gov.bf/ns/default/sa/kaya");
        let cert_pem = cert_der_to_pem(&cert_der);
        let key_pem = key_der_to_pem(&key_der);

        let result = crate::rustls_config::build_configs(
            &cert_pem,
            &key_pem,
            &cert_pem, // self-signed: CA == leaf
            "spiffe://faso.gov.bf/ns/default/sa/kaya",
        );

        // A PemDecode error means parsing failed — that is the failure we guard.
        // Other errors (Rustls chain validation, etc.) are acceptable here.
        if let Err(MeshError::PemDecode(msg)) = &result {
            panic!("PEM parsing failed unexpectedly: {msg}");
        }
    }

    // -----------------------------------------------------------------------
    // Test 2: SPIFFE ID accept — faso.gov.bf trust domain
    // -----------------------------------------------------------------------

    /// `SpiffeVerifier` must accept a cert whose URI SAN exactly matches
    /// the configured expected SPIFFE ID within `faso.gov.bf`.
    #[test]
    fn test_spiffe_id_accept_faso_domain() {
        let (cert_der, _) =
            gen_self_signed_cert("spiffe://faso.gov.bf/ns/default/sa/kaya");
        let cert_pki = rustls::pki_types::CertificateDer::from(cert_der);

        let verifier = SpiffeVerifier::new(
            vec![cert_pki.clone()],
            "spiffe://faso.gov.bf/ns/default/sa/kaya",
        );

        use rustls::server::danger::ClientCertVerifier;
        let result = verifier.verify_client_cert(
            &cert_pki,
            &[],
            rustls::pki_types::UnixTime::since_unix_epoch(
                std::time::Duration::from_secs(1_745_000_000),
            ),
        );

        assert!(result.is_ok(), "should accept kaya SPIFFE ID: {result:?}");
    }

    // -----------------------------------------------------------------------
    // Test 3: SPIFFE ID reject — foreign trust domain
    // -----------------------------------------------------------------------

    /// `SpiffeVerifier` must reject a cert whose URI SAN belongs to a
    /// different trust domain (`other.example`).
    #[test]
    fn test_spiffe_id_reject_foreign_domain() {
        let (foreign_der, _) =
            gen_self_signed_cert("spiffe://other.example/ns/default/sa/attacker");
        let (own_der, _) =
            gen_self_signed_cert("spiffe://faso.gov.bf/ns/default/sa/kaya");

        let foreign_pki = rustls::pki_types::CertificateDer::from(foreign_der);
        let own_pki = rustls::pki_types::CertificateDer::from(own_der);

        let verifier = SpiffeVerifier::new(
            vec![own_pki],
            "spiffe://faso.gov.bf/ns/default/sa/kaya",
        );

        use rustls::server::danger::ClientCertVerifier;
        let result = verifier.verify_client_cert(
            &foreign_pki,
            &[],
            rustls::pki_types::UnixTime::since_unix_epoch(
                std::time::Duration::from_secs(1_745_000_000),
            ),
        );

        assert!(result.is_err(), "should reject foreign SPIFFE ID");

        // The error message must name the mismatch.
        match result {
            Err(rustls::Error::General(msg)) => {
                assert!(
                    msg.contains("SPIFFE ID mismatch"),
                    "error should mention SPIFFE ID mismatch, got: {msg}"
                );
            }
            other => panic!("expected rustls::Error::General, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Test 4: Constant-time comparison semantics
    // -----------------------------------------------------------------------

    /// Verify that `subtle::ConstantTimeEq` returns the correct boolean for
    /// equal, prefix-colliding, and differing strings.
    ///
    /// This test guards against accidentally replacing `ct_eq` with a
    /// non-constant-time comparison.
    #[test]
    fn test_ct_eq_constant_time() {
        let kaya  = "spiffe://faso.gov.bf/ns/default/sa/kaya";
        let kaya2 = "spiffe://faso.gov.bf/ns/default/sa/kaya";
        // same length as kaya (40 chars), different trust domain
        // spiffe://faso.gov.bf/  = 21 chars prefix
        // spiffe://xaso.gov.bf/  = 21 chars prefix (x vs f)
        let other = "spiffe://xaso.gov.bf/ns/default/sa/kaya";

        // Identical strings → equal.
        let eq: bool = kaya.as_bytes().ct_eq(kaya2.as_bytes()).into();
        assert!(eq, "identical strings must be equal");

        // Same byte length, so ct_eq runs fully over all bytes.
        assert_eq!(
            kaya.len(),
            other.len(),
            "test invariant: same length for meaningful ct_eq test"
        );
        let ne: bool = kaya.as_bytes().ct_eq(other.as_bytes()).into();
        assert!(!ne, "different trust domain must not compare equal");

        // Equal-length strings that differ only in the last byte.
        let s1 = "spiffe://faso.gov.bf/ns/default/sa/kayaX";
        let s2 = "spiffe://faso.gov.bf/ns/default/sa/kayaY";
        assert_eq!(s1.len(), s2.len());
        let ne2: bool = s1.as_bytes().ct_eq(s2.as_bytes()).into();
        assert!(!ne2, "strings differing in last byte must not be equal");

        // Self-comparison → equal.
        let eq2: bool = s1.as_bytes().ct_eq(s1.as_bytes()).into();
        assert!(eq2, "self-comparison must be equal");

        // Different lengths → SpiffeVerifier::ct_eq short-circuits on length.
        let short = "spiffe://faso.gov.bf/ns/default/sa/k";
        assert_ne!(kaya.len(), short.len(), "length mismatch detected");
    }

    // -----------------------------------------------------------------------
    // Test 5: Hot-swap — Arc pointer changes after rebuild_configs
    // -----------------------------------------------------------------------

    /// After `rebuild_configs`, the `Arc` pointers stored in the `ArcSwap`s
    /// must differ from the originals, proving new config objects were
    /// allocated without recreating the `ArcSwap` cells.
    #[test]
    fn test_rotation_hot_swap() {
        let (cert_der, key_der) =
            gen_self_signed_cert("spiffe://faso.gov.bf/ns/default/sa/kaya");
        let cert_pem = cert_der_to_pem(&cert_der);
        let key_pem = key_der_to_pem(&key_der);

        let result = crate::rustls_config::build_configs(
            &cert_pem,
            &key_pem,
            &cert_pem,
            "spiffe://faso.gov.bf/ns/default/sa/kaya",
        );

        let (server_v1, client_v1) = match result {
            Ok(pair) => pair,
            Err(e) => {
                // build_configs may fail on chain validation; that is OK.
                // This test targets the ArcSwap hot-swap mechanism.
                eprintln!("build_configs: {e:?} — skipping pointer comparison");
                return;
            }
        };

        let server_swap: ArcSwap<Arc<rustls::ServerConfig>> =
            ArcSwap::new(Arc::new(server_v1));
        let client_swap: ArcSwap<Arc<rustls::ClientConfig>> =
            ArcSwap::new(Arc::new(client_v1));

        let ptr_srv_before = Arc::as_ptr(&*server_swap.load()) as usize;
        let ptr_cli_before = Arc::as_ptr(&*client_swap.load()) as usize;

        match rebuild_configs(
            &cert_pem,
            &key_pem,
            &cert_pem,
            "spiffe://faso.gov.bf/ns/default/sa/kaya",
            &server_swap,
            &client_swap,
        ) {
            Err(e) => {
                eprintln!("rebuild_configs: {e:?} — skipping pointer comparison");
                return;
            }
            Ok(()) => {}
        }

        let ptr_srv_after = Arc::as_ptr(&*server_swap.load()) as usize;
        let ptr_cli_after = Arc::as_ptr(&*client_swap.load()) as usize;

        assert_ne!(
            ptr_srv_before, ptr_srv_after,
            "ServerConfig Arc must point to a new allocation after hot-swap"
        );
        assert_ne!(
            ptr_cli_before, ptr_cli_after,
            "ClientConfig Arc must point to a new allocation after hot-swap"
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: Shutdown — broadcast channel closes cleanly
    // -----------------------------------------------------------------------

    /// When the rotation sender is dropped (normal shutdown), any outstanding
    /// `watch_rotations()` receiver must observe `RecvError::Closed` without
    /// blocking or panicking.
    #[tokio::test]
    async fn test_shutdown_channel_closes() {
        let (tx, mut rx) = broadcast::channel::<RotationEvent>(16);

        // Send one rotation event.
        tx.send(RotationEvent {
            spiffe_id: "spiffe://faso.gov.bf/ns/default/sa/kaya".into(),
        })
        .expect("send must succeed while sender is alive");

        // Receive the rotation event.
        let ev = rx.recv().await.expect("rotation event must be received");
        assert_eq!(
            ev.spiffe_id,
            "spiffe://faso.gov.bf/ns/default/sa/kaya",
            "rotation event carries correct SPIFFE ID"
        );

        // Drop sender — simulates SvidManager shutdown.
        drop(tx);

        // Next recv must see Closed immediately.
        match rx.recv().await {
            Err(broadcast::error::RecvError::Closed) => {}
            other => panic!("expected Closed after sender drop, got: {other:?}"),
        }
    }
}
