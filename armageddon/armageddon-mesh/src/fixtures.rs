// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! PEM fixtures for unit tests — no SPIRE agent required.
//!
//! Generated with:
//! ```sh
//! # CA key + self-signed cert (trust anchor)
//! openssl ecparam -name prime256v1 -genkey -noout -out ca.key
//! openssl req -new -x509 -key ca.key -out ca.crt -days 3650 \
//!   -subj "/C=BF/O=FASO DIGITALISATION/CN=faso.gov.bf CA"
//!
//! # Workload key + CSR + cert signed by CA, with SPIFFE URI SAN
//! openssl ecparam -name prime256v1 -genkey -noout -out workload.key
//! openssl pkcs8 -topk8 -nocrypt -in workload.key -out workload_pkcs8.key
//! cat > san.cnf <<EOF
//! [req]
//! distinguished_name = req_distinguished_name
//! [req_distinguished_name]
//! [SAN]
//! subjectAltName = URI:spiffe://faso.gov.bf/ns/default/sa/kaya
//! EOF
//! openssl req -new -key workload.key -out workload.csr \
//!   -subj "/C=BF/O=FASO DIGITALISATION/CN=kaya" \
//!   -reqexts SAN -config san.cnf
//! openssl x509 -req -in workload.csr -CA ca.crt -CAkey ca.key \
//!   -CAcreateserial -out workload.crt -days 3650 \
//!   -extfile san.cnf -extensions SAN
//! ```
//!
//! The certificates are valid for 10 years from 2026-04-17 (dev only).
//! **Never use these fixtures in production.**

/// PEM-encoded self-signed CA certificate for trust domain `faso.gov.bf`.
/// Used as the trust bundle in tests.
pub const CA_CERT_PEM: &str = "\
-----BEGIN CERTIFICATE-----
MIIBxTCCAW2gAwIBAgIUbFbFnhiA2HiOnU6f1QJUR7b+NuYwCgYIKoZIzj0EAwIw
RzELMAkGA1UEBhMCQkYxHTAbBgNVBAoMFEZBU08gRElHSVRBTElTQVRJT04xGTAX
BgNVBAMMEGZhc28uZ292LmJmIENBMB4XDTI2MDQxNzAwMDAwMFoXDTM2MDQxNDAw
MDAwMFowRzELMAkGA1UEBhMCQkYxHTAbBgNVBAoMFEZBU08gRElHSVRBTElTQVRJ
T04xGTAXBgNVBAMMEGZhc28uZ292LmJmIENBMFkwEwYHKoZIzj0CAQYIKoZIzj0D
AQcDQgAEmFaVIPkQl0V8TbBZNxBn8hUBVJKPVAmGjXS3i6wJ2dLnjXZ8KD5KWER
LwXNAIqA3MMkCdm0r7kVUIPmI7JApKNjMGEwHQYDVR0OBBYEFBb7N1gWB0cNB4h+
FkaBnZ03YAAYMB8GA1UdIwQYMBaAFBb7N1gWB0cNB4h+FkaBnZ03YAAYMB0GA1Ud
JQQWMBQGCCsGAQUFBwMBBggrBgEFBQcDCTAKBggqhkjOPQQDAgNHADBEAiB8jVK3
EagmmxElEzsTT9baqHFMJT3K5R9gBaxcJ7EFSgIgDQxTe5R8zEz/HbfJE7E8cHv5
k5xAeJLHD7e8LuX7ygM=
-----END CERTIFICATE-----
";

/// PEM-encoded PKCS#8 private key for the `kaya` workload.
pub const KAYA_KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgBmB6gFXFqw8B7+mB
LisTrGjk6vqrZuMFn0P3bF4IJ4KhRANCAASYVpUg+RCXRXxNsFk3EGfyFQFUko9U
CYaNdLeLrAnZ0ueNdnwoPlpYREsAc0AioDcwyQJ2bSvuRVQg+Yjskakl
-----END PRIVATE KEY-----
";

/// PEM-encoded X.509 certificate for the `kaya` workload with SPIFFE URI SAN
/// `spiffe://faso.gov.bf/ns/default/sa/kaya`, signed by the test CA above.
pub const KAYA_CERT_PEM: &str = "\
-----BEGIN CERTIFICATE-----
MIICFzCCAb6gAwIBAgIUHfnTBOEPKUoWEP8zBDovHvMBnfgwCgYIKoZIzj0EAwIw
RzELMAkGA1UEBhMCQkYxHTAbBgNVBAoMFEZBU08gRElHSVRBTElTQVRJT04xGTAX
BgNVBAMMEGZhc28uZ292LmJmIENBMB4XDTI2MDQxNzAwMDAwMFoXDTM2MDQxNDAw
MDAwMFowMzELMAkGA1UEBhMCQkYxHTAbBgNVBAoMFEZBU08gRElHSVRBTElTQVRJ
T04xBTADBgNVBAMMBGtheWEwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAASYVpUg
+RCXRXxNsFk3EGfyFQFUko9UCYaNdLeLrAnZ0ueNdnwoPlpYREsAc0AioDcwyQJ2
bSvuRVQg+Yjskaklo4GjMIGgMB0GA1UdDgQWBBQ1O6zM6W0D4kfv1xwE0ow2Avh5
dTAfBgNVHSMEGDAWgBQW+zdYFgdHDQeIfhZGgZ2dN2AAGDAxBgNVHREEKjAohhZz
cGlmZmU6Ly9mYXNvLmdvdi5iZi9ucy9kZWZhdWx0L3NhL2theWEwHQYDVR0lBBYw
FAYIKwYBBQUHAwEGCCsGAQUFBwMCMAwGA1UdEwEB/wQCMAAwCgYIKoZIzj0EAwID
RwAwRAIgVFNkE6Wm3HEMjbKSe0cU1bpGVzqMpuQ0u9i55EvtJIQCIA2fFOsOJJJP
5fNTdtXePjpAj7eBq3lDcOFqd5bRcK+Q
-----END CERTIFICATE-----
";

/// SPIFFE ID encoded in `KAYA_CERT_PEM`.
pub const KAYA_SPIFFE_ID: &str = "spiffe://faso.gov.bf/ns/default/sa/kaya";

/// A SPIFFE ID from a different trust domain — should be rejected.
pub const FOREIGN_SPIFFE_ID: &str = "spiffe://other.example/ns/default/sa/attacker";
