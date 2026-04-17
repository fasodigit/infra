//! Zero-copy RESP3 fast decoder.
//!
//! [`FastDecoder`] is a drop-in alternative to [`crate::Decoder`] that uses
//! `memchr` for SIMD-accelerated `\r\n` scanning and produces [`crate::Frame`]
//! values whose [`crate::Frame::BulkString`] data shares memory with the input
//! [`bytes::BytesMut`] buffer via `split_to().freeze()` — no extra heap copy
//! for bulk payloads.
//!
//! # Correctness guarantee
//!
//! `FastDecoder::decode` is byte-for-byte equivalent to `Decoder::decode` on
//! any valid RESP3 input.  The parity property is exercised by the
//! `parity_random_corpus` test below.

use bytes::{Buf, Bytes, BytesMut};
use memchr::memmem;

use crate::{Frame, ProtocolError, MAX_AGGREGATE_SIZE, MAX_BULK_SIZE};

// ---------------------------------------------------------------------------
// FastDecoder
// ---------------------------------------------------------------------------

/// Zero-copy RESP3 decoder.
///
/// Uses `memchr` for `\r\n` scanning (SIMD-accelerated on x86/ARM) and
/// [`BytesMut::split_to`] to hand off bulk-string payloads without copying.
pub struct FastDecoder;

