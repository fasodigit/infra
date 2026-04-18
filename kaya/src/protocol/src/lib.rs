//! KAYA RESP3+ Protocol Parser and Serializer
//!
//! Implements the RESP3 wire protocol (port 6380) for compatibility with
//! RESP3-compatible clients, plus KAYA-specific extensions.

pub mod fast_decoder;
pub use fast_decoder::FastDecoder;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Frame size limits (SecFinding-DOS-FRAME-SIZE)
// ---------------------------------------------------------------------------

/// Maximum number of elements in a single Array/Map/Set/Push frame.
///
/// WHY: An attacker can send `*65537\r\n` to force a 65 537-element
/// `Vec::with_capacity`, exhausting heap memory before a single byte of
/// payload arrives. 65 536 elements cover all legitimate command sizes.
pub const MAX_AGGREGATE_SIZE: usize = 65_536;

/// Maximum byte length of a single BulkString payload.
///
/// WHY: Same OOM amplification risk as aggregate frames. 512 MiB matches the
/// RESP3 specification maximum for a single bulk payload.
pub const MAX_BULK_SIZE: usize = 512 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("incomplete frame, need more data")]
    Incomplete,

    #[error("invalid frame type byte: {0:#x}")]
    InvalidFrameType(u8),

    #[error("protocol parse error: {0}")]
    Parse(String),

    #[error("invalid utf-8 in frame")]
    InvalidUtf8,

    #[error("integer overflow")]
    IntegerOverflow,

    #[error("frame too large: {size} bytes (max {max})")]
    FrameTooLarge { size: usize, max: usize },
}

// ---------------------------------------------------------------------------
// Protocol version (HELLO negotiation)
// ---------------------------------------------------------------------------

/// Protocol version negotiated per-connection via the HELLO command.
///
/// `Resp2` is the default (all clients that do not send HELLO start here).
/// `Resp3` is negotiated when a client sends `HELLO 3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtocolVersion {
    /// RESP2 — legacy mode. All aggregate responses are Array.
    #[default]
    Resp2,
    /// RESP3 — enables Map, Set, Push, Double, Boolean, Null, BigNumber,
    /// VerbatimString frames.
    Resp3,
}

// ---------------------------------------------------------------------------
// RESP3 Frame types
// ---------------------------------------------------------------------------

/// A parsed RESP3 frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// Simple string: `+OK\r\n`
    SimpleString(String),

    /// Simple error: `-ERR message\r\n`
    Error(String),

    /// Integer: `:42\r\n`
    Integer(i64),

    /// Bulk string: `$5\r\nhello\r\n`
    BulkString(Bytes),

    /// Array: `*2\r\n...`
    Array(Vec<Frame>),

    /// Null: `_\r\n`
    Null,

    /// Boolean (RESP3): `#t\r\n` / `#f\r\n`
    Boolean(bool),

    /// Double (RESP3): `,3.14\r\n`
    Double(f64),

    /// Big number (RESP3): `(12345...\r\n`
    BigNumber(String),

    /// Verbatim string (RESP3): `=15\r\ntxt:hello world\r\n`
    VerbatimString { encoding: String, data: String },

    /// Map (RESP3): `%2\r\n...`
    Map(Vec<(Frame, Frame)>),

    /// Set (RESP3): `~3\r\n...`
    Set(Vec<Frame>),

    /// Push (RESP3): `>2\r\n...`
    Push(Vec<Frame>),
}

impl Frame {
    /// Convenience: build a BulkString from `&str`.
    pub fn bulk<S: Into<Bytes>>(s: S) -> Self {
        Frame::BulkString(s.into())
    }

    /// Convenience: build an OK simple string.
    pub fn ok() -> Self {
        Frame::SimpleString("OK".into())
    }

    /// Convenience: build an error frame.
    pub fn err<S: Into<String>>(msg: S) -> Self {
        Frame::Error(msg.into())
    }

    /// Check if this frame is an error.
    pub fn is_error(&self) -> bool {
        matches!(self, Frame::Error(_))
    }

