// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Zero-copy bidirectional TCP splice for the ARMAGEDDON FORGE L4 proxy.
//!
//! # Linux `splice(2)` approach
//!
//! The Linux kernel's `splice(2)` syscall moves pages between a file descriptor
//! and a pipe without ever copying bytes through userspace.  Because
//! `splice(2)` requires at least one end of the transfer to be a pipe, a
//! bidirectional socket↔socket transfer is done with **two pipes**:
//!
//! ```text
//! client_socket  ──splice──▶  pipe_ab  ──splice──▶  upstream_socket
//! upstream_socket ──splice──▶  pipe_ba  ──splice──▶  client_socket
//! ```
//!
//! Each direction runs as an independent future (joined with `tokio::join!`).
//! When either side closes its write end the half-close propagates naturally
//! and the other direction drains to EOF.
//!
//! # Fallback
//!
//! On non-Linux targets, or when the kernel is detected to be older than 4.5
//! (where `SPLICE_F_NONBLOCK` on sockets was reliably fixed), the function
//! transparently falls back to `tokio::io::copy_bidirectional`.
//!
//! Runtime detection runs once at process start via [`is_splice_supported`] and
//! caches the result in an `AtomicBool`; subsequent calls pay only an atomic
//! load.
//!
//! # Usage
//!
//! ```ignore
//! use armageddon_forge::splice::splice_bidirectional;
//! use tokio::net::TcpStream;
//!
//! let (bytes_a_to_b, bytes_b_to_a) =
//!     splice_bidirectional(client, upstream, 65536).await?;
//! ```

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpStream;

// ---------------------------------------------------------------------------
// Runtime detection
// ---------------------------------------------------------------------------

/// Cached result of the splice-support check.  `true` = splice is usable.
static SPLICE_SUPPORTED: AtomicBool = AtomicBool::new(false);
/// Set to `true` after the first detection run so the check is only done once.
static SPLICE_DETECTED: AtomicBool = AtomicBool::new(false);

/// Return `true` when Linux `splice(2)` can be used on this kernel.
///
/// Detection is performed once (lazily on first call) and cached in a pair of
/// `AtomicBool`s.  On non-Linux platforms this always returns `false`.
pub fn is_splice_supported() -> bool {
    #[cfg(not(target_os = "linux"))]
    {
        false
    }

    #[cfg(target_os = "linux")]
    {
        if SPLICE_DETECTED.load(Ordering::Relaxed) {
            return SPLICE_SUPPORTED.load(Ordering::Relaxed);
        }
        let supported = detect_splice_support_linux();
        SPLICE_SUPPORTED.store(supported, Ordering::Relaxed);
        SPLICE_DETECTED.store(true, Ordering::Relaxed);
        supported
    }
}

/// Parse `/proc/version` and return `true` when the kernel is ≥ 4.5.
#[cfg(target_os = "linux")]
fn detect_splice_support_linux() -> bool {
    // Format: "Linux version X.Y.Z-..."
    match std::fs::read_to_string("/proc/version") {
        Ok(content) => {
            if let Some(version_str) = content.split_whitespace().nth(2) {
                let parts: Vec<&str> = version_str.splitn(3, '.').collect();
                if parts.len() >= 2 {
                    let major: u32 = parts[0].parse().unwrap_or(0);
                    let minor: u32 = parts[1]
                        .split(|c: char| !c.is_ascii_digit())
                        .next()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0);
                    let ok = major > 4 || (major == 4 && minor >= 5);
                    if ok {
                        tracing::debug!(
                            "splice: kernel {}.{} >= 4.5 — zero-copy enabled",
                            major,
                            minor
                        );
                    } else {
                        tracing::warn!(
                            "splice: kernel {}.{} < 4.5 — falling back to copy_bidirectional",
                            major,
                            minor
                        );
                    }
                    return ok;
                }
            }
            tracing::warn!("splice: could not parse kernel version — using copy fallback");
            false
        }
        Err(e) => {
            tracing::warn!(
                "splice: /proc/version unreadable ({}) — using copy fallback",
                e
            );
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Copy bytes bidirectionally between two `TcpStream`s with minimal overhead.
///
/// On Linux ≥ 4.5 this uses `splice(2)` via an intermediate pipe pair so that
/// no bytes pass through userspace.  On all other platforms or when the kernel
/// is too old the call transparently delegates to
/// [`tokio::io::copy_bidirectional`].
///
/// Returns `(bytes_from_a_to_b, bytes_from_b_to_a)` on success.
///
/// # Errors
///
/// Any underlying I/O error is propagated as [`std::io::Error`].
pub async fn splice_bidirectional(
    a: TcpStream,
    b: TcpStream,
    buf_size: usize,
) -> io::Result<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        if is_splice_supported() {
            return linux::splice_bidirectional(a, b, buf_size).await;
        }
    }

    // Non-Linux or old kernel: fall back to tokio's userspace copy.
    let _ = buf_size;
    fallback_copy_bidirectional(a, b).await
}

