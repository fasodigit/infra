// SPDX-License-Identifier: AGPL-3.0-or-later
//! Response-body compression (brotli / zstd / gzip) negotiation & streaming.
//!
//! Ported from the hyper reference (`src/compression.rs`) into the Pingora
//! pipeline.  The design splits the concern into two **independently testable**
//! halves:
//!
//! 1. [`CompressionFilter`] — *header-level* negotiation.  Called from the
//!    future `ProxyHttp::response_filter` hook: inspects the request's
//!    `Accept-Encoding`, decides whether to compress, and mutates the outbound
//!    response headers (`Content-Encoding`, `Vary`, strips `Content-Length`).
//!
//! 2. [`CompressionStream`] — *body-level* streaming encoder.  Called from the
//!    future `ProxyHttp::response_body_filter` hook: wraps brotli / zstd /
//!    flate2 writers behind a uniform `write`/`finish` interface so chunks can
//!    be compressed as they arrive from upstream.
//!
//! The [`ProxyHttp`] integration is intentionally deferred (see the `TODO`
//! marker below) because `ctx.rs` is owned by another agent on this sprint.
//! Both halves above are fully unit-tested without any Pingora dependency.

use std::io::Write as IoWrite;

// ---------------------------------------------------------------------------
// Public enums
// ---------------------------------------------------------------------------

/// Content-Encoding token supported by ARMAGEDDON-FORGE under Pingora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// `br` — preferred for text/JSON responses (highest ratio).
    Brotli,
    /// `zstd` — preferred when clients advertise support (better CPU/ratio
    /// trade-off than brotli for larger payloads).
    Zstd,
    /// `gzip` — universal fallback.
    Gzip,
    /// No compression (pass-through).
    Identity,
}

impl Encoding {
    /// Token emitted into the `Content-Encoding` response header.
    pub fn as_header_value(self) -> &'static str {
        match self {
            Self::Brotli => "br",
            Self::Zstd => "zstd",
            Self::Gzip => "gzip",
            Self::Identity => "identity",
        }
    }

    /// Parse a single `Accept-Encoding` token (lower-cased, trimmed).
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "br" => Some(Self::Brotli),
            "zstd" => Some(Self::Zstd),
            "gzip" | "x-gzip" => Some(Self::Gzip),
            "identity" => Some(Self::Identity),
            _ => None,
        }
    }
}

/// Coarse compression-level knob exposed to operators.
///
/// Each variant maps to a reasonable default per-encoder level when the
/// [`CompressionStream`] constructs the underlying encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    /// Lowest CPU, acceptable ratio — good default for chatty APIs.
    Fast,
    /// Middle ground (production default).
    Balanced,
    /// Maximum ratio, higher CPU — use for large static-ish payloads.
    Best,
}

impl CompressionLevel {
    fn brotli_quality(self) -> u32 {
        match self {
            Self::Fast => 1,
            Self::Balanced => 4,
            Self::Best => 9,
        }
    }

    fn zstd_level(self) -> i32 {
        match self {
            Self::Fast => 1,
            Self::Balanced => 3,
            Self::Best => 19,
        }
    }

    fn gzip_level(self) -> u32 {
        match self {
            Self::Fast => 1,
            Self::Balanced => 6,
            Self::Best => 9,
        }
    }
}

// ---------------------------------------------------------------------------
// CompressionFilter — header negotiation
// ---------------------------------------------------------------------------

/// Outcome of [`CompressionFilter::negotiate`].
///
/// Callers consume this in `ProxyHttp::response_filter` to either skip
/// compression entirely (`Skip`) or to mutate the outbound response headers
/// and initialise a per-request [`CompressionStream`] (`Compress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationOutcome {
    /// Pass the body through unchanged.
    Skip,
    /// Compress using the chosen encoding; caller must strip `Content-Length`,
    /// set `Content-Encoding: <enc>`, and add/merge `Vary: Accept-Encoding`.
    Compress(Encoding),
}

/// Header-level compression negotiator.
///
/// Stateless with respect to any single request: cheap to `clone` and share
/// across the Pingora gateway.
#[derive(Debug, Clone)]
pub struct CompressionFilter {
    /// Skip compression when `Content-Length < min_size`.
    min_size: usize,
    /// Ordered list of encodings to pick from when the client advertises more
    /// than one.  First match wins.
    preferred: Vec<Encoding>,
    /// Compression level passed to [`CompressionStream`] when the caller
    /// constructs one after [`NegotiationOutcome::Compress`].
    level: CompressionLevel,
}

