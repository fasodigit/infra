// SPDX-License-Identifier: AGPL-3.0-or-later
//! JA4 TLS client fingerprinting — native implementation.
//!
//! JA4 is a richer successor to JA3 that encodes:
//! - Transport type (`t` = TCP/TLS, `q` = QUIC, `d` = DTLS)
//! - TLS version negotiated in ClientHello
//! - Whether the ClientHello has an SNI extension
//! - Number of cipher suites (excluding GREASE)
//! - Number of extensions (excluding GREASE)
//! - First ALPN value (two-character tag)
//! - Truncated SHA-256 of sorted cipher suites
//! - Truncated SHA-256 of sorted extension types + signature algorithms
//!
//! Reference: <https://github.com/FoxIO-LLC/ja4>
//!
//! Format: `t<ver><sni><ciphers_count><ext_count><alpn>_<cipher_hash>_<ext_hash>`
//!
//! JA4S (server fingerprint) covers the ServerHello side and follows the same
//! truncated-SHA-256 scheme.

use arc_swap::ArcSwap;
use dashmap::DashMap;
use prometheus::{register_int_counter_vec, IntCounterVec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{info, warn};

// -- constants --

/// GREASE values defined in RFC 8701 (all 0xXAXA patterns).
const GREASE_VALUES: &[u16] = &[
    0x0a0a, 0x1a1a, 0x2a2a, 0x3a3a, 0x4a4a, 0x5a5a, 0x6a6a, 0x7a7a, 0x8a8a, 0x9a9a, 0xaaaa,
    0xbaba, 0xcaca, 0xdada, 0xeaea, 0xfafa,
];

/// TLS extension type 0x0000 = SNI.
#[allow(dead_code)]
pub const EXT_SNI: u16 = 0x0000;
/// TLS extension type 0x000d = signature_algorithms.
#[allow(dead_code)]
pub const EXT_SIG_ALGS: u16 = 0x000d;
/// TLS extension type 0x0010 = ALPN.
#[allow(dead_code)]
pub const EXT_ALPN: u16 = 0x0010;

// -- public data types --

/// All data extracted from a TLS ClientHello, transport-independent.
///
/// Callers populating this struct must filter nothing — JA4 computation
/// applies GREASE filtering internally.
#[derive(Debug, Clone, Default)]
pub struct ClientHello {
    /// Offered TLS version (from `supported_versions` extension if present,
    /// else the legacy `client_hello.version` field).
    pub tls_version: u16,
    /// Raw cipher suite list in wire order.
    pub cipher_suites: Vec<u16>,
    /// Extension type codes in wire order.
    pub extension_types: Vec<u16>,
    /// SNI hostname, if present.
    pub sni: Option<String>,
    /// ALPN protocols in preference order.
    pub alpn_protocols: Vec<String>,
    /// Signature algorithm pairs (hash_alg << 8 | sig_alg), wire order.
    pub sig_algs: Vec<u16>,
    /// Transport type: `'t'` for TCP/TLS, `'q'` for QUIC, `'d'` for DTLS.
    pub transport: char,
}

/// All data extracted from a TLS ServerHello (for JA4S computation).
#[derive(Debug, Clone, Default)]
pub struct ServerHello {
    /// Selected TLS version.
    pub tls_version: u16,
    /// Selected cipher suite.
    pub cipher_suite: u16,
    /// Extension type codes returned by the server, in wire order.
    pub extension_types: Vec<u16>,
    /// ALPN protocol selected by the server.
    pub alpn_protocol: Option<String>,
}

/// A JA4 fingerprint, decomposed into its canonical fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ja4Fingerprint {
    /// Full JA4 string, e.g. `t13d1516h2_acb65a...8fe2_93af...`
    pub ja4: String,
    /// Raw (unhashed) representation, useful for debugging.
    pub ja4_r: String,
}

/// A JA4S fingerprint (server-side).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ja4sFingerprint {
    /// Full JA4S string.
    pub ja4s: String,
    /// Raw (unhashed) representation.
    pub ja4s_r: String,
}

// -- helper functions --

/// Returns `true` if the value is a GREASE byte sequence.
#[inline]
fn is_grease(v: u16) -> bool {
    GREASE_VALUES.contains(&v)
}

