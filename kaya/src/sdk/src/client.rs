//! KAYA client: connects to a KAYA server over RESP3.

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use kaya_protocol::{Decoder, Encoder, Frame};

use crate::{ClientConfig, SdkError};

/// Async KAYA client.
pub struct KayaClient {
    stream: TcpStream,
    read_buf: BytesMut,
    write_buf: BytesMut,
}

impl KayaClient {
    /// Connect to a KAYA server.
    pub async fn connect(config: &ClientConfig) -> Result<Self, SdkError> {
        let addr = format!("{}:{}", config.host, config.port);
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| SdkError::Connection(e.to_string()))?;

        let mut client = Self {
            stream,
            read_buf: BytesMut::with_capacity(4096),
            write_buf: BytesMut::with_capacity(4096),
        };

        // Authenticate if password is set.
        if let Some(ref password) = config.password {
            let resp = client
                .execute_raw(&["AUTH", password])
                .await?;
            if resp.is_error() {
                return Err(SdkError::Server("authentication failed".into()));
            }
        }

        Ok(client)
    }

    /// Send a raw command and receive the response.
    pub async fn execute_raw(&mut self, args: &[&str]) -> Result<Frame, SdkError> {
        let frame = Frame::Array(
            args.iter()
                .map(|a| Frame::BulkString(Bytes::from(a.to_string())))
                .collect(),
        );

        self.write_buf.clear();
        Encoder::encode(&frame, &mut self.write_buf);
        self.stream.write_all(&self.write_buf).await?;

        // Read response.
        loop {
            let n = self.stream.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(SdkError::Connection("connection closed".into()));
            }

            match Decoder::decode(&mut self.read_buf) {
                Ok(frame) => return Ok(frame),
                Err(kaya_protocol::ProtocolError::Incomplete) => continue,
                Err(e) => return Err(SdkError::Protocol(e)),
            }
        }
    }

    // -- convenience methods ------------------------------------------------

    pub async fn ping(&mut self) -> Result<String, SdkError> {
        let resp = self.execute_raw(&["PING"]).await?;
        Ok(resp.as_str().unwrap_or("PONG").to_string())
    }

    pub async fn get(&mut self, key: &str) -> Result<Option<Bytes>, SdkError> {
        let resp = self.execute_raw(&["GET", key]).await?;
        match resp {
            Frame::BulkString(b) => Ok(Some(b)),
            Frame::Null => Ok(None),
            Frame::Error(e) => Err(SdkError::Server(e)),
            _ => Ok(None),
        }
    }

    pub async fn set(&mut self, key: &str, value: &str) -> Result<(), SdkError> {
        let resp = self.execute_raw(&["SET", key, value]).await?;
        if resp.is_error() {
            if let Frame::Error(e) = resp {
                return Err(SdkError::Server(e));
            }
        }
        Ok(())
    }

    pub async fn set_ex(
        &mut self,
        key: &str,
        value: &str,
        seconds: u64,
    ) -> Result<(), SdkError> {
        let ttl_str = seconds.to_string();
        let resp = self
            .execute_raw(&["SET", key, value, "EX", &ttl_str])
            .await?;
        if resp.is_error() {
            if let Frame::Error(e) = resp {
                return Err(SdkError::Server(e));
            }
        }
        Ok(())
    }

    pub async fn del(&mut self, keys: &[&str]) -> Result<i64, SdkError> {
        let mut args = vec!["DEL"];
        args.extend_from_slice(keys);
        let resp = self.execute_raw(&args).await?;
        match resp {
            Frame::Integer(n) => Ok(n),
            Frame::Error(e) => Err(SdkError::Server(e)),
            _ => Ok(0),
        }
    }

    pub async fn exists(&mut self, key: &str) -> Result<bool, SdkError> {
        let resp = self.execute_raw(&["EXISTS", key]).await?;
        match resp {
            Frame::Integer(n) => Ok(n > 0),
            _ => Ok(false),
        }
    }

    pub async fn incr(&mut self, key: &str) -> Result<i64, SdkError> {
        let resp = self.execute_raw(&["INCR", key]).await?;
        match resp {
            Frame::Integer(n) => Ok(n),
            Frame::Error(e) => Err(SdkError::Server(e)),
            _ => Err(SdkError::Server("unexpected response".into())),
        }
    }

    pub async fn xadd(
        &mut self,
        stream: &str,
        id: &str,
        fields: &[(&str, &str)],
    ) -> Result<String, SdkError> {
        let mut args = vec!["XADD", stream, id];
        for (k, v) in fields {
            args.push(k);
            args.push(v);
        }
        let resp = self.execute_raw(&args).await?;
        match resp {
            Frame::BulkString(b) => {
                Ok(String::from_utf8(b.to_vec()).unwrap_or_default())
            }
            Frame::Error(e) => Err(SdkError::Server(e)),
            _ => Err(SdkError::Server("unexpected response".into())),
        }
    }
}