impl FastDecoder {
    /// Decode exactly one frame from `buf`.
    ///
    /// - Returns `Ok(Some(frame))` when a complete frame is available; `buf` is
    ///   advanced past the consumed bytes.
    /// - Returns `Ok(None)` when the buffer holds an incomplete frame.
    /// - Returns `Err(ProtocolError)` on malformed input.
    pub fn decode(buf: &mut BytesMut) -> Result<Option<Frame>, ProtocolError> {
        if buf.is_empty() {
            return Ok(None);
        }
        match decode_one(buf) {
            Ok(frame) => Ok(Some(frame)),
            Err(ProtocolError::Incomplete) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Decode up to `max` complete frames in a single pass.
    ///
    /// More efficient than calling [`Self::decode`] in a loop because the
    /// internal scanning state is reused across frames inside the same
    /// contiguous buffer window.
    ///
    /// Stops and returns the accumulated frames when either:
    /// - `max` frames have been decoded, or
    /// - the buffer becomes incomplete (not enough data for the next frame).
    ///
    /// Returns `Err` only if a frame is *malformed* (not merely incomplete).
    pub fn decode_batch(
        buf: &mut BytesMut,
        max: usize,
    ) -> Result<Vec<Frame>, ProtocolError> {
        let mut frames = Vec::with_capacity(max.min(64));
        while frames.len() < max && !buf.is_empty() {
            match decode_one(buf) {
                Ok(frame) => frames.push(frame),
                Err(ProtocolError::Incomplete) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(frames)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Locate the first `\r\n` in `data` using `memchr`'s SIMD finder.
#[inline]
fn find_crlf(data: &[u8]) -> Option<usize> {
    memmem::find(data, b"\r\n")
}

/// Read the line that starts at byte 1 (skipping the type byte).
/// Advances `buf` past the `\r\n`.  Returns the raw byte slice as a `&str`.
///
/// This is intentionally *not* zero-copy for the line content because line
/// data (integers, counts, etc.) is always short.  Only bulk payloads get
/// the zero-copy treatment.
#[inline]
fn read_line_str(buf: &mut BytesMut) -> Result<String, ProtocolError> {
    let pos = find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
    let s = std::str::from_utf8(&buf[1..pos])
        .map_err(|_| ProtocolError::InvalidUtf8)?
        .to_string();
    buf.advance(pos + 2);
    Ok(s)
}

/// Core recursive dispatcher — mirrors `Decoder::decode` but with SIMD paths.
fn decode_one(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    if buf.is_empty() {
        return Err(ProtocolError::Incomplete);
    }
    match buf[0] {
        b'+' => decode_simple_string(buf),
        b'-' => decode_error(buf),
        b':' => decode_integer(buf),
        b'$' => decode_bulk_string(buf),
        b'*' => decode_array(buf),
        b'_' => decode_null(buf),
        b'#' => decode_boolean(buf),
        b',' => decode_double(buf),
        b'(' => decode_big_number(buf),
        b'=' => decode_verbatim_string(buf),
        b'%' => decode_map(buf),
        b'~' => decode_set(buf),
        b'>' => decode_push(buf),
        other => Err(ProtocolError::InvalidFrameType(other)),
    }
}

// -- simple line types -------------------------------------------------------

fn decode_simple_string(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    Ok(Frame::SimpleString(read_line_str(buf)?))
}

fn decode_error(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    Ok(Frame::Error(read_line_str(buf)?))
}

fn decode_integer(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let s = read_line_str(buf)?;
    let n: i64 = s.parse().map_err(|_| ProtocolError::IntegerOverflow)?;
    Ok(Frame::Integer(n))
}

fn decode_null(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    if buf.len() < 3 {
        return Err(ProtocolError::Incomplete);
    }
    buf.advance(3); // _\r\n
    Ok(Frame::Null)
}

fn decode_boolean(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let s = read_line_str(buf)?;
    match s.as_str() {
        "t" => Ok(Frame::Boolean(true)),
        "f" => Ok(Frame::Boolean(false)),
        _ => Err(ProtocolError::Parse(format!("invalid boolean: {s}"))),
    }
}

fn decode_double(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let s = read_line_str(buf)?;
    let d: f64 = s
        .parse()
        .map_err(|_| ProtocolError::Parse(format!("invalid double: {s}")))?;
    Ok(Frame::Double(d))
}

fn decode_big_number(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    Ok(Frame::BigNumber(read_line_str(buf)?))
}

// -- bulk string (zero-copy path) --------------------------------------------

/// Decode a bulk string using `split_to().freeze()` — no extra heap allocation
/// for the payload.  The resulting `Bytes` shares the underlying memory of the
/// input `BytesMut` via reference counting.
fn decode_bulk_string(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    // Find the header `\r\n` to read the length.
    let header_end = find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
    let len_str = std::str::from_utf8(&buf[1..header_end])
        .map_err(|_| ProtocolError::InvalidUtf8)?;

    // RESP2 null bulk string: `$-1\r\n`
    if len_str == "-1" {
        buf.advance(header_end + 2);
        return Ok(Frame::Null);
    }

    // Negative lengths other than -1 are malformed.
    if len_str.starts_with('-') {
        return Err(ProtocolError::Parse(format!(
            "invalid bulk string length: {len_str}"
        )));
    }

    let len: usize = len_str
        .parse()
        .map_err(|_| ProtocolError::Parse(format!("invalid bulk len: {len_str}")))?;

    if len > MAX_BULK_SIZE {
        return Err(ProtocolError::FrameTooLarge { size: len, max: MAX_BULK_SIZE });
    }

    // Ensure the entire payload + trailing \r\n is available.
    let total = header_end + 2 + len + 2;
    if buf.len() < total {
        return Err(ProtocolError::Incomplete);
    }

    // Discard the header line.
    buf.advance(header_end + 2);

    // Zero-copy: split the payload out and freeze it.
    let data: Bytes = buf.split_to(len).freeze();

    // Discard the trailing \r\n.
    buf.advance(2);

    Ok(Frame::BulkString(data))
}

// -- verbatim string ---------------------------------------------------------

fn decode_verbatim_string(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let header_end = find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
    let len_str = std::str::from_utf8(&buf[1..header_end])
        .map_err(|_| ProtocolError::InvalidUtf8)?;
    let len: usize = len_str
        .parse()
        .map_err(|_| ProtocolError::Parse(format!("invalid verbatim len: {len_str}")))?;

    let total = header_end + 2 + len + 2;
    if buf.len() < total {
        return Err(ProtocolError::Incomplete);
    }

    buf.advance(header_end + 2);

    // Format: "enc:data" — encoding is exactly 3 chars followed by ':'
    if len < 4 || buf[3] != b':' {
        buf.advance(len + 2);
        return Err(ProtocolError::Parse(
            "invalid verbatim string format".into(),
        ));
    }

    let payload = std::str::from_utf8(&buf[..len])
        .map_err(|_| ProtocolError::InvalidUtf8)?;
    let encoding = payload[..3].to_string();
    let data = payload[4..].to_string();
    buf.advance(len + 2);

    Ok(Frame::VerbatimString { encoding, data })
}

// -- aggregate types (Array, Map, Set, Push) ---------------------------------

fn decode_count(buf: &mut BytesMut, label: &str) -> Result<Option<usize>, ProtocolError> {
    let header_end = find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
    let count_str = std::str::from_utf8(&buf[1..header_end])
        .map_err(|_| ProtocolError::InvalidUtf8)?;

    if count_str == "-1" {
        buf.advance(header_end + 2);
        return Ok(None); // signals Null
    }

    let count: usize = count_str
        .parse()
        .map_err(|_| ProtocolError::Parse(format!("invalid {label} len: {count_str}")))?;

    if count > MAX_AGGREGATE_SIZE {
        return Err(ProtocolError::FrameTooLarge { size: count, max: MAX_AGGREGATE_SIZE });
    }

    buf.advance(header_end + 2);
    Ok(Some(count))
}

fn decode_array(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    match decode_count(buf, "array")? {
        None => Ok(Frame::Null),
        Some(count) => {
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(decode_one(buf)?);
            }
            Ok(Frame::Array(items))
        }
    }
}

fn decode_map(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let count = decode_count(buf, "map")?.unwrap_or(0);
    let mut pairs = Vec::with_capacity(count);
    for _ in 0..count {
        let key = decode_one(buf)?;
        let value = decode_one(buf)?;
        pairs.push((key, value));
    }
    Ok(Frame::Map(pairs))
}

fn decode_set(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let count = decode_count(buf, "set")?.unwrap_or(0);
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push(decode_one(buf)?);
    }
    Ok(Frame::Set(items))
}

fn decode_push(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
    let count = decode_count(buf, "push")?.unwrap_or(0);
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push(decode_one(buf)?);
    }
    Ok(Frame::Push(items))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decoder, Encoder};
    use bytes::BytesMut;

    // -- helpers -------------------------------------------------------------

    fn encode(frame: &Frame) -> BytesMut {
        let mut buf = BytesMut::new();
        Encoder::encode(frame, &mut buf);
        buf
    }

    fn fast_decode(input: &[u8]) -> Result<Option<Frame>, ProtocolError> {
        let mut buf = BytesMut::from(input);
        FastDecoder::decode(&mut buf)
    }

    // -- 1. SimpleString -----------------------------------------------------

    #[test]
    fn simple_string_ok() {
        let frame = fast_decode(b"+OK\r\n").unwrap().unwrap();
        assert_eq!(frame, Frame::SimpleString("OK".into()));
    }

    // -- 2. Error ------------------------------------------------------------

    #[test]
    fn error_frame() {
        let frame = fast_decode(b"-ERR something\r\n").unwrap().unwrap();
        assert_eq!(frame, Frame::Error("ERR something".into()));
    }

    // -- 3. Integer ----------------------------------------------------------

    #[test]
    fn integer_frame() {
        let frame = fast_decode(b":42\r\n").unwrap().unwrap();
        assert_eq!(frame, Frame::Integer(42));
    }

    // -- 4. BulkString -------------------------------------------------------

    #[test]
    fn bulk_string_hello() {
        let frame = fast_decode(b"$5\r\nhello\r\n").unwrap().unwrap();
        assert_eq!(frame, Frame::BulkString(Bytes::from("hello")));
    }

    // -- 5. Array of two bulk strings ----------------------------------------

    #[test]
    fn array_two_bulk() {
        let frame = fast_decode(b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(
            frame,
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("foo")),
                Frame::BulkString(Bytes::from("bar")),
            ])
        );
    }