/// Encode a TLS version to the two-character JA4 tag.
///
/// | Wire value | Tag |
/// |-----------|-----|
/// | 0x0304    | 13  |
/// | 0x0303    | 12  |
/// | 0x0302    | 11  |
/// | 0x0301    | 10  |
/// | 0x0300    | s3  |
/// | unknown   | 00  |
fn encode_tls_version(v: u16) -> &'static str {
    match v {
        0x0304 => "13",
        0x0303 => "12",
        0x0302 => "11",
        0x0301 => "10",
        0x0300 => "s3",
        _ => "00",
    }
}

/// Return the first two characters of the first ALPN value, right-padded with
/// `'0'` if shorter, or `"00"` if the list is empty.
fn encode_alpn(alpn: &[String]) -> String {
    match alpn.first() {
        None => "00".to_string(),
        Some(proto) => {
            let bytes = proto.as_bytes();
            match bytes.len() {
                0 => "00".to_string(),
                1 => format!("{}0", bytes[0] as char),
                _ => format!("{}{}", bytes[0] as char, bytes[1] as char),
            }
        }
    }
}

/// Compute the 12-character truncated hex SHA-256 of a comma-separated list
/// of decimal values.
fn truncated_sha256_of_csv(values: &[u16]) -> String {
    let csv: String = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let hash = Sha256::digest(csv.as_bytes());
    // First 12 hex chars = 6 bytes
    hex::encode(&hash[..6])
}

// -- JA4 computation --

/// Compute the JA4 fingerprint from a parsed `ClientHello`.
///
/// Returns both the fingerprint string and its raw (unhashed) variant.
pub fn compute_ja4(hello: &ClientHello) -> Ja4Fingerprint {
    // Filter GREASE from ciphers and extensions
    let ciphers: Vec<u16> = hello
        .cipher_suites
        .iter()
        .copied()
        .filter(|&c| !is_grease(c))
        .collect();

    let extensions: Vec<u16> = hello
        .extension_types
        .iter()
        .copied()
        .filter(|&e| !is_grease(e))
        .collect();

    let transport = hello.transport;
    let version_tag = encode_tls_version(hello.tls_version);
    let sni_tag = if hello.sni.is_some() { 'd' } else { 'i' }; // d=domain, i=IP/no-SNI
    let cipher_count = ciphers.len().min(99);
    let ext_count = extensions.len().min(99);
    let alpn_tag = encode_alpn(&hello.alpn_protocols);

    // JA4 part A: header
    let part_a = format!(
        "{transport}{version_tag}{sni_tag}{cipher_count:02}{ext_count:02}{alpn_tag}"
    );

    // JA4 part B: sorted cipher suites, SHA-256 truncated
    let mut sorted_ciphers = ciphers.clone();
    sorted_ciphers.sort_unstable();
    let part_b = truncated_sha256_of_csv(&sorted_ciphers);

    // JA4 part C: sorted extensions + sorted sig algs appended, SHA-256 truncated
    let mut sorted_exts = extensions.clone();
    sorted_exts.sort_unstable();

    let mut sorted_sigs = hello.sig_algs.clone();
    sorted_sigs.sort_unstable();

    // Concatenate extension codes and sig alg codes with separator '_'
    let ext_csv: String = sorted_exts
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sig_csv: String = sorted_sigs
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let part_c_raw = if sig_csv.is_empty() {
        ext_csv.clone()
    } else {
        format!("{ext_csv}_{sig_csv}")
    };
    let hash_c = Sha256::digest(part_c_raw.as_bytes());
    let part_c = hex::encode(&hash_c[..6]);

    // Raw variant (debugging / threat intel sharing)
    let raw_b = sorted_ciphers
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let ja4_r = format!("{part_a}_{raw_b}_{part_c_raw}");
    let ja4 = format!("{part_a}_{part_b}_{part_c}");

    Ja4Fingerprint { ja4, ja4_r }
}

// -- JA4S computation --

/// Compute the JA4S fingerprint from a parsed `ServerHello`.
pub fn compute_ja4s(hello: &ServerHello) -> Ja4sFingerprint {
    let extensions: Vec<u16> = hello
        .extension_types
        .iter()
        .copied()
        .filter(|&e| !is_grease(e))
        .collect();

    let version_tag = encode_tls_version(hello.tls_version);
    let ext_count = extensions.len().min(99);
    let alpn_tag = encode_alpn(
        hello
            .alpn_protocol
            .as_ref()
            .map(|p| std::slice::from_ref(p))
            .unwrap_or(&[]),
    );

    let part_a = format!("t{version_tag}{ext_count:02}{alpn_tag}");

    // JA4S part B: selected cipher suite (single value)
    let part_b = format!("{:04x}", hello.cipher_suite);

    // JA4S part C: sorted extensions SHA-256 truncated
    let mut sorted_exts = extensions.clone();
    sorted_exts.sort_unstable();
    let ext_csv: String = sorted_exts
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let hash_c = Sha256::digest(ext_csv.as_bytes());
    let part_c = hex::encode(&hash_c[..6]);

    let ja4s = format!("{part_a}_{part_b}_{part_c}");
    let ja4s_r = format!("{part_a}_{part_b}_{ext_csv}");

    Ja4sFingerprint { ja4s, ja4s_r }
}

