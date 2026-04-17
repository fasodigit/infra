// SPDX-License-Identifier: AGPL-3.0-only
//! Active TCP health probe.
//!
//! Attempts a raw TCP connection to `addr` within `timeout`. Returns
//! [`ProbeResult::Healthy`] if the handshake completes, [`ProbeResult::Unhealthy`]
//! otherwise.

use crate::health::ProbeResult;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;

// -- probe --

/// Attempt a TCP connect to `addr` within `timeout`.
///
/// # Example
/// ```no_run
/// # tokio_test::block_on(async {
/// use std::net::SocketAddr;
/// use std::time::Duration;
/// use armageddon_forge::health_tcp::tcp_probe;
///
/// let addr: SocketAddr = "127.0.0.1:6380".parse().unwrap();
/// let result = tcp_probe(addr, Duration::from_secs(2)).await;
/// # })
/// ```
pub async fn tcp_probe(addr: SocketAddr, timeout: Duration) -> ProbeResult {
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => {
            tracing::debug!("tcp_probe: {} reachable", addr);
            ProbeResult::Healthy
        }
        Ok(Err(e)) => {
            tracing::debug!("tcp_probe: {} refused – {}", addr, e);
            ProbeResult::Unhealthy(format!("connection refused: {e}"))
        }
        Err(_elapsed) => {
            tracing::debug!("tcp_probe: {} timed out after {:?}", addr, timeout);
            ProbeResult::Unhealthy(format!("timeout after {:?}", timeout))
        }
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::net::TcpListener;

    /// TCP probe succeeds when a listener is bound on the target port.
    #[tokio::test]
    async fn test_tcp_probe_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();

        // Accept in background so the handshake can complete.
        tokio::spawn(async move {
            let _ = listener.accept().await;
        });

        let result = tcp_probe(addr, Duration::from_secs(2)).await;
        assert!(
            matches!(result, ProbeResult::Healthy),
            "expected Healthy, got {result:?}"
        );
    }

    /// TCP probe reports Unhealthy when nothing listens on the port.
    #[tokio::test]
    async fn test_tcp_probe_refused() {
        // Pick a port that is almost certainly not listening.
        let addr: SocketAddr = "127.0.0.1:19999".parse().unwrap();
        let result = tcp_probe(addr, Duration::from_secs(2)).await;
        assert!(
            matches!(result, ProbeResult::Unhealthy(_)),
            "expected Unhealthy, got {result:?}"
        );
    }

    /// TCP probe reports Unhealthy (timeout) when the connection hangs.
    ///
    /// We simulate a "black-hole" by binding but never calling accept(),
    /// with a very short timeout so the test stays fast.
    ///
    /// NOTE: on Linux the kernel completes the TCP three-way handshake at
    /// the socket level (SYN-SYN/ACK-ACK) even before `accept()` is
    /// called, so the connect() itself succeeds immediately.  To guarantee
    /// a real timeout we route to a non-routable address (TEST-NET-1).
    #[tokio::test]
    async fn test_tcp_probe_timeout() {
        // 192.0.2.1 is TEST-NET-1 (RFC 5737) – packets are silently dropped.
        let addr: SocketAddr = "192.0.2.1:9999".parse().unwrap();
        let result = tcp_probe(addr, Duration::from_millis(200)).await;
        assert!(
            matches!(result, ProbeResult::Unhealthy(_)),
            "expected Unhealthy due to timeout, got {result:?}"
        );
    }
}