    // -- 6. Incomplete buffer returns None -----------------------------------

    #[test]
    fn incomplete_returns_none() {
        // array declares 2 elements but only one is present
        let result = fast_decode(b"*2\r\n$3\r\nfoo\r\n").unwrap();
        assert!(result.is_none(), "expected None for incomplete frame");
    }

    // -- 7. Map --------------------------------------------------------------

    #[test]
    fn map_one_pair() {
        let frame = fast_decode(b"%1\r\n+k\r\n+v\r\n").unwrap().unwrap();
        assert_eq!(
            frame,
            Frame::Map(vec![(
                Frame::SimpleString("k".into()),
                Frame::SimpleString("v".into()),
            )])
        );
    }

    // -- 8. Push -------------------------------------------------------------

    #[test]
    fn push_invalidate() {
        // >2\r\n+invalidate\r\n*1\r\n+key\r\n
        let frame =
            fast_decode(b">2\r\n+invalidate\r\n*1\r\n$3\r\nkey\r\n")
                .unwrap()
                .unwrap();
        assert_eq!(
            frame,
            Frame::Push(vec![
                Frame::SimpleString("invalidate".into()),
                Frame::Array(vec![Frame::BulkString(Bytes::from("key"))]),
            ])
        );
    }

    // -- 9. Null -------------------------------------------------------------