// ---------------------------------------------------------------------------
// Userspace fallback
// ---------------------------------------------------------------------------

/// Transparent fallback backed by `tokio::io::copy_bidirectional`.
pub(crate) async fn fallback_copy_bidirectional(
    mut a: TcpStream,
    mut b: TcpStream,
) -> io::Result<(u64, u64)> {
    tokio::io::copy_bidirectional(&mut a, &mut b).await
}

// ---------------------------------------------------------------------------
// Linux zero-copy path
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use std::io;
    use std::os::fd::{AsFd, OwnedFd};
    use nix::fcntl::{splice, SpliceFFlags};
    use nix::unistd::pipe2;
    use nix::fcntl::OFlag;
    use tokio::net::TcpStream;

    /// Drive a single direction `src → dst` through a kernel pipe until EOF.
    ///
    /// A fresh `O_NONBLOCK | O_CLOEXEC` pipe is created for this direction;
    /// it is closed when the function returns (via `OwnedFd` drop).
    ///
    /// Readiness tracking uses `TcpStream::ready()` + `clear_ready()` on
    /// `EAGAIN` so we do not busy-spin when the kernel says "no data yet"
    /// after tokio's cached readability flag is already set. Using
    /// `readable().await` alone would return immediately on a stale cached
    /// flag and burn 100 % CPU in a tight `splice`/`EAGAIN` loop.
    async fn splice_one_direction(
        src: &TcpStream,
        dst: &TcpStream,
        chunk: usize,
    ) -> io::Result<u64> {
        use tokio::io::Interest;

        // Create a non-blocking, close-on-exec pipe.
        let (pipe_rd, pipe_wr): (OwnedFd, OwnedFd) =
            pipe2(OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
                .map_err(|e| io::Error::from_raw_os_error(e as i32))?;

        let flags = SpliceFFlags::SPLICE_F_MOVE | SpliceFFlags::SPLICE_F_NONBLOCK;
        let mut total: u64 = 0;

        loop {
            // ----------------------------------------------------------------
            // Phase 1: wait until src is readable, then splice src → pipe_wr
            // ----------------------------------------------------------------
            // `try_io` wraps the syscall so tokio can manage readiness: on
            // `WouldBlock` it clears the cached readable flag and the next
            // `.ready(...).await` actually waits for a fresh kernel event.
            // This prevents the 100 %-CPU busy-spin that a naïve
            // `readable().await` + raw `splice()` loop suffers from.
            let n: usize = match src
                .async_io(Interest::READABLE, || {
                    match splice(
                        src.as_fd(),
                        None,
                        pipe_wr.as_fd(),
                        None,
                        chunk,
                        flags,
                    ) {
                        Ok(n) => Ok(n),
                        Err(nix::errno::Errno::EAGAIN) => {
                            Err(io::Error::from(io::ErrorKind::WouldBlock))
                        }
                        Err(nix::errno::Errno::EINTR) => {
                            Err(io::Error::from(io::ErrorKind::Interrupted))
                        }
                        Err(nix::errno::Errno::EPIPE)
                        | Err(nix::errno::Errno::ECONNRESET) => {
                            Err(io::Error::new(
                                io::ErrorKind::ConnectionReset,
                                "source closed",
                            ))
                        }
                        Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
                    }
                })
                .await
            {
                Ok(0) => return Ok(total), // EOF on source
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                // Source closed by peer or reset: clean end-of-stream.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {
                    return Ok(total)
                }
                Err(e) => return Err(e),
            };

            if n == 0 {
                continue;
            }

            // ----------------------------------------------------------------
            // Phase 2: drain pipe_rd → dst (exactly the `n` bytes we put in)
            // ----------------------------------------------------------------
            let mut drained = 0usize;
            while drained < n {
                let remaining = n - drained;
                let moved: usize = match dst
                    .async_io(Interest::WRITABLE, || {
                        match splice(
                            pipe_rd.as_fd(),
                            None,
                            dst.as_fd(),
                            None,
                            remaining,
                            flags,
                        ) {
                            Ok(m) => Ok(m),
                            Err(nix::errno::Errno::EAGAIN) => {
                                Err(io::Error::from(io::ErrorKind::WouldBlock))
                            }
                            Err(nix::errno::Errno::EINTR) => {
                                Err(io::Error::from(io::ErrorKind::Interrupted))
                            }
                            Err(nix::errno::Errno::EPIPE)
                            | Err(nix::errno::Errno::ECONNRESET) => {
                                Err(io::Error::new(
                                    io::ErrorKind::BrokenPipe,
                                    "destination closed",
                                ))
                            }
                            Err(e) => Err(io::Error::from_raw_os_error(e as i32)),
                        }
                    })
                    .await
                {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "pipe drained prematurely during splice",
                        ));
                    }
                    Ok(m) => m,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    // Destination closed: return what we've already drained.
                    Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                        return Ok(total)
                    }
                    Err(e) => return Err(e),
                };
                drained += moved;
            }

            total += n as u64;
        }
    }

    /// Propagate a half-close: after `src → dst` has reached clean EOF, shut
    /// down the write half of `dst` so the peer on the other side of `dst`
    /// sees EOF on its read half. Without this, a client that does
    /// `shutdown(write)` after sending a request would observe the proxy's
    /// `client → upstream` splice return cleanly, but the upstream's read
    /// loop would hang — and the reverse direction of `splice_bidirectional`
    /// would never finish, deadlocking the whole proxy connection.
    fn shutdown_write_half(dst: &TcpStream) {
        use std::os::fd::AsRawFd;
        // SAFETY: the fd is owned by `dst` for the lifetime of this call.
        // The return value is deliberately ignored: the peer may already
        // have closed (ENOTCONN) or the socket may have been reset
        // (EPIPE) — both are harmless for the proxy's half-close semantics.
        unsafe {
            let _ = libc::shutdown(dst.as_raw_fd(), libc::SHUT_WR);
        }
    }

    /// Bidirectional zero-copy splice.
    ///
    /// Both directions run concurrently via `tokio::join!`. When one
    /// direction completes with a clean EOF we propagate the half-close to
    /// the peer on the *other* side of the destination socket by calling
    /// `shutdown(SHUT_WR)`. This unblocks cases where a client half-closes
    /// after writing a request (e.g. HTTP/1 without `Content-Length`): the
    /// upstream's read loop can then see EOF and finish, allowing the
    /// reverse direction to drain and return.
    pub async fn splice_bidirectional(
        a: TcpStream,
        b: TcpStream,
        buf_size: usize,
    ) -> io::Result<(u64, u64)> {
        use std::sync::Arc;

        let a = Arc::new(a);
        let b = Arc::new(b);

        let a_ab = Arc::clone(&a);
        let b_ab = Arc::clone(&b);
        let a_ba = Arc::clone(&a);
        let b_ba = Arc::clone(&b);

        let ab = async move {
            let res = splice_one_direction(&a_ab, &b_ab, buf_size).await;
            // Only propagate the half-close on a clean termination. On an
            // I/O error the other direction will observe its own error /
            // EOF naturally and we must not pre-empt its accounting.
            if res.is_ok() {
                shutdown_write_half(&b_ab);
            }
            res
        };
        let ba = async move {
            let res = splice_one_direction(&b_ba, &a_ba, buf_size).await;
            if res.is_ok() {
                shutdown_write_half(&a_ba);
            }
            res
        };

        let (res_ab, res_ba) = tokio::join!(ab, ba);
        Ok((res_ab?, res_ba?))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Spin up a loopback TCP echo server; returns its bound address.
    async fn echo_server() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut r, mut w) = stream.split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });
        addr
    }

    /// Return a connected (client, server-side) TCP pair over loopback.
    async fn loopback_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (client_res, server_res) =
            tokio::join!(TcpStream::connect(addr), listener.accept());
        let client = client_res.unwrap();
        let (server, _peer) = server_res.unwrap();
        (client, server)
    }

    // -----------------------------------------------------------------------
    // Test 1 — 10 MB roundtrip: bytes sent == bytes received
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_splice_10mb_roundtrip() {
        const MB10: usize = 10 * 1024 * 1024;

        let echo_addr = echo_server().await;
        let (mut test_end, proxy_client) = loopback_pair().await;
        let proxy_upstream = TcpStream::connect(echo_addr).await.unwrap();

        // The "proxy" task bridges proxy_client ↔ proxy_upstream.
        tokio::spawn(async move {
            let _ = splice_bidirectional(proxy_client, proxy_upstream, 65536).await;
        });

        // Send 10 MB, receive the echo.
        let payload: Vec<u8> = (0..MB10).map(|i| (i & 0xFF) as u8).collect();
        let write_payload = payload.clone();

        let handle = tokio::spawn(async move {
            test_end.write_all(&write_payload).await.unwrap();
            test_end.shutdown().await.unwrap();
            let mut received = Vec::with_capacity(MB10);
            test_end.read_to_end(&mut received).await.unwrap();
            received
        });

        let received = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            handle,
        )
        .await
        .expect("10 MB roundtrip timed out")
        .unwrap();

        assert_eq!(
            received.len(),
            MB10,
            "bytes received ({}) != bytes sent ({})",
            received.len(),
            MB10
        );
        assert_eq!(received, payload, "payload content mismatch");
    }

    // -----------------------------------------------------------------------
    // Test 2 — byte counts returned by splice_bidirectional are accurate
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_splice_byte_counts_accurate() {
        const SIZE: usize = 8192;

        let echo_addr = echo_server().await;
        let (mut test_end, proxy_client) = loopback_pair().await;
        let proxy_upstream = TcpStream::connect(echo_addr).await.unwrap();

        let splice_handle = tokio::spawn(async move {
            splice_bidirectional(proxy_client, proxy_upstream, 65536).await
        });

        let payload = vec![0x42u8; SIZE];
        test_end.write_all(&payload).await.unwrap();
        test_end.shutdown().await.unwrap();
        let mut buf = Vec::new();
        test_end.read_to_end(&mut buf).await.unwrap();
        assert_eq!(buf.len(), SIZE, "echo returned wrong byte count");

        let (a_to_b, b_to_a) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            splice_handle,
        )
        .await
        .expect("splice_handle timed out")
        .unwrap()
        .unwrap();

        assert_eq!(a_to_b, SIZE as u64, "a_to_b counter wrong");
        assert_eq!(b_to_a, SIZE as u64, "b_to_a counter wrong");
    }

    // -----------------------------------------------------------------------
    // Test 3 — is_splice_supported is idempotent (same value every call)
    // -----------------------------------------------------------------------
    #[test]
    fn test_splice_support_detection_is_idempotent() {
        let a = is_splice_supported();
        let b = is_splice_supported();
        let c = is_splice_supported();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    // -----------------------------------------------------------------------
    // Test 4 — explicit fallback path works correctly
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_fallback_copy_bidirectional_correctness() {
        let echo_addr = echo_server().await;
        let (mut test_end, proxy_client) = loopback_pair().await;
        let proxy_upstream = TcpStream::connect(echo_addr).await.unwrap();

        let splice_handle = tokio::spawn(async move {
            fallback_copy_bidirectional(proxy_client, proxy_upstream).await
        });

        let payload = b"fallback test - AGPL-3.0";
        test_end.write_all(payload).await.unwrap();
        test_end.shutdown().await.unwrap();
        let mut buf = Vec::new();
        test_end.read_to_end(&mut buf).await.unwrap();

        let _ = splice_handle.await.unwrap().unwrap();
        assert_eq!(&buf, payload, "fallback returned wrong payload");
    }

    // -----------------------------------------------------------------------
    // Test 5 — bench: splice vs copy throughput logged to stdout
    //
    // Uses 64 MB for a stable measurement.  Prints MB/s for both paths.
    // No hard threshold (CI hardware varies); correctness is asserted inside.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn bench_splice_vs_copy_throughput() {
        const DATA: usize = 64 * 1024 * 1024;

        async fn measure(label: &str, use_splice: bool) -> f64 {
            let echo_addr = echo_server().await;
            let (mut test_end, proxy_client) = loopback_pair().await;
            let proxy_upstream = TcpStream::connect(echo_addr).await.unwrap();

            let proxy = tokio::spawn(async move {
                if use_splice {
                    splice_bidirectional(proxy_client, proxy_upstream, 65536).await
                } else {
                    fallback_copy_bidirectional(proxy_client, proxy_upstream).await
                }
            });

            let payload = vec![0x55u8; DATA];
            let t0 = std::time::Instant::now();

            test_end.write_all(&payload).await.unwrap();
            test_end.shutdown().await.unwrap();
            let mut received = Vec::with_capacity(DATA);
            test_end.read_to_end(&mut received).await.unwrap();

            let elapsed = t0.elapsed().as_secs_f64();
            let mbs = (DATA as f64 / (1024.0 * 1024.0)) / elapsed;

            let _ = proxy.await.unwrap().unwrap();

            println!("[bench] {}: {:.1} MB/s ({:.3}s)", label, mbs, elapsed);
            assert_eq!(received.len(), DATA, "{}: byte count mismatch", label);
            mbs
        }

        let splice_mbs = measure("splice_bidirectional", true).await;
        let copy_mbs = measure("copy_bidirectional (fallback)", false).await;

        println!(
            "[bench] splice={:.1} MB/s  copy={:.1} MB/s  ratio={:.2}x",
            splice_mbs,
            copy_mbs,
            splice_mbs / copy_mbs.max(0.001)
        );

        assert!(splice_mbs > 0.0, "splice throughput must be positive");
        assert!(copy_mbs > 0.0, "copy throughput must be positive");
    }
}