    /// Try to interpret this frame as a UTF-8 string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Frame::SimpleString(s) => Some(s.as_str()),
            Frame::BulkString(b) => std::str::from_utf8(b).ok(),
            _ => None,
        }
    }

    /// Try to interpret this frame as an integer.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Frame::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Extract array elements, if this is an Array frame.
    pub fn into_array(self) -> Option<Vec<Frame>> {
        match self {
            Frame::Array(v) => Some(v),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

/// Encodes [`Frame`] values into the RESP3 wire format.
pub struct Encoder;

impl Encoder {
    /// Write a frame into the provided buffer using native RESP3 encoding.
    ///
    /// All frame types are encoded verbatim. [`Frame::Null`] emits `_\r\n`.
    /// Use [`encode_versioned`] when the connection protocol version matters
    /// (i.e. for outbound server responses that may go to RESP2 clients).
    pub fn encode(frame: &Frame, buf: &mut BytesMut) {
        Self::encode_versioned(frame, ProtocolVersion::Resp3, buf);
    }

    /// Write a frame with awareness of the negotiated [`ProtocolVersion`].
    ///
    /// This is the **correct** method to use on outbound server responses.
    /// The encoding differs between RESP2 and RESP3 for the following types:
    ///
    /// | Frame            | RESP2 wire              | RESP3 wire           |
    /// |------------------|-------------------------|----------------------|
    /// | `Null`           | `$-1\r\n`               | `_\r\n`              |
    /// | `Boolean(true)`  | `:1\r\n`                | `#t\r\n`             |
    /// | `Boolean(false)` | `:0\r\n`                | `#f\r\n`             |
    /// | `Double`         | bulk string repr        | `,<val>\r\n`         |
    /// | `BigNumber`      | bulk string repr        | `(<val>\r\n`         |
    /// | `VerbatimString` | bulk string (data only) | `=<n>\r\n<enc:data>` |
    /// | `Map`            | flat Array `[k,v,…]`    | `%<n>\r\n…`          |
    /// | `Set`            | Array                   | `~<n>\r\n…`          |
    /// | `Push`           | Array                   | `><n>\r\n…`          |
    ///
    /// `Array`, `BulkString`, `SimpleString`, `Error`, `Integer` are identical
    /// in both protocols. An empty `Array` always emits `*0\r\n`.
    #[inline]
    pub fn encode_versioned(frame: &Frame, proto: ProtocolVersion, buf: &mut BytesMut) {
        match frame {
            Frame::SimpleString(s) => {
                buf.put_u8(b'+');
                buf.put_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            Frame::Error(s) => {
                buf.put_u8(b'-');
                buf.put_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            Frame::Integer(n) => {
                buf.put_u8(b':');
                buf.put_slice(n.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
            Frame::BulkString(data) => {
                buf.put_u8(b'$');
                buf.put_slice(data.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.put_slice(data);
                buf.put_slice(b"\r\n");
            }
            Frame::Array(items) => {
                buf.put_u8(b'*');
                buf.put_slice(items.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for item in items {
                    Self::encode_versioned(item, proto, buf);
                }
            }
            Frame::Null => match proto {
                // RESP3: unified null type `_\r\n`
                ProtocolVersion::Resp3 => {
                    buf.put_slice(b"_\r\n");
                }
                // RESP2: null bulk string `$-1\r\n`
                // WHY: Lettuce and other RESP2 clients interpret `_\r\n` as an
                // unknown frame type (0x5f) and throw a DataAccessException.
                // RESP2 has no standalone null type; the idiomatic encoding is
                // the null bulk string `$-1\r\n`.
                ProtocolVersion::Resp2 => {
                    buf.put_slice(b"$-1\r\n");
                }
            },
            Frame::Boolean(b) => match proto {
                ProtocolVersion::Resp3 => {
                    buf.put_u8(b'#');
                    buf.put_u8(if *b { b't' } else { b'f' });
                    buf.put_slice(b"\r\n");
                }
                // RESP2: emulate with integer 1 / 0 (idiomatic Redis behaviour).
                ProtocolVersion::Resp2 => {
                    buf.put_u8(b':');
                    buf.put_u8(if *b { b'1' } else { b'0' });
                    buf.put_slice(b"\r\n");
                }
            },
            Frame::Double(d) => match proto {
                ProtocolVersion::Resp3 => {
                    buf.put_u8(b',');
                    buf.put_slice(d.to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                }
                // RESP2: encode as bulk string representation.
                ProtocolVersion::Resp2 => {
                    let s = d.to_string();
                    buf.put_u8(b'$');
                    buf.put_slice(s.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    buf.put_slice(s.as_bytes());
                    buf.put_slice(b"\r\n");
                }
            },
            Frame::BigNumber(s) => match proto {
                ProtocolVersion::Resp3 => {
                    buf.put_u8(b'(');
                    buf.put_slice(s.as_bytes());
                    buf.put_slice(b"\r\n");
                }
                // RESP2: encode as bulk string.
                ProtocolVersion::Resp2 => {
                    buf.put_u8(b'$');
                    buf.put_slice(s.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    buf.put_slice(s.as_bytes());
                    buf.put_slice(b"\r\n");
                }
            },
            Frame::VerbatimString { encoding, data } => match proto {
                ProtocolVersion::Resp3 => {
                    let payload = format!("{encoding}:{data}");
                    buf.put_u8(b'=');
                    buf.put_slice(payload.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    buf.put_slice(payload.as_bytes());
                    buf.put_slice(b"\r\n");
                }
                // RESP2: strip the encoding prefix, return data as bulk string.
                ProtocolVersion::Resp2 => {
                    buf.put_u8(b'$');
                    buf.put_slice(data.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    buf.put_slice(data.as_bytes());
                    buf.put_slice(b"\r\n");
                }
            },
            Frame::Map(pairs) => match proto {
                ProtocolVersion::Resp3 => {
                    buf.put_u8(b'%');
                    buf.put_slice(pairs.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for (k, v) in pairs {
                        Self::encode_versioned(k, proto, buf);
                        Self::encode_versioned(v, proto, buf);
                    }
                }
                // RESP2: flatten map into a `*N*2` array of interleaved k/v.
                ProtocolVersion::Resp2 => {
                    buf.put_u8(b'*');
                    buf.put_slice((pairs.len() * 2).to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for (k, v) in pairs {
                        Self::encode_versioned(k, proto, buf);
                        Self::encode_versioned(v, proto, buf);
                    }
                }
            },
            Frame::Set(items) => match proto {
                ProtocolVersion::Resp3 => {
                    buf.put_u8(b'~');
                    buf.put_slice(items.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for item in items {
                        Self::encode_versioned(item, proto, buf);
                    }
                }
                // RESP2: encode as array.
                ProtocolVersion::Resp2 => {
                    buf.put_u8(b'*');
                    buf.put_slice(items.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for item in items {
                        Self::encode_versioned(item, proto, buf);
                    }
                }
            },
            Frame::Push(items) => match proto {
                ProtocolVersion::Resp3 => {
                    buf.put_u8(b'>');
                    buf.put_slice(items.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for item in items {
                        Self::encode_versioned(item, proto, buf);
                    }
                }
                // RESP2: encode as array.
                ProtocolVersion::Resp2 => {
                    buf.put_u8(b'*');
                    buf.put_slice(items.len().to_string().as_bytes());
                    buf.put_slice(b"\r\n");
                    for item in items {
                        Self::encode_versioned(item, proto, buf);
                    }
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/// Decodes RESP3 frames from a byte buffer.
pub struct Decoder;

impl Decoder {
    /// Attempt to parse a single frame from `buf`.
    ///
    /// On success, advances `buf` past the consumed bytes and returns the frame.
    /// Returns `Err(ProtocolError::Incomplete)` if more data is needed.
    pub fn decode(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        if buf.is_empty() {
            return Err(ProtocolError::Incomplete);
        }

        match buf[0] {
            b'+' => Self::decode_simple_string(buf),
            b'-' => Self::decode_error(buf),
            b':' => Self::decode_integer(buf),
            b'$' => Self::decode_bulk_string(buf),
            b'*' => Self::decode_array(buf),
            b'_' => Self::decode_null(buf),
            b'#' => Self::decode_boolean(buf),
            b',' => Self::decode_double(buf),
            b'(' => Self::decode_big_number(buf),
            b'=' => Self::decode_verbatim_string(buf),
            b'%' => Self::decode_map(buf),
            b'~' => Self::decode_set(buf),
            b'>' => Self::decode_push(buf),
            other => Err(ProtocolError::InvalidFrameType(other)),
        }
    }

    fn find_crlf(buf: &[u8]) -> Option<usize> {
        buf.windows(2).position(|w| w == b"\r\n")
    }

    fn read_line(buf: &mut BytesMut) -> Result<String, ProtocolError> {
        let pos = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
        // skip type byte
        let line = &buf[1..pos];
        let s = std::str::from_utf8(line)
            .map_err(|_| ProtocolError::InvalidUtf8)?
            .to_string();
        buf.advance(pos + 2);
        Ok(s)
    }

    fn decode_simple_string(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        Ok(Frame::SimpleString(Self::read_line(buf)?))
    }

    fn decode_error(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        Ok(Frame::Error(Self::read_line(buf)?))
    }

    fn decode_integer(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let s = Self::read_line(buf)?;
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
        let s = Self::read_line(buf)?;
        match s.as_str() {
            "t" => Ok(Frame::Boolean(true)),
            "f" => Ok(Frame::Boolean(false)),
            _ => Err(ProtocolError::Parse(format!("invalid boolean: {s}"))),
        }
    }

    fn decode_double(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let s = Self::read_line(buf)?;
        let d: f64 = s
            .parse()
            .map_err(|_| ProtocolError::Parse(format!("invalid double: {s}")))?;
        Ok(Frame::Double(d))
    }

    fn decode_bulk_string(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let header_end = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
        let len_str = std::str::from_utf8(&buf[1..header_end])
            .map_err(|_| ProtocolError::InvalidUtf8)?;

        if len_str == "-1" {
            buf.advance(header_end + 2);
            return Ok(Frame::Null);
        }

        let len: usize = len_str
            .parse()
            .map_err(|_| ProtocolError::Parse(format!("invalid bulk len: {len_str}")))?;

        if len > MAX_BULK_SIZE {
            return Err(ProtocolError::FrameTooLarge { size: len, max: MAX_BULK_SIZE });
        }

        let total = header_end + 2 + len + 2;
        if buf.len() < total {
            return Err(ProtocolError::Incomplete);
        }

        buf.advance(header_end + 2);
        let data = Bytes::copy_from_slice(&buf[..len]);
        buf.advance(len + 2);
        Ok(Frame::BulkString(data))
    }

    fn decode_array(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let header_end = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
        let count_str = std::str::from_utf8(&buf[1..header_end])
            .map_err(|_| ProtocolError::InvalidUtf8)?;

        if count_str == "-1" {
            buf.advance(header_end + 2);
            return Ok(Frame::Null);
        }

        let count: usize = count_str
            .parse()
            .map_err(|_| ProtocolError::Parse(format!("invalid array len: {count_str}")))?;

        if count > MAX_AGGREGATE_SIZE {
            return Err(ProtocolError::FrameTooLarge { size: count, max: MAX_AGGREGATE_SIZE });
        }

        buf.advance(header_end + 2);

        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(Self::decode(buf)?);
        }

        Ok(Frame::Array(items))
    }

    fn decode_big_number(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        Ok(Frame::BigNumber(Self::read_line(buf)?))
    }

    fn decode_verbatim_string(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let header_end = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
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
        let payload = std::str::from_utf8(&buf[..len])
            .map_err(|_| ProtocolError::InvalidUtf8)?;

        // Format: "enc:data" where enc is exactly 3 chars
        if payload.len() < 4 || payload.as_bytes()[3] != b':' {
            buf.advance(len + 2);
            return Err(ProtocolError::Parse("invalid verbatim string format".into()));
        }
        let encoding = payload[..3].to_string();
        let data = payload[4..].to_string();
        buf.advance(len + 2);

        Ok(Frame::VerbatimString { encoding, data })
    }

    fn decode_map(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let header_end = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
        let count_str = std::str::from_utf8(&buf[1..header_end])
            .map_err(|_| ProtocolError::InvalidUtf8)?;
        let count: usize = count_str
            .parse()
            .map_err(|_| ProtocolError::Parse(format!("invalid map len: {count_str}")))?;

        if count > MAX_AGGREGATE_SIZE {
            return Err(ProtocolError::FrameTooLarge { size: count, max: MAX_AGGREGATE_SIZE });
        }

        buf.advance(header_end + 2);

        let mut pairs = Vec::with_capacity(count);
        for _ in 0..count {
            let key = Self::decode(buf)?;
            let value = Self::decode(buf)?;
            pairs.push((key, value));
        }

        Ok(Frame::Map(pairs))
    }

    fn decode_set(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let header_end = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
        let count_str = std::str::from_utf8(&buf[1..header_end])
            .map_err(|_| ProtocolError::InvalidUtf8)?;
        let count: usize = count_str
            .parse()
            .map_err(|_| ProtocolError::Parse(format!("invalid set len: {count_str}")))?;

        if count > MAX_AGGREGATE_SIZE {
            return Err(ProtocolError::FrameTooLarge { size: count, max: MAX_AGGREGATE_SIZE });
        }

        buf.advance(header_end + 2);

        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(Self::decode(buf)?);
        }

        Ok(Frame::Set(items))
    }

    fn decode_push(buf: &mut BytesMut) -> Result<Frame, ProtocolError> {
        let header_end = Self::find_crlf(buf).ok_or(ProtocolError::Incomplete)?;
        let count_str = std::str::from_utf8(&buf[1..header_end])
            .map_err(|_| ProtocolError::InvalidUtf8)?;
        let count: usize = count_str
            .parse()
            .map_err(|_| ProtocolError::Parse(format!("invalid push len: {count_str}")))?;

        if count > MAX_AGGREGATE_SIZE {
            return Err(ProtocolError::FrameTooLarge { size: count, max: MAX_AGGREGATE_SIZE });
        }

        buf.advance(header_end + 2);

        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(Self::decode(buf)?);
        }

        Ok(Frame::Push(items))
    }

    /// Decode all complete frames from the buffer (for pipelining support).
    /// Returns the list of successfully decoded frames. The buffer is advanced
    /// past consumed bytes; any remaining incomplete data stays in the buffer.
    pub fn decode_all(buf: &mut BytesMut) -> Vec<Frame> {
        let mut frames = Vec::new();
        loop {
            match Self::decode(buf) {
                Ok(frame) => frames.push(frame),
                Err(ProtocolError::Incomplete) => break,
                Err(_) => break,
            }
        }
        frames
    }
}

// ---------------------------------------------------------------------------
// Command parsing helper
// ---------------------------------------------------------------------------

/// A parsed client command (e.g. `["SET", "key", "value"]`).
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub args: Vec<Bytes>,
}

impl Command {
    /// Parse a frame into a Command.
    ///
    /// Accepted frame types:
    /// - [`Frame::Array`] — the standard RESP2/RESP3 command encoding (`*`).
    /// - [`Frame::Push`] — RESP3 bidirectional push frame (`>`). Some RESP3
    ///   clients (e.g. Lettuce 6.x in RESP3 mode) send pipelined commands
    ///   wrapped in a Push frame during MULTI/EXEC or after `HELLO 3`. The
    ///   wire structure is identical to Array so we decode it identically.
    /// - [`Frame::BulkString`] — bare bulk string used by some clients as a
    ///   zero-arg command shorthand (e.g. `$4\r\nPING\r\n` for PING). Treated
    ///   as a command with no arguments. Lettuce 6.x sends these during the
    ///   initial connection health-check before HELLO negotiation.
    /// - [`Frame::SimpleString`] — inline-style single-token command. Treated
    ///   the same as a BulkString command (no args).
    ///
    /// Returns `Err(ProtocolError::Parse("expected array frame"))` for any
    /// other frame type (retains the original error text for client compat).
    pub fn from_frame(frame: Frame) -> Result<Self, ProtocolError> {
        // Fast path: a bare BulkString or SimpleString is a zero-arg command.
        // Some clients (Lettuce during initial health-check) send these.
        match &frame {
            Frame::BulkString(b) => {
                let name = std::str::from_utf8(b)
                    .map(|s| s.to_ascii_uppercase())
                    .map_err(|_| ProtocolError::InvalidUtf8)?;
                return Ok(Self { name, args: vec![] });
            }
            Frame::SimpleString(s) => {
                return Ok(Self {
                    name: s.to_ascii_uppercase(),
                    args: vec![],
                });
            }
            _ => {}
        }

        // Extract the element list from Array or Push — both are a counted
        // sequence of frames on the wire and carry the same semantics here.
        let parts: Vec<Frame> = match frame {
            Frame::Array(v) => v,
            Frame::Push(v) => v,
            _ => return Err(ProtocolError::Parse("expected array frame".into())),
        };

        if parts.is_empty() {
            return Err(ProtocolError::Parse("empty command".into()));
        }

        let mut iter = parts.into_iter();
        // Safety: we checked is_empty above.
        let name_frame = iter.next().unwrap();
        let name = match name_frame {
            Frame::BulkString(b) => {
                // Zero-copy: interpret bytes in-place as UTF-8.
                std::str::from_utf8(&b)
                    .map(|s| s.to_ascii_uppercase())
                    .map_err(|_| ProtocolError::InvalidUtf8)?
            }
            Frame::SimpleString(s) => s.to_ascii_uppercase(),
            _ => return Err(ProtocolError::Parse("command name must be string".into())),
        };

        let mut args = Vec::with_capacity(iter.len());
        for frame in iter {
            match frame {
                Frame::BulkString(b) => args.push(b),
                Frame::SimpleString(s) => args.push(Bytes::from(s.into_bytes())),
                Frame::Integer(n) => args.push(Bytes::from(n.to_string().into_bytes())),
                _ => return Err(ProtocolError::Parse("unsupported arg type".into())),
            }
        }

        Ok(Self { name, args })
    }

    /// Number of arguments (excluding the command name).
    pub fn arg_count(&self) -> usize {
        self.args.len()
    }

    /// Get argument at index as a UTF-8 string.
    pub fn arg_str(&self, idx: usize) -> Result<&str, ProtocolError> {
        self.args
            .get(idx)
            .ok_or_else(|| ProtocolError::Parse(format!("missing argument at index {idx}")))
            .and_then(|b| std::str::from_utf8(b).map_err(|_| ProtocolError::InvalidUtf8))
    }

    /// Get argument at index as raw bytes.
    pub fn arg_bytes(&self, idx: usize) -> Result<&Bytes, ProtocolError> {
        self.args
            .get(idx)
            .ok_or_else(|| ProtocolError::Parse(format!("missing argument at index {idx}")))
    }

    /// Get argument at index as i64.
    pub fn arg_i64(&self, idx: usize) -> Result<i64, ProtocolError> {
        let s = self.arg_str(idx)?;
        s.parse()
            .map_err(|_| ProtocolError::Parse(format!("argument {idx} is not an integer")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_simple_string() {
        let frame = Frame::SimpleString("OK".into());
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_bulk_string() {
        let frame = Frame::bulk(Bytes::from("hello"));
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_array() {
        let frame = Frame::Array(vec![
            Frame::bulk(Bytes::from("SET")),
            Frame::bulk(Bytes::from("key")),
            Frame::bulk(Bytes::from("value")),
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn parse_command() {
        let frame = Frame::Array(vec![
            Frame::bulk(Bytes::from("set")),
            Frame::bulk(Bytes::from("mykey")),
            Frame::bulk(Bytes::from("myvalue")),
        ]);
        let cmd = Command::from_frame(frame).unwrap();
        assert_eq!(cmd.name, "SET");
        assert_eq!(cmd.arg_count(), 2);
        assert_eq!(cmd.arg_str(0).unwrap(), "mykey");
        assert_eq!(cmd.arg_str(1).unwrap(), "myvalue");
    }

    #[test]
    fn encode_decode_null() {
        let frame = Frame::Null;
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_integer() {
        let frame = Frame::Integer(-42);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_boolean() {
        for val in [true, false] {
            let frame = Frame::Boolean(val);
            let mut buf = BytesMut::new();
            Encoder::encode(&frame, &mut buf);
            let decoded = Decoder::decode(&mut buf).unwrap();
            assert_eq!(decoded, frame);
        }
    }

    #[test]
    fn encode_decode_double() {
        let frame = Frame::Double(3.14);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_error() {
        let frame = Frame::Error("ERR something went wrong".into());
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_big_number() {
        let frame = Frame::BigNumber("123456789012345678901234567890".into());
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_verbatim_string() {
        let frame = Frame::VerbatimString {
            encoding: "txt".into(),
            data: "hello world".into(),
        };
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_map() {
        let frame = Frame::Map(vec![
            (Frame::bulk(Bytes::from("key1")), Frame::Integer(1)),
            (Frame::bulk(Bytes::from("key2")), Frame::Integer(2)),
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_set_frame() {
        let frame = Frame::Set(vec![
            Frame::Integer(1),
            Frame::Integer(2),
            Frame::Integer(3),
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn encode_decode_push() {
        let frame = Frame::Push(vec![
            Frame::bulk(Bytes::from("message")),
            Frame::bulk(Bytes::from("channel1")),
            Frame::bulk(Bytes::from("hello")),
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let decoded = Decoder::decode(&mut buf).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn pipeline_decode_multiple_frames() {
        let mut buf = BytesMut::new();
        // Encode three commands into one buffer (pipelining)
        let cmd1 = Frame::Array(vec![
            Frame::bulk(Bytes::from("PING")),
        ]);
        let cmd2 = Frame::Array(vec![
            Frame::bulk(Bytes::from("SET")),
            Frame::bulk(Bytes::from("k")),
            Frame::bulk(Bytes::from("v")),
        ]);
        let cmd3 = Frame::Array(vec![
            Frame::bulk(Bytes::from("GET")),
            Frame::bulk(Bytes::from("k")),
        ]);
        Encoder::encode(&cmd1, &mut buf);
        Encoder::encode(&cmd2, &mut buf);
        Encoder::encode(&cmd3, &mut buf);

        let frames = Decoder::decode_all(&mut buf);
        assert_eq!(frames.len(), 3);
        assert!(buf.is_empty());
    }

    #[test]
    fn decode_incomplete_returns_error() {
        let mut buf = BytesMut::from(&b"+OK\r"[..]);  // missing \n
        let result = Decoder::decode(&mut buf);
        assert!(matches!(result, Err(ProtocolError::Incomplete)));
    }

    #[test]
    fn decode_bulk_null() {
        let mut buf = BytesMut::from(&b"$-1\r\n"[..]);
        let frame = Decoder::decode(&mut buf).unwrap();
        assert_eq!(frame, Frame::Null);
    }

    #[test]
    fn decode_array_null() {
        let mut buf = BytesMut::from(&b"*-1\r\n"[..]);
        let frame = Decoder::decode(&mut buf).unwrap();
        assert_eq!(frame, Frame::Null);
    }

    // -----------------------------------------------------------------------
    // Bug B regression tests — RESP3 Push frame as command (Lettuce 6.x)
    // -----------------------------------------------------------------------

    /// Lettuce in RESP3 mode can send pipelined commands as Push frames (`>`).
    /// `Command::from_frame` must accept these without emitting "expected array
    /// frame" errors.
    #[test]
    fn push_frame_parses_as_command() {
        let push = Frame::Push(vec![
            Frame::BulkString(Bytes::from("PING")),
        ]);
        let cmd = Command::from_frame(push).expect("Push frame must be accepted as a command");
        assert_eq!(cmd.name, "PING");
        assert_eq!(cmd.arg_count(), 0);
    }

    #[test]
    fn push_frame_with_args_parses_as_command() {
        let push = Frame::Push(vec![
            Frame::BulkString(Bytes::from("SET")),
            Frame::BulkString(Bytes::from("mykey")),
            Frame::BulkString(Bytes::from("myval")),
        ]);
        let cmd = Command::from_frame(push).expect("Push frame with args must parse");
        assert_eq!(cmd.name, "SET");
        assert_eq!(cmd.arg_count(), 2);
        assert_eq!(cmd.arg_str(0).unwrap(), "mykey");
        assert_eq!(cmd.arg_str(1).unwrap(), "myval");
    }

    #[test]
    fn non_array_non_push_frame_returns_error() {
        let integer = Frame::Integer(42);
        let result = Command::from_frame(integer);
        assert!(
            matches!(&result, Err(ProtocolError::Parse(msg)) if msg == "expected array frame"),
            "non-Array/Push frame must return 'expected array frame', got: {result:?}"
        );
    }

    #[test]
    fn pipeline_with_push_frame_command_decodes_correctly() {
        // Simulate a Lettuce RESP3 pipeline: HELLO 3 (Array) + GET key (Push)
        let mut buf = BytesMut::new();
        // Standard array command
        let array_cmd = Frame::Array(vec![
            Frame::BulkString(Bytes::from("HELLO")),
            Frame::BulkString(Bytes::from("3")),
        ]);
        // Push-wrapped command (RESP3 bidirectional)
        let push_cmd = Frame::Push(vec![
            Frame::BulkString(Bytes::from("GET")),
            Frame::BulkString(Bytes::from("session:abc")),
        ]);
        Encoder::encode(&array_cmd, &mut buf);
        Encoder::encode(&push_cmd, &mut buf);

        let frames = Decoder::decode_all(&mut buf);
        assert_eq!(frames.len(), 2, "both frames must decode");

        // Both frames must convert to valid commands.
        let cmd1 = Command::from_frame(frames[0].clone()).expect("Array → Command");
        let cmd2 = Command::from_frame(frames[1].clone()).expect("Push  → Command");
        assert_eq!(cmd1.name, "HELLO");
        assert_eq!(cmd2.name, "GET");
        assert_eq!(cmd2.arg_str(0).unwrap(), "session:abc");
    }

    // -- DoS frame-size limit tests (SecFinding-DOS-FRAME-SIZE) ---------------

    #[test]
    fn oversized_array_count_is_rejected() {
        // Declare count = MAX_AGGREGATE_SIZE + 1 without any payload data.
        let oversized = format!("*{}\r\n", MAX_AGGREGATE_SIZE + 1);
        let mut buf = BytesMut::from(oversized.as_bytes());
        let result = Decoder::decode(&mut buf);
        assert!(
            matches!(result, Err(ProtocolError::FrameTooLarge { .. })),
            "array count above limit must be rejected, got: {result:?}"
        );
    }

    #[test]
    fn oversized_map_count_is_rejected() {
        let oversized = format!("%{}\r\n", MAX_AGGREGATE_SIZE + 1);
        let mut buf = BytesMut::from(oversized.as_bytes());
        let result = Decoder::decode(&mut buf);
        assert!(
            matches!(result, Err(ProtocolError::FrameTooLarge { .. })),
            "map count above limit must be rejected"
        );
    }

    #[test]
    fn max_aggregate_size_is_accepted() {
        // Exactly at the limit must still yield Incomplete (needs elements),
        // NOT FrameTooLarge.
        let at_limit = format!("*{}\r\n", MAX_AGGREGATE_SIZE);
        let mut buf = BytesMut::from(at_limit.as_bytes());
        let result = Decoder::decode(&mut buf);
        assert!(
            matches!(result, Err(ProtocolError::Incomplete)),
            "count exactly at limit should be Incomplete, not error: {result:?}"
        );
    }

    // -- BulkString / SimpleString zero-arg command parsing -------------------

    #[test]
    fn bulk_string_frame_parses_as_zero_arg_command() {
        // Lettuce 6.x sends bare BulkStrings during initial health checks
        // (before HELLO negotiation). KAYA must accept them.
        let frame = Frame::BulkString(Bytes::from("PING"));
        let cmd = Command::from_frame(frame).expect("BulkString → Command");
        assert_eq!(cmd.name, "PING");
        assert_eq!(cmd.arg_count(), 0);
    }

    #[test]
    fn simple_string_frame_parses_as_zero_arg_command() {
        let frame = Frame::SimpleString("RESET".into());
        let cmd = Command::from_frame(frame).expect("SimpleString → Command");
        assert_eq!(cmd.name, "RESET");
        assert_eq!(cmd.arg_count(), 0);
    }

    #[test]
    fn bulk_string_command_name_is_uppercased() {
        let frame = Frame::BulkString(Bytes::from("hello"));
        let cmd = Command::from_frame(frame).unwrap();
        assert_eq!(cmd.name, "HELLO");
    }

    #[test]
    fn map_frame_still_rejected_as_command() {
        // Map frames must never be treated as commands — this keeps our
        // from_frame guard against unexpected client behaviour.
        let frame = Frame::Map(vec![]);
        let result = Command::from_frame(frame);
        assert!(
            matches!(result, Err(ProtocolError::Parse(_))),
            "Map frame must still be rejected as a command, got: {result:?}"
        );
    }

    // -- encode_versioned: RESP2 vs RESP3 null / empty array -------------------

    /// RESP3: Frame::Null must encode as `_\r\n` (unified null type).
    #[test]
    fn encode_versioned_null_resp3_emits_underscore() {
        let mut buf = BytesMut::new();
        Encoder::encode_versioned(&Frame::Null, ProtocolVersion::Resp3, &mut buf);
        assert_eq!(
            &buf[..],
            b"_\r\n",
            "RESP3 null must be `_\\r\\n`, got: {buf:?}"
        );
    }

    /// RESP2: Frame::Null must encode as `$-1\r\n` (null bulk string).
    ///
    /// WHY: Lettuce and other RESP2 clients have no parser for `_\r\n` — they
    /// see byte 0x5f (underscore) as an unknown type and throw a
    /// DataAccessException "Error in execution". The RESP2 convention for
    /// absent/nil values is the null bulk string `$-1\r\n`.
    #[test]
    fn encode_versioned_null_resp2_emits_null_bulk_string() {
        let mut buf = BytesMut::new();
        Encoder::encode_versioned(&Frame::Null, ProtocolVersion::Resp2, &mut buf);
        assert_eq!(
            &buf[..],
            b"$-1\r\n",
            "RESP2 null must be `$-1\\r\\n`, got: {buf:?}"
        );
    }

    /// Empty Array encodes as `*0\r\n` identically in RESP2 and RESP3.
    #[test]
    fn encode_versioned_empty_array_same_in_both_protocols() {
        let frame = Frame::Array(vec![]);
        let mut buf2 = BytesMut::new();
        let mut buf3 = BytesMut::new();
        Encoder::encode_versioned(&frame, ProtocolVersion::Resp2, &mut buf2);
        Encoder::encode_versioned(&frame, ProtocolVersion::Resp3, &mut buf3);
        assert_eq!(&buf2[..], b"*0\r\n");
        assert_eq!(&buf3[..], b"*0\r\n");
    }

    /// Array containing Null items: each element must use the correct encoding.
    #[test]
    fn encode_versioned_array_with_null_elements_resp2() {
        let frame = Frame::Array(vec![
            Frame::BulkString(Bytes::from("val")),
            Frame::Null,
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode_versioned(&frame, ProtocolVersion::Resp2, &mut buf);
        // Expected: *2\r\n$3\r\nval\r\n$-1\r\n
        let expected = b"*2\r\n$3\r\nval\r\n$-1\r\n";
        assert_eq!(
            &buf[..],
            expected.as_ref(),
            "RESP2 array null element must be `$-1\\r\\n`"
        );
    }

    #[test]
    fn encode_versioned_array_with_null_elements_resp3() {
        let frame = Frame::Array(vec![
            Frame::BulkString(Bytes::from("val")),
            Frame::Null,
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode_versioned(&frame, ProtocolVersion::Resp3, &mut buf);
        // Expected: *2\r\n$3\r\nval\r\n_\r\n
        let expected = b"*2\r\n$3\r\nval\r\n_\r\n";
        assert_eq!(
            &buf[..],
            expected.as_ref(),
            "RESP3 array null element must be `_\\r\\n`"
        );
    }

    /// RESP2: Boolean must be encoded as `:1\r\n` or `:0\r\n`.
    #[test]
    fn encode_versioned_boolean_resp2_emits_integer() {
        let mut buf_true = BytesMut::new();
        let mut buf_false = BytesMut::new();
        Encoder::encode_versioned(&Frame::Boolean(true), ProtocolVersion::Resp2, &mut buf_true);
        Encoder::encode_versioned(&Frame::Boolean(false), ProtocolVersion::Resp2, &mut buf_false);
        assert_eq!(&buf_true[..], b":1\r\n");
        assert_eq!(&buf_false[..], b":0\r\n");
    }

    /// RESP3: Boolean must be encoded as `#t\r\n` or `#f\r\n`.
    #[test]
    fn encode_versioned_boolean_resp3_emits_bool_type() {
        let mut buf_true = BytesMut::new();
        let mut buf_false = BytesMut::new();
        Encoder::encode_versioned(&Frame::Boolean(true), ProtocolVersion::Resp3, &mut buf_true);
        Encoder::encode_versioned(&Frame::Boolean(false), ProtocolVersion::Resp3, &mut buf_false);
        assert_eq!(&buf_true[..], b"#t\r\n");
        assert_eq!(&buf_false[..], b"#f\r\n");
    }

    /// RESP2 Map must be flattened to a `*N*2` Array (interleaved key-value pairs).
    #[test]
    fn encode_versioned_map_resp2_flattens_to_array() {
        let frame = Frame::Map(vec![
            (Frame::BulkString(Bytes::from("k")), Frame::Integer(1)),
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode_versioned(&frame, ProtocolVersion::Resp2, &mut buf);
        // Expected: *2\r\n$1\r\nk\r\n:1\r\n
        let expected = b"*2\r\n$1\r\nk\r\n:1\r\n";
        assert_eq!(&buf[..], expected.as_ref(), "RESP2 Map must flatten to Array");
    }

    /// RESP3 Map encodes with the `%` prefix.
    #[test]
    fn encode_versioned_map_resp3_uses_map_type() {
        let frame = Frame::Map(vec![
            (Frame::BulkString(Bytes::from("k")), Frame::Integer(1)),
        ]);
        let mut buf = BytesMut::new();
        Encoder::encode_versioned(&frame, ProtocolVersion::Resp3, &mut buf);
        // Starts with '%'
        assert_eq!(buf[0], b'%', "RESP3 Map must start with '%'");
    }

    /// encode() (the arity-1 method) remains pure RESP3 — `_\r\n` for Null.
    #[test]
    fn encode_without_version_remains_resp3() {
        let mut buf = BytesMut::new();
        Encoder::encode(&Frame::Null, &mut buf);
        assert_eq!(&buf[..], b"_\r\n", "encode() must still emit RESP3 null");
    }
}
