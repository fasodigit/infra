//! KAYA RESP3+ Protocol Parser and Serializer
//!
//! Implements the RESP3 wire protocol (port 6380) for compatibility with
//! Redis clients, plus KAYA-specific extensions.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;

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
    /// Write a frame into the provided buffer.
    pub fn encode(frame: &Frame, buf: &mut BytesMut) {
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
                    Self::encode(item, buf);
                }
            }
            Frame::Null => {
                buf.put_slice(b"_\r\n");
            }
            Frame::Boolean(b) => {
                buf.put_u8(b'#');
                buf.put_u8(if *b { b't' } else { b'f' });
                buf.put_slice(b"\r\n");
            }
            Frame::Double(d) => {
                buf.put_u8(b',');
                buf.put_slice(d.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
            Frame::BigNumber(s) => {
                buf.put_u8(b'(');
                buf.put_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            Frame::VerbatimString { encoding, data } => {
                let payload = format!("{encoding}:{data}");
                buf.put_u8(b'=');
                buf.put_slice(payload.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.put_slice(payload.as_bytes());
                buf.put_slice(b"\r\n");
            }
            Frame::Map(pairs) => {
                buf.put_u8(b'%');
                buf.put_slice(pairs.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for (k, v) in pairs {
                    Self::encode(k, buf);
                    Self::encode(v, buf);
                }
            }
            Frame::Set(items) => {
                buf.put_u8(b'~');
                buf.put_slice(items.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for item in items {
                    Self::encode(item, buf);
                }
            }
            Frame::Push(items) => {
                buf.put_u8(b'>');
                buf.put_slice(items.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for item in items {
                    Self::encode(item, buf);
                }
            }
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
    /// Parse a [`Frame::Array`] of bulk strings into a Command.
    pub fn from_frame(frame: Frame) -> Result<Self, ProtocolError> {
        let parts = frame
            .into_array()
            .ok_or_else(|| ProtocolError::Parse("expected array frame".into()))?;

        if parts.is_empty() {
            return Err(ProtocolError::Parse("empty command".into()));
        }

        let mut iter = parts.into_iter();
        let name_frame = iter.next().unwrap();
        let name = match name_frame {
            Frame::BulkString(b) => {
                String::from_utf8(b.to_vec()).map_err(|_| ProtocolError::InvalidUtf8)?
            }
            Frame::SimpleString(s) => s,
            _ => return Err(ProtocolError::Parse("command name must be string".into())),
        };

        let mut args = Vec::with_capacity(iter.len());
        for frame in iter {
            match frame {
                Frame::BulkString(b) => args.push(b),
                Frame::SimpleString(s) => args.push(Bytes::from(s)),
                Frame::Integer(n) => args.push(Bytes::from(n.to_string())),
                _ => return Err(ProtocolError::Parse("unsupported arg type".into())),
            }
        }

        Ok(Self {
            name: name.to_ascii_uppercase(),
            args,
        })
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
}