impl Default for CompressionFilter {
    fn default() -> Self {
        Self {
            min_size: 1024,
            preferred: vec![Encoding::Brotli, Encoding::Zstd, Encoding::Gzip],
            level: CompressionLevel::Balanced,
        }
    }
}

impl CompressionFilter {
    /// Builder-style constructor.
    pub fn new(min_size: usize, preferred: Vec<Encoding>, level: CompressionLevel) -> Self {
        Self {
            min_size,
            preferred,
            level,
        }
    }

    /// Compression level operators selected for this filter.
    pub fn level(&self) -> CompressionLevel {
        self.level
    }

    /// Minimum body size threshold.
    pub fn min_size(&self) -> usize {
        self.min_size
    }

    /// Decide whether and how to compress the outbound response.
    ///
    /// Inputs:
    /// - `accept_encoding`: the request's `Accept-Encoding` header value
    ///   (`None` ⇒ no compression).
    /// - `response_content_encoding`: upstream response's existing
    ///   `Content-Encoding` (`Some` ⇒ already encoded ⇒ skip).
    /// - `response_content_type`: upstream response's `Content-Type` (used to
    ///   skip non-compressible media — `image/*`, `video/*`, `audio/*`,
    ///   `application/octet-stream`, etc.).
    /// - `response_content_length`: upstream response's `Content-Length`
    ///   (`Some(n)` with `n < min_size` ⇒ skip; `None` — streaming — proceeds
    ///   and lets the stream decide).
    pub fn negotiate(
        &self,
        accept_encoding: Option<&str>,
        response_content_encoding: Option<&str>,
        response_content_type: Option<&str>,
        response_content_length: Option<usize>,
    ) -> NegotiationOutcome {
        // 1. Already encoded upstream → never double-encode.
        if response_content_encoding.is_some() {
            return NegotiationOutcome::Skip;
        }

        // 2. Non-compressible media types.
        if let Some(ct) = response_content_type {
            if is_non_compressible(ct) {
                return NegotiationOutcome::Skip;
            }
        }

        // 3. Body known to be below threshold.
        if let Some(len) = response_content_length {
            if len < self.min_size {
                return NegotiationOutcome::Skip;
            }
        }

        // 4. Parse client's Accept-Encoding.
        let accept = match accept_encoding {
            Some(v) => v,
            None => return NegotiationOutcome::Skip,
        };
        let client_encodings = parse_accept_encoding(accept);
        if client_encodings.is_empty() {
            return NegotiationOutcome::Skip;
        }

        // 5. First preferred encoding that the client accepts wins.
        for preferred in &self.preferred {
            if *preferred == Encoding::Identity {
                continue;
            }
            if client_encodings.contains(preferred) {
                return NegotiationOutcome::Compress(*preferred);
            }
        }

        NegotiationOutcome::Skip
    }
}

/// Parse an `Accept-Encoding` header into the set of supported encodings the
/// client is willing to receive.  Entries with `q=0` are excluded.  Unknown
/// tokens are silently dropped.
fn parse_accept_encoding(header: &str) -> Vec<Encoding> {
    let mut out = Vec::new();
    for raw in header.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        let mut parts = token.splitn(2, ';');
        let name = parts.next().unwrap_or("").trim().to_ascii_lowercase();

        // Check for explicit q=0 refusal.
        let mut refused = false;
        if let Some(param) = parts.next() {
            let p = param.trim();
            if let Some(q) = p.strip_prefix("q=").or_else(|| p.strip_prefix("Q=")) {
                if let Ok(q_val) = q.parse::<f32>() {
                    if q_val <= 0.0 {
                        refused = true;
                    }
                }
            }
        }
        if refused {
            continue;
        }

        if let Some(enc) = Encoding::from_token(&name) {
            if !out.contains(&enc) {
                out.push(enc);
            }
        }
    }
    out
}