// -- blocklist entry --

/// An entry in the JA4 blocklist YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ja4BlockEntry {
    /// JA4 hash string to block.
    pub hash: String,
    /// Human-readable description (bot family, scanner name, …).
    pub description: String,
    /// Whether to hard-block (`true`) or only challenge (`false`).
    #[serde(default = "default_true")]
    pub block: bool,
}

fn default_true() -> bool {
    true
}

/// Deserialization wrapper for the `ja4-blocklist.yaml` file.
#[derive(Debug, Deserialize)]
struct Ja4BlocklistFile {
    blocklist: Vec<Ja4BlockEntry>,
}

// -- engine --

lazy_static::lazy_static! {
    static ref JA4_COUNTER: IntCounterVec = register_int_counter_vec!(
        "armageddon_ja4_total",
        "Total TLS connections per JA4 fingerprint",
        &["hash"]
    )
    .expect("failed to register armageddon_ja4_total metric");
}

/// Hot-reloadable JA4 blocklist + Prometheus metrics engine.
pub struct Ja4Engine {
    /// Current blocklist, updated atomically on hot-reload.
    blocklist: Arc<ArcSwap<HashSet<String>>>,
    /// Path to the YAML blocklist file.
    blocklist_path: Option<String>,
    /// Per-hash counter for top-10 reporting.
    top_hashes: Arc<DashMap<String, u64>>,
}

impl Ja4Engine {
    /// Create a new `Ja4Engine`, optionally loading an initial blocklist.
    pub fn new(blocklist_path: Option<&str>) -> Self {
        let engine = Self {
            blocklist: Arc::new(ArcSwap::new(Arc::new(HashSet::new()))),
            blocklist_path: blocklist_path.map(String::from),
            top_hashes: Arc::new(DashMap::new()),
        };
        if let Some(path) = blocklist_path {
            if let Err(e) = engine.reload_blocklist_from(path) {
                warn!("JA4 blocklist initial load failed ({}): {}", path, e);
            }
        }
        engine
    }

    /// Reload the blocklist from disk. Safe to call from a hot-reload task.
    pub fn reload(&self) -> std::io::Result<usize> {
        match &self.blocklist_path {
            None => Ok(0),
            Some(path) => self.reload_blocklist_from(path),
        }
    }

    fn reload_blocklist_from(&self, path: &str) -> std::io::Result<usize> {
        let content = std::fs::read_to_string(path)?;
        let parsed: Ja4BlocklistFile = serde_yaml::from_str(&content).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        let hashes: HashSet<String> = parsed
            .blocklist
            .into_iter()
            .map(|e| e.hash)
            .collect();
        let count = hashes.len();
        self.blocklist.store(Arc::new(hashes));
        info!("JA4 blocklist reloaded: {} entries from {}", count, path);
        Ok(count)
    }

    /// Record a JA4 fingerprint observation and return whether it is blocked.
    ///
    /// Increments the Prometheus counter `armageddon_ja4_total{hash}`.
    pub fn observe(&self, fingerprint: &str) -> bool {
        // Prometheus counter — use only for the top-10 bucket approach
        JA4_COUNTER.with_label_values(&[fingerprint]).inc();

        // In-memory frequency map for top-10 reporting
        *self.top_hashes.entry(fingerprint.to_string()).or_insert(0) += 1;

        self.blocklist.load().contains(fingerprint)
    }

    /// Returns whether a JA4 hash is on the blocklist without updating counters.
    pub fn is_blocked(&self, fingerprint: &str) -> bool {
        self.blocklist.load().contains(fingerprint)
    }

