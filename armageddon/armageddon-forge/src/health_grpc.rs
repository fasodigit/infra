// SPDX-License-Identifier: AGPL-3.0-only
//! Active gRPC health probe.
//!
//! Uses the standard gRPC Health Checking Protocol (grpc.health.v1) via
//! `tonic-health`. Returns [`ProbeResult::Healthy`] when the response is
//! `SERVING`, [`ProbeResult::Unhealthy`] for every other status or on error.

use crate::health::ProbeResult;
use std::time::Duration;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::HealthCheckRequest;

// -- probe --

/// Call `grpc.health.v1.Health/Check` on `endpoint`.
///
/// * `endpoint` – a URI accepted by [`tonic::transport::Channel`], e.g.
///   `"http://127.0.0.1:50051"`.
/// * `service`  – the service name to check; `None` or `Some("")` checks the
///   server overall.
/// * `timeout`  – hard deadline for the entire RPC round-trip.
///
/// Returns [`ProbeResult::Healthy`] iff the server responds `SERVING`.
pub async fn grpc_probe(
    endpoint: &str,
    service: Option<String>,
    timeout: Duration,
) -> ProbeResult {
    // Build the endpoint descriptor.
    let ep = match tonic::transport::Channel::from_shared(endpoint.to_owned()) {
        Ok(e) => e,
        Err(e) => return ProbeResult::Unhealthy(format!("invalid endpoint URI: {e}")),
    };

    // Establish the channel with a timeout.
    let channel = match tokio::time::timeout(timeout, ep.connect()).await {
        Err(_) => {
            return ProbeResult::Unhealthy(format!(
                "timeout connecting to {endpoint} after {timeout:?}"
            ))
        }
        Ok(Err(e)) => return ProbeResult::Unhealthy(format!("channel error: {e}")),
        Ok(Ok(ch)) => ch,
    };

    let mut client = HealthClient::new(channel);
    let service_name = service.unwrap_or_default();

    let rpc = client.check(HealthCheckRequest {
        service: service_name.clone(),
    });

    match tokio::time::timeout(timeout, rpc).await {
        Err(_) => ProbeResult::Unhealthy(format!("rpc timeout for service '{service_name}'")),
        Ok(Err(status)) => {
            ProbeResult::Unhealthy(format!("rpc error {}: {}", status.code(), status.message()))
        }
        Ok(Ok(response)) => {
            use tonic_health::pb::health_check_response::ServingStatus;
            let inner = response.into_inner();
            // Compare via the enum's integer discriminant.
            if inner.status == ServingStatus::Serving as i32 {
                tracing::debug!("grpc_probe: {} service='{}' SERVING", endpoint, service_name);
                ProbeResult::Healthy
            } else {
                let status_name = ServingStatus::try_from(inner.status)
                    .map(|s| format!("{s:?}"))
                    .unwrap_or_else(|_| format!("unknown({})", inner.status));
                tracing::debug!(
                    "grpc_probe: {} service='{}' {}",
                    endpoint,
                    service_name,
                    status_name
                );
                ProbeResult::Unhealthy(format!("serving status: {status_name}"))
            }
        }
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tonic::transport::Server;
    use tonic_health::server::HealthReporter;
    // tonic_health::ServingStatus is the server-side enum used by HealthReporter;
    // tonic_health::pb::health_check_response::ServingStatus is the protobuf enum.
    use tonic_health::ServingStatus as ServerServingStatus;

    /// Spawn a real tonic health server, configure it, return (endpoint, reporter).
    async fn spawn_health_server(
        service: &str,
        status: ServerServingStatus,
    ) -> (String, HealthReporter) {
        use tokio::net::TcpListener;
        use tokio_stream::wrappers::TcpListenerStream;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let endpoint = format!("http://{addr}");

        let (mut reporter, health_service) = tonic_health::server::health_reporter();
        reporter.set_service_status(service, status).await;

        let reporter_clone = reporter.clone();
        tokio::spawn(async move {
            Server::builder()
                .add_service(health_service)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .ok();
        });

        // Brief pause so the server socket is ready.
        tokio::time::sleep(Duration::from_millis(50)).await;

        (endpoint, reporter_clone)
    }

    /// gRPC probe returns Healthy when the server reports SERVING.
    #[tokio::test]
    async fn test_grpc_probe_serving() {
        let (ep, _reporter) =
            spawn_health_server("my.Service", ServerServingStatus::Serving).await;
        let result = grpc_probe(&ep, Some("my.Service".into()), Duration::from_secs(5)).await;
        assert!(
            matches!(result, ProbeResult::Healthy),
            "expected Healthy, got {result:?}"
        );
    }

    /// gRPC probe returns Unhealthy when the server reports NOT_SERVING.
    #[tokio::test]
    async fn test_grpc_probe_not_serving() {
        let (ep, _reporter) =
            spawn_health_server("my.Service", ServerServingStatus::NotServing).await;
        let result = grpc_probe(&ep, Some("my.Service".into()), Duration::from_secs(5)).await;
        assert!(
            matches!(result, ProbeResult::Unhealthy(_)),
            "expected Unhealthy, got {result:?}"
        );
        if let ProbeResult::Unhealthy(msg) = &result {
            assert!(
                msg.contains("NotServing") || msg.contains("not_serving") || msg.contains("2"),
                "message should mention NotServing status: {msg}"
            );
        }
    }
}