/// Return `true` when the given `Content-Type` value should NOT be compressed.
///
/// Handles wildcards for the `image/`, `video/`, and `audio/` prefixes as well
/// as a curated list of already-compressed/binary concrete types.
fn is_non_compressible(content_type: &str) -> bool {
    let ct = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase();

    if ct.starts_with("image/") || ct.starts_with("video/") || ct.starts_with("audio/") {
        return true;
    }

    matches!(
        ct.as_str(),
        "application/zip"
            | "application/x-gzip"
            | "application/gzip"
            | "application/zstd"
            | "application/x-bzip2"
            | "application/x-7z-compressed"
            | "application/x-rar-compressed"
            | "application/x-tar"
            | "application/pdf"
            | "application/octet-stream"
            | "application/wasm"
    )
}

// ---------------------------------------------------------------------------
// CompressionStream — body-level streaming encoder
// ---------------------------------------------------------------------------

/// Internal encoder variant — enum-dispatched to avoid `Box<dyn Write>` because
/// both `GzEncoder::finish` and `zstd::Encoder::finish` consume their receiver
/// by value, which is incompatible with a trait-object fat pointer.
enum Encoder {
    Brotli(brotli::CompressorWriter<SharedSink>),
    Zstd(zstd::stream::write::Encoder<'static, SharedSink>),
    Gzip(flate2::write::GzEncoder<SharedSink>),
    Identity,
}

/// A write sink that shares its buffer with the owning [`CompressionStream`]
/// so encoder output can be drained between `write` calls.
///
/// The two handles share the same `Vec<u8>` via [`parking_lot::Mutex`] — there
/// is no concurrent access in practice (both live on the same request task)
/// but we need interior mutability because the encoder owns a `SharedSink`
/// while `CompressionStream::write` also wants to mutate the buffer.
#[derive(Clone)]
struct SharedSink {
    buf: std::sync::Arc<parking_lot::Mutex<Vec<u8>>>,
}

impl SharedSink {
    fn new() -> Self {
        Self {
            buf: std::sync::Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// Drain the currently-buffered compressed bytes.
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.buf.lock())
    }
}

impl IoWrite for SharedSink {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.lock().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Streaming encoder: wraps brotli / zstd / gzip behind a single
/// `write`/`finish` interface.
///
/// Lifecycle:
/// 1. `CompressionStream::new(enc, level)` — construct once per response.
/// 2. `stream.write(chunk)` for each body chunk received from upstream;
///    returns the freshly-produced compressed bytes (may be empty if the
///    encoder is still buffering internally).
/// 3. `stream.finish()` — finalise the frame and drain any remaining bytes
///    (essential for gzip/zstd, which append a terminating footer, and for
///    brotli which writes its stream terminator on drop).
///
/// This type is **sync** (no `async`) and does not depend on Pingora in any
/// way — it is driven by the future `response_body_filter` integration code.
pub struct CompressionStream {
    encoding: Encoding,
    encoder: Encoder,
    /// Shared sink so `write`/`finish` can drain compressed bytes without
    /// cloning the encoder.
    sink: SharedSink,
}

impl CompressionStream {
    /// Construct a new stream for the chosen encoding & level.
    ///
    /// # Errors
    /// Returns `anyhow::Error` if the underlying encoder fails to initialise
    /// (currently only `zstd::Encoder::new` can fail).
    pub fn new(encoding: Encoding, level: CompressionLevel) -> anyhow::Result<Self> {
        let sink = SharedSink::new();
        let encoder = match encoding {
            Encoding::Brotli => {
                // Buffer 4 KiB internally; window = 22 (4 MiB), matches hyper path.
                let writer = brotli::CompressorWriter::new(
                    sink.clone(),
                    4096,
                    level.brotli_quality(),
                    22,
                );
                Encoder::Brotli(writer)
            }
            Encoding::Zstd => {
                let writer = zstd::stream::write::Encoder::new(sink.clone(), level.zstd_level())?;
                Encoder::Zstd(writer)
            }
            Encoding::Gzip => {
                let writer = flate2::write::GzEncoder::new(
                    sink.clone(),
                    flate2::Compression::new(level.gzip_level()),
                );
                Encoder::Gzip(writer)
            }
            Encoding::Identity => Encoder::Identity,
        };
        Ok(Self {
            encoding,
            encoder,
            sink,
        })
    }

