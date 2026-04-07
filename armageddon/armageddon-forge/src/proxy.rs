//! Core proxy logic: request forwarding, load balancing, upstream connection.
//!
//! Uses hyper 1.x + tokio for HTTP reverse proxying.

use armageddon_common::error::{ArmageddonError, Result};
use armageddon_common::types::Endpoint;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Load balancing algorithm.
#[derive(Debug, Clone, Copy)]
pub enum LoadBalancer {
    RoundRobin,
    LeastConnections,
    Random,
    WeightedRoundRobin,
}

/// Atomic round-robin counter for endpoint selection.
pub struct RoundRobinCounter {
    counter: AtomicUsize,
}

impl RoundRobinCounter {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }

    /// Get next index, wrapping around the given length.
    pub fn next(&self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        self.counter.fetch_add(1, Ordering::Relaxed) % len
    }
}

impl Default for RoundRobinCounter {
    fn default() -> Self {
        Self::new()
    }
}

/// Select an endpoint using round-robin from healthy endpoints.
pub fn select_endpoint_round_robin(
    endpoints: &[Endpoint],
    counter: &RoundRobinCounter,
) -> Option<usize> {
    let healthy: Vec<usize> = endpoints
        .iter()
        .enumerate()
        .filter(|(_, e)| e.healthy)
        .map(|(i, _)| i)
        .collect();

    if healthy.is_empty() {
        return None;
    }

    let idx = counter.next(healthy.len());
    Some(healthy[idx])
}

/// Forward an HTTP request to an upstream endpoint and return the response.
///
/// This is the core proxying function. It:
/// 1. Builds the upstream URI from the endpoint address/port
/// 2. Forwards the request method, headers, and body
/// 3. Returns the upstream response
pub async fn forward_request(
    endpoint: &Endpoint,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: Option<Bytes>,
    timeout_ms: u64,
) -> Result<ProxyResponse> {
    let upstream_uri = format!("http://{}:{}{}", endpoint.address, endpoint.port, path);

    // Build the request
    let mut builder = hyper::Request::builder()
        .method(method)
        .uri(&upstream_uri);

    for (name, value) in headers {
        // Skip hop-by-hop headers
        let lower = name.to_lowercase();
        if matches!(
            lower.as_str(),
            "connection"
                | "keep-alive"
                | "transfer-encoding"
                | "te"
                | "trailer"
                | "upgrade"
                | "proxy-authorization"
                | "proxy-authenticate"
        ) {
            continue;
        }
        // Skip host header -- we set it to the upstream
        if lower == "host" {
            continue;
        }
        builder = builder.header(name.as_str(), value.as_str());
    }

    // Set the host header to the upstream
    builder = builder.header("host", format!("{}:{}", endpoint.address, endpoint.port));

    let req_body = body.unwrap_or_default();
    let request = builder
        .body(Full::new(req_body))
        .map_err(|e| ArmageddonError::Internal(format!("failed to build request: {}", e)))?;

    // Create a hyper client
    let client = Client::builder(TokioExecutor::new()).build_http();

    // Send the request with a timeout
    let response = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        client.request(request),
    )
    .await
    .map_err(|_| ArmageddonError::UpstreamTimeout(timeout_ms))?
    .map_err(|e| ArmageddonError::UpstreamConnection(e.to_string()))?;

    // Extract response parts
    let (parts, body) = response.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|e| ArmageddonError::UpstreamConnection(format!("body read error: {}", e)))?
        .to_bytes();

    let resp_headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    Ok(ProxyResponse {
        status: parts.status.as_u16(),
        headers: resp_headers,
        body: body_bytes,
    })
}

/// Response from proxying to an upstream.
pub struct ProxyResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
}

/// Proxy context for a single request being forwarded.
pub struct ProxyContext {
    pub upstream: Endpoint,
    pub timeout_ms: u64,
    pub retries_remaining: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_robin_counter() {
        let counter = RoundRobinCounter::new();
        assert_eq!(counter.next(3), 0);
        assert_eq!(counter.next(3), 1);
        assert_eq!(counter.next(3), 2);
        assert_eq!(counter.next(3), 0); // wraps
        assert_eq!(counter.next(3), 1);
    }

    #[test]
    fn test_round_robin_counter_empty() {
        let counter = RoundRobinCounter::new();
        assert_eq!(counter.next(0), 0);
    }

    #[test]
    fn test_select_endpoint_round_robin() {
        let endpoints = vec![
            Endpoint {
                address: "10.0.0.1".to_string(),
                port: 8080,
                weight: 1,
                healthy: true,
            },
            Endpoint {
                address: "10.0.0.2".to_string(),
                port: 8080,
                weight: 1,
                healthy: false, // unhealthy
            },
            Endpoint {
                address: "10.0.0.3".to_string(),
                port: 8080,
                weight: 1,
                healthy: true,
            },
        ];

        let counter = RoundRobinCounter::new();
        // Should only pick from healthy: indices 0 and 2
        let first = select_endpoint_round_robin(&endpoints, &counter).unwrap();
        assert!(first == 0 || first == 2);
        let second = select_endpoint_round_robin(&endpoints, &counter).unwrap();
        assert!(second == 0 || second == 2);
        assert_ne!(first, second);
    }

    #[test]
    fn test_select_endpoint_no_healthy() {
        let endpoints = vec![Endpoint {
            address: "10.0.0.1".to_string(),
            port: 8080,
            weight: 1,
            healthy: false,
        }];

        let counter = RoundRobinCounter::new();
        assert!(select_endpoint_round_robin(&endpoints, &counter).is_none());
    }
}