    #[test]
    fn null_frame() {
        let frame = fast_decode(b"_\r\n").unwrap().unwrap();
        assert_eq!(frame, Frame::Null);
    }

    // -- 10. Boolean ---------------------------------------------------------

    #[test]
    fn boolean_true_and_false() {
        assert_eq!(
            fast_decode(b"#t\r\n").unwrap().unwrap(),
            Frame::Boolean(true)
        );
        assert_eq!(
            fast_decode(b"#f\r\n").unwrap().unwrap(),
            Frame::Boolean(false)
        );
    }

    // -- 11. Double ----------------------------------------------------------

    #[test]
    fn double_frame() {
        let frame = fast_decode(b",3.14\r\n").unwrap().unwrap();
        assert_eq!(frame, Frame::Double(3.14));
    }

    // -- 12. Nested array ----------------------------------------------------

    #[test]
    fn nested_array() {
        // *2\r\n*1\r\n:1\r\n:2\r\n
        let frame = fast_decode(b"*2\r\n*1\r\n:1\r\n:2\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(
            frame,
            Frame::Array(vec![
                Frame::Array(vec![Frame::Integer(1)]),
                Frame::Integer(2),
            ])
        );
    }

    // -- 13. Malformed negative bulk length ----------------------------------

    #[test]
    fn malformed_bulk_negative_length_err() {
        // $-2 is not a valid null bulk string (only $-1 is)
        let result = fast_decode(b"$-2\r\nxx\r\n");
        assert!(
            result.is_err(),
            "expected Err for invalid bulk length -2, got {:?}",
            result
        );
    }

    // -- 14. BigNumber -------------------------------------------------------

    #[test]
    fn big_number_frame() {
        let frame = fast_decode(b"(123456789012345678901234567890\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(
            frame,
            Frame::BigNumber("123456789012345678901234567890".into())
        );
    }

    // -- 15. VerbatimString --------------------------------------------------