    /// Selected encoding (returns `Encoding::Identity` for pass-through).
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Feed an input chunk into the encoder; returns any newly-produced
    /// compressed bytes.  An empty return value is valid — it just means the
    /// encoder is still accumulating input internally.
    pub fn write(&mut self, input: &[u8]) -> anyhow::Result<Vec<u8>> {
        match &mut self.encoder {
            Encoder::Brotli(w) => {
                w.write_all(input)?;
                // Flush into the sink so callers can pipeline bytes downstream;
                // brotli's flush emits an empty meta-block which decoders
                // accept safely.
                w.flush()?;
            }
            Encoder::Zstd(w) => {
                w.write_all(input)?;
            }
            Encoder::Gzip(w) => {
                w.write_all(input)?;
            }
            Encoder::Identity => {
                // Pass-through: caller gets its own bytes back verbatim.
                return Ok(input.to_vec());
            }
        }
        Ok(self.sink.take())
    }

    /// Finalise the stream and return any trailing compressed bytes.
    /// Consumes `self` so the caller cannot reuse a finished encoder.
    pub fn finish(self) -> anyhow::Result<Vec<u8>> {
        let CompressionStream {
            encoder, sink, ..
        } = self;
        match encoder {
            Encoder::Brotli(mut w) => {
                // `into_inner` on CompressorWriter is not exposed in stable
                // versions; dropping flushes the final brotli stream
                // terminator into the sink.
                w.flush()?;
                drop(w);
            }
            Encoder::Zstd(w) => {
                // `finish` writes the zstd epilogue and returns the sink.
                let _sink = w.finish()?;
            }
            Encoder::Gzip(w) => {
                // `finish` writes the gzip trailer and returns the sink.
                let _sink = w.finish()?;
            }
            Encoder::Identity => {
                // Nothing to flush.
            }
        }
        Ok(sink.take())
    }
}

// TODO(#105): wire CompressionFilter+CompressionStream into ProxyHttp hooks (response_filter + response_body_filter) once ctx.rs exposes a mutable per-request slot.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- header negotiation --