    /// Return the top-N most-seen JA4 hashes and their counts.
    pub fn top_n(&self, n: usize) -> Vec<(String, u64)> {
        let mut pairs: Vec<(String, u64)> = self
            .top_hashes
            .iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs.truncate(n);
        pairs
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a ClientHello that mimics a known TLS client.
    fn make_hello(
        transport: char,
        tls_version: u16,
        sni: Option<&str>,
        cipher_suites: Vec<u16>,
        extension_types: Vec<u16>,
        alpn_protocols: Vec<&str>,
        sig_algs: Vec<u16>,
    ) -> ClientHello {
        ClientHello {
            transport,
            tls_version,
            sni: sni.map(String::from),
            cipher_suites,
            extension_types,
            alpn_protocols: alpn_protocols.into_iter().map(String::from).collect(),
            sig_algs,
        }
    }

    /// Chrome 120 (TLS 1.3) — canonical fingerprint must be deterministic.
    #[test]
    fn test_ja4_chrome_deterministic() {
        let hello = make_hello(
            't',
            0x0304, // TLS 1.3
            Some("example.com"),
            // Typical Chrome cipher list (GREASE + real suites)
            vec![
                0x4a4a, // GREASE — must be filtered
                0x1301, // TLS_AES_128_GCM_SHA256
                0x1302, // TLS_AES_256_GCM_SHA384
                0x1303, // TLS_CHACHA20_POLY1305_SHA256
                0xc02b, // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
                0xc02f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
            ],
            vec![
                0x0000, // SNI
                0x0017, // extended_master_secret
                0xff01, // renegotiation_info
                0x000a, // supported_groups
                0x000b, // ec_point_formats
                0x0010, // ALPN
                0x0012, // signed_cert_timestamp
                0x000d, // signature_algorithms
                0x002b, // supported_versions
                0x002d, // psk_key_exchange_modes
                0x0033, // key_share
            ],
            vec!["h2", "http/1.1"],
            vec![0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806, 0x0601],
        );

        let fp = compute_ja4(&hello);

        // Part A: transport=t, version=13, sni=d (has SNI), ciphers=05 (6 - 1 GREASE),
        //         exts=11, alpn=h2
        assert!(fp.ja4.starts_with("t13d"), "part A prefix: {}", fp.ja4);
        assert!(fp.ja4.contains("h2"), "ALPN tag: {}", fp.ja4);

        // Output is always `partA_12hexchars_12hexchars`
        let parts: Vec<&str> = fp.ja4.split('_').collect();
        assert_eq!(parts.len(), 3, "JA4 must have 3 underscore-delimited parts");
        assert_eq!(parts[1].len(), 12, "cipher hash must be 12 chars");
        assert_eq!(parts[2].len(), 12, "ext hash must be 12 chars");

        // Idempotent: same input → same output
        let fp2 = compute_ja4(&hello);
        assert_eq!(fp.ja4, fp2.ja4);
    }

    /// Firefox 121 (TLS 1.3, no ALPN in hello).
    #[test]
    fn test_ja4_firefox_no_alpn() {
        let hello = make_hello(
            't',
            0x0304,
            Some("www.faso.bf"),
            vec![
                0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8,
            ],
            vec![
                0x0000, 0xff01, 0x000a, 0x000b, 0x0023, 0x0010, 0x000d, 0x002b, 0x002d, 0x0033,
            ],
            vec![], // no ALPN offered
            vec![0x0403, 0x0503, 0x0603],
        );
        let fp = compute_ja4(&hello);

        // ALPN tag must be "00" when no protocols are offered
        let parts: Vec<&str> = fp.ja4.split('_').collect();
        assert!(
            parts[0].ends_with("00"),
            "ALPN tag should be 00 for Firefox without ALPN: {}",
            fp.ja4
        );
    }

    /// curl/7.x (TLS 1.2, no SNI forced, minimal cipher list).
    #[test]
    fn test_ja4_curl_tls12_no_sni() {
        let hello = make_hello(
            't',
            0x0303, // TLS 1.2
            None,   // no SNI
            vec![0xc02b, 0xc02f, 0x009e, 0x009c, 0x003d, 0x003c],
            vec![0x000d, 0x000a, 0x000b, 0x0017, 0xff01],
            vec!["http/1.1"],
            vec![0x0401, 0x0501, 0x0601],
        );
        let fp = compute_ja4(&hello);

        // No SNI → 'i' tag in part A
        assert!(fp.ja4.starts_with("t12i"), "expected t12i prefix: {}", fp.ja4);
    }

    /// Go http.Client default (TLS 1.3, no SNI override, ALPN h2).
    #[test]
    fn test_ja4_go_httpclient() {
        let hello = make_hello(
            't',
            0x0304,
            Some("api.faso.bf"),
            vec![
                0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc030, 0xc02c, 0xcca9, 0xcca8,
            ],
            vec![
                0x0000, 0x0017, 0xff01, 0x000a, 0x000b, 0x0010, 0x000d, 0x002b, 0x0033,
            ],
            vec!["h2", "http/1.1"],
            vec![0x0403, 0x0804, 0x0401, 0x0503],
        );
        let fp = compute_ja4(&hello);
        assert!(fp.ja4.starts_with("t13d"), "Go http.Client: {}", fp.ja4);
        let parts: Vec<&str> = fp.ja4.split('_').collect();
        assert_eq!(parts.len(), 3);
    }

    /// Safari 17 (TLS 1.3, QUIC transport, ALPN h3).
    #[test]
    fn test_ja4_safari_quic() {
        let hello = make_hello(
            'q',             // QUIC transport
            0x0304,
            Some("faso.bf"),
            vec![0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f],
            vec![0x0000, 0x0010, 0x000d, 0x002b, 0x0033, 0x002d],
            vec!["h3", "h2"],
            vec![0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806],
        );
        let fp = compute_ja4(&hello);
        // QUIC transport → starts with 'q'
        assert!(fp.ja4.starts_with("q13d"), "Safari QUIC: {}", fp.ja4);
        // ALPN first proto is "h3" → tag "h3"
        assert!(fp.ja4.contains("h3"), "ALPN tag h3: {}", fp.ja4);
    }

    /// JA4S: server fingerprint structure check.
    #[test]
    fn test_ja4s_structure() {
        let server_hello = ServerHello {
            tls_version: 0x0304,
            cipher_suite: 0x1301, // TLS_AES_128_GCM_SHA256
            extension_types: vec![0x002b, 0x0033, 0x0010],
            alpn_protocol: Some("h2".to_string()),
        };
        let fp = compute_ja4s(&server_hello);
        let parts: Vec<&str> = fp.ja4s.split('_').collect();
        assert_eq!(parts.len(), 3, "JA4S must have 3 parts: {}", fp.ja4s);
        // Part B for JA4S is a 4-character hex cipher suite code
        assert_eq!(parts[1], "1301", "cipher suite hex: {}", parts[1]);
    }

    /// Blocklist engine: observe increments counter and detects blocked hash.
    #[test]
    fn test_ja4_engine_blocklist() {
        use std::io::Write;

        // Write a temporary YAML blocklist
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            "blocklist:\n  - hash: t13d0511h2_deadbeef1234_cafebabe5678\n    description: test-bot\n    block: true"
        )
        .unwrap();

        let engine = Ja4Engine::new(Some(tmp.path().to_str().unwrap()));
        let blocked = "t13d0511h2_deadbeef1234_cafebabe5678";
        let allowed = "t13d0511h2_aabbccddee00_112233445566";

        // Observe blocked twice, allowed once — blocked must rank higher in top_n.
        assert!(engine.observe(blocked), "blocked hash must be detected (1st)");
        assert!(engine.observe(blocked), "blocked hash must be detected (2nd)");
        assert!(!engine.observe(allowed), "unknown hash must pass");

        // Top-1 must be the blocked hash observed twice.
        let top = engine.top_n(1);
        assert_eq!(top[0].0, blocked, "top-1 hash mismatch");
        assert_eq!(top[0].1, 2, "top-1 count should be 2");
    }

    /// GREASE filtering: GREASE values must not appear in cipher counts.
    #[test]
    fn test_grease_filtering() {
        let hello = make_hello(
            't',
            0x0304,
            Some("example.com"),
            // 3 real ciphers + 1 GREASE
            vec![0xaaaa, 0x1301, 0x1302, 0x1303],
            vec![0xfafa, 0x0000, 0x0010],
            vec!["h2"],
            vec![],
        );
        let fp = compute_ja4(&hello);
        // Cipher count after GREASE removal = 3 → "03"
        // Extension count after GREASE removal = 2 → "02"
        let parts: Vec<&str> = fp.ja4.split('_').collect();
        let header = parts[0];
        // header = t13d0302h2, positions: t|13|d|03|02|h2
        assert_eq!(&header[4..6], "03", "cipher count: {}", header);
        assert_eq!(&header[6..8], "02", "ext count: {}", header);
    }
}