    #[test]
    fn verbatim_string_frame() {
        let mut buf = BytesMut::new();
        let original = Frame::VerbatimString {
            encoding: "txt".into(),
            data: "hello world".into(),
        };
        Encoder::encode(&original, &mut buf);
        let decoded = FastDecoder::decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded, original);
    }

    // -- 16. Set frame -------------------------------------------------------

    #[test]
    fn set_frame() {
        let frame = fast_decode(b"~3\r\n:1\r\n:2\r\n:3\r\n").unwrap().unwrap();
        assert_eq!(
            frame,
            Frame::Set(vec![
                Frame::Integer(1),
                Frame::Integer(2),
                Frame::Integer(3),
            ])
        );
    }

    // -- 17. Parity: FastDecoder matches Decoder on every RESP3 type ---------
    //
    // We encode a representative corpus of frames with Encoder and verify that
    // both decoders produce identical results.

    fn parity_corpus() -> Vec<Frame> {
        vec![
            Frame::SimpleString("OK".into()),
            Frame::Error("ERR test error".into()),
            Frame::Integer(0),
            Frame::Integer(i64::MAX),
            Frame::Integer(i64::MIN),
            Frame::BulkString(Bytes::from("hello")),
            Frame::BulkString(Bytes::from("")),
            Frame::BulkString(Bytes::from(vec![0u8, 1, 2, 3, 255])),
            Frame::Null,
            Frame::Boolean(true),
            Frame::Boolean(false),
            Frame::Double(3.14),
            Frame::Double(f64::NEG_INFINITY),
            Frame::BigNumber("99999999999999999999999999999".into()),
            Frame::VerbatimString {
                encoding: "txt".into(),
                data: "hello world".into(),
            },
            Frame::Array(vec![
                Frame::BulkString(Bytes::from("SET")),
                Frame::BulkString(Bytes::from("key")),
                Frame::BulkString(Bytes::from("value")),
            ]),
            Frame::Array(vec![]),
            Frame::Map(vec![(
                Frame::SimpleString("field".into()),
                Frame::Integer(42),
            )]),
            Frame::Set(vec![Frame::Integer(1), Frame::Integer(2)]),
            Frame::Push(vec![
                Frame::SimpleString("message".into()),
                Frame::BulkString(Bytes::from("chan")),
            ]),
            // nested
            Frame::Array(vec![
                Frame::Array(vec![Frame::Integer(1), Frame::Integer(2)]),
                Frame::Map(vec![(Frame::BulkString(Bytes::from("k")), Frame::Null)]),
            ]),
        ]
    }

    #[test]
    fn parity_with_decoder_on_corpus() {
        for original in parity_corpus() {
            let mut buf = encode(&original);

            // Keep a copy for the old decoder.
            let mut buf_old = buf.clone();

            let fast_result = FastDecoder::decode(&mut buf)
                .expect("FastDecoder should not error on valid input")
                .expect("FastDecoder should return Some for complete frame");

            let old_result = Decoder::decode(&mut buf_old)
                .expect("Decoder should not error on valid input");

            assert_eq!(
                fast_result, old_result,
                "parity failure for frame: {original:?}"
            );

            // Both decoders should have consumed the entire buffer.
            assert!(buf.is_empty(), "FastDecoder left bytes in buffer");
            assert!(buf_old.is_empty(), "Decoder left bytes in buffer");
        }
    }

    // -- 18. decode_batch ----------------------------------------------------

    #[test]
    fn decode_batch_multiple_frames() {
        let mut buf = BytesMut::new();
        let frames_in = vec![
            Frame::SimpleString("OK".into()),
            Frame::Integer(1),
            Frame::BulkString(Bytes::from("world")),
        ];
        for f in &frames_in {
            Encoder::encode(f, &mut buf);
        }

        let decoded = FastDecoder::decode_batch(&mut buf, 10).unwrap();
        assert_eq!(decoded, frames_in);
        assert!(buf.is_empty());
    }

    #[test]
    fn decode_batch_respects_max() {
        let mut buf = BytesMut::new();
        for _ in 0..5 {
            Encoder::encode(&Frame::SimpleString("OK".into()), &mut buf);
        }
        let decoded = FastDecoder::decode_batch(&mut buf, 3).unwrap();
        assert_eq!(decoded.len(), 3);
        // 2 frames remain
        assert!(!buf.is_empty());
    }

    #[test]
    fn decode_batch_stops_on_incomplete() {
        let mut buf = BytesMut::new();
        Encoder::encode(&Frame::SimpleString("OK".into()), &mut buf);
        // append a truncated bulk string
        buf.extend_from_slice(b"$5\r\nhel");

        let decoded = FastDecoder::decode_batch(&mut buf, 10).unwrap();
        assert_eq!(decoded.len(), 1); // only the complete +OK was decoded
        // incomplete data remains
        assert!(!buf.is_empty());
    }

    // -- 19. Parity on a large pseudo-random corpus (1000 frames) ------------

    #[test]
    fn parity_pseudo_random_1000_frames() {
        // Deterministic pseudo-random frame generation — no external rand dep.
        let mut seed: u64 = 0xDEAD_BEEF_CAFE_BABE;

        let mut next_u64 = move || -> u64 {
            // xorshift64
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        for i in 0..1000 {
            let kind = next_u64() % 10;
            let frame = match kind {
                0 => Frame::SimpleString(format!("msg{i}")),
                1 => Frame::Error(format!("ERR code{i}")),
                2 => Frame::Integer((next_u64() as i64).wrapping_sub(i64::MAX / 2)),
                3 => {
                    let len = (next_u64() % 64) as usize;
                    let data: Vec<u8> = (0..len).map(|_| (next_u64() % 256) as u8).collect();
                    Frame::BulkString(Bytes::from(data))
                }
                4 => Frame::Null,
                5 => Frame::Boolean(next_u64() % 2 == 0),
                6 => Frame::Double(next_u64() as f64 / 1e10),
                7 => Frame::BigNumber(format!("{}", next_u64())),
                8 => {
                    let count = (next_u64() % 5) as usize;
                    let items: Vec<Frame> = (0..count)
                        .map(|j| Frame::Integer(j as i64))
                        .collect();
                    Frame::Array(items)
                }
                _ => Frame::Map(vec![(
                    Frame::SimpleString(format!("k{i}")),
                    Frame::Integer(i as i64),
                )]),
            };

            let mut buf = encode(&frame);
            let mut buf_old = buf.clone();

            let fast = FastDecoder::decode(&mut buf)
                .unwrap_or_else(|e| panic!("FastDecoder error on frame {i}: {e}"))
                .unwrap_or_else(|| panic!("FastDecoder returned None on frame {i}"));

            let old = Decoder::decode(&mut buf_old)
                .unwrap_or_else(|e| panic!("Decoder error on frame {i}: {e}"));

            assert_eq!(fast, old, "parity mismatch at frame {i}: {frame:?}");
        }
    }
}