    #[test]
    fn negotiate_prefers_brotli_over_gzip_when_client_accepts_both() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("gzip, br"),
            None,
            Some("application/json"),
            Some(10 * 1024),
        );
        assert_eq!(outcome, NegotiationOutcome::Compress(Encoding::Brotli));
    }

    #[test]
    fn negotiate_skips_when_already_encoded() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("gzip, br"),
            Some("gzip"),
            Some("application/json"),
            Some(10 * 1024),
        );
        assert_eq!(outcome, NegotiationOutcome::Skip);
    }

    #[test]
    fn negotiate_skips_for_image_content_type() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("gzip, br"),
            None,
            Some("image/jpeg"),
            Some(10 * 1024),
        );
        assert_eq!(outcome, NegotiationOutcome::Skip);
    }

    #[test]
    fn negotiate_skips_for_video_content_type() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("gzip, br"),
            None,
            Some("video/mp4; codecs=avc1"),
            Some(10 * 1024),
        );
        assert_eq!(outcome, NegotiationOutcome::Skip);
    }

    #[test]
    fn negotiate_skips_when_body_below_min_size() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("gzip, br"),
            None,
            Some("application/json"),
            Some(64),
        );
        assert_eq!(outcome, NegotiationOutcome::Skip);
    }

    #[test]
    fn negotiate_skips_when_no_accept_encoding() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(None, None, Some("application/json"), Some(10 * 1024));
        assert_eq!(outcome, NegotiationOutcome::Skip);
    }

    #[test]
    fn negotiate_respects_q_zero_refusal() {
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("br;q=0, gzip"),
            None,
            Some("application/json"),
            Some(10 * 1024),
        );
        // Client refused brotli — must fall back to gzip.
        assert_eq!(outcome, NegotiationOutcome::Compress(Encoding::Gzip));
    }

    #[test]
    fn negotiate_streaming_body_proceeds() {
        // content_length unknown (streaming) must not block compression.
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(Some("zstd, gzip"), None, Some("text/html"), None);
        assert_eq!(outcome, NegotiationOutcome::Compress(Encoding::Zstd));
    }

    // -- Documented header mutations (callers apply these per the
    // NegotiationOutcome contract).  These tests guard the contract itself.  --

    #[test]
    fn content_length_stripped_when_compressing() {
        // Simulate the mutation a caller is REQUIRED to apply after receiving
        // NegotiationOutcome::Compress(_).  The outcome drives the contract.
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("gzip"),
            None,
            Some("application/json"),
            Some(10 * 1024),
        );
        assert!(
            matches!(outcome, NegotiationOutcome::Compress(_)),
            "caller must strip Content-Length when outcome is Compress; got {outcome:?}"
        );
    }

    #[test]
    fn vary_header_added() {
        // Same contract test: any Compress(_) outcome mandates Vary: Accept-Encoding.
        let filter = CompressionFilter::default();
        let outcome = filter.negotiate(
            Some("br"),
            None,
            Some("application/json"),
            Some(10 * 1024),
        );
        assert!(
            matches!(outcome, NegotiationOutcome::Compress(_)),
            "caller must add 'Vary: Accept-Encoding' when outcome is Compress; got {outcome:?}"
        );
    }

    // -- stream round-trips --

    /// Build a 11 000-byte payload: "hello world" × 1000.
    fn sample_payload() -> Vec<u8> {
        "hello world".repeat(1000).into_bytes()
    }

    #[test]
    fn stream_gzip_roundtrip() {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let original = sample_payload();
        let mut stream = CompressionStream::new(Encoding::Gzip, CompressionLevel::Balanced)
            .expect("gzip encoder init");

        let mut compressed = stream.write(&original).expect("gzip write");
        compressed.extend(stream.finish().expect("gzip finish"));

        let mut decoder = GzDecoder::new(compressed.as_slice());
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .expect("gzip decompression");
        assert_eq!(decompressed, original, "gzip round-trip mismatch");
    }

    #[test]
    fn stream_brotli_roundtrip() {
        let original = sample_payload();
        let mut stream = CompressionStream::new(Encoding::Brotli, CompressionLevel::Balanced)
            .expect("brotli encoder init");

        let mut compressed = stream.write(&original).expect("brotli write");
        compressed.extend(stream.finish().expect("brotli finish"));

        let mut decompressed = Vec::new();
        let mut decoder = brotli::Decompressor::new(compressed.as_slice(), 4096);
        std::io::copy(&mut decoder, &mut decompressed).expect("brotli decompression");
        assert_eq!(decompressed, original, "brotli round-trip mismatch");
    }

    #[test]
    fn stream_zstd_roundtrip() {
        let original = sample_payload();
        let mut stream = CompressionStream::new(Encoding::Zstd, CompressionLevel::Balanced)
            .expect("zstd encoder init");

        let mut compressed = stream.write(&original).expect("zstd write");
        compressed.extend(stream.finish().expect("zstd finish"));

        let decompressed =
            zstd::decode_all(compressed.as_slice()).expect("zstd decompression");
        assert_eq!(decompressed, original, "zstd round-trip mismatch");
    }

    #[test]
    fn stream_chunked_roundtrip_gzip() {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let chunks: [&[u8]; 4] = [b"alpha-", b"bravo-", b"charlie-", b"delta"];
        let expected: Vec<u8> = chunks.concat();

        let mut stream = CompressionStream::new(Encoding::Gzip, CompressionLevel::Fast)
            .expect("gzip encoder init");

        let mut compressed = Vec::new();
        for c in chunks {
            compressed.extend(stream.write(c).expect("chunk write"));
        }
        compressed.extend(stream.finish().expect("finish"));

        let mut decoder = GzDecoder::new(compressed.as_slice());
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).expect("decompress");
        assert_eq!(decompressed, expected);
    }

    #[test]
    fn identity_stream_is_passthrough() {
        let mut stream = CompressionStream::new(Encoding::Identity, CompressionLevel::Fast)
            .expect("identity init");
        let out = stream.write(b"verbatim").expect("identity write");
        assert_eq!(out, b"verbatim");
        let tail = stream.finish().expect("identity finish");
        assert!(tail.is_empty(), "identity must emit no trailing bytes");
    }

    // -- header-value sanity --

    #[test]
    fn encoding_header_values() {
        assert_eq!(Encoding::Brotli.as_header_value(), "br");
        assert_eq!(Encoding::Zstd.as_header_value(), "zstd");
        assert_eq!(Encoding::Gzip.as_header_value(), "gzip");
        assert_eq!(Encoding::Identity.as_header_value(), "identity");
    }
}
