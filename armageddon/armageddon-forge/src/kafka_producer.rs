//! Redpanda/Kafka producer abstraction for ARMAGEDDON webhook pipeline.
//!
//! Architecture:
//! - `ProducerBackend` trait: async `produce(topic, key, payload)`.
//! - `RedpandaProducer`: production implementation backed by `rdkafka::FutureProducer`
//!   (compiled only when feature `rdkafka-backend` is active).
//! - `LoggingProducer`: stand-in for test/dev environments — logs the message
//!   via `tracing` without requiring a running broker.
//!
//! Prometheus metrics:
//! - `armageddon_kafka_produce_total{topic, status}` — counter
//! - `armageddon_kafka_produce_latency_seconds{topic}` — histogram

use armageddon_common::error::Result;
use async_trait::async_trait;
use prometheus::{
    register_counter_vec, register_histogram_vec, CounterVec, HistogramOpts, HistogramVec, Opts,
};
use std::sync::Arc;
use std::time::Instant;

// -- constants --

/// Maximum transient-error retries.
const MAX_RETRIES: u8 = 3;
/// Initial retry back-off (ms); doubles each attempt.
const RETRY_BASE_MS: u64 = 50;

// -- metrics --

fn produce_total() -> &'static CounterVec {
    static ONCE: std::sync::OnceLock<CounterVec> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        register_counter_vec!(
            Opts::new(
                "armageddon_kafka_produce_total",
                "Total Kafka/Redpanda produce attempts"
            ),
            &["topic", "status"]
        )
        .expect("register armageddon_kafka_produce_total")
    })
}

fn produce_latency() -> &'static HistogramVec {
    static ONCE: std::sync::OnceLock<HistogramVec> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        register_histogram_vec!(
            HistogramOpts::new(
                "armageddon_kafka_produce_latency_seconds",
                "Kafka/Redpanda produce latency in seconds"
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["topic"]
        )
        .expect("register armageddon_kafka_produce_latency_seconds")
    })
}

// -- producer backend trait --

/// Asynchronous Kafka/Redpanda producer back-end.
///
/// Implement this to swap the underlying transport (rdkafka, log-only, test stub, …).
#[async_trait]
pub trait ProducerBackend: Send + Sync + std::fmt::Debug {
    /// Produce a single message.
    async fn produce_raw(&self, topic: &str, key: &str, payload: &[u8]) -> Result<()>;
}

// -- log-only (dev/test) implementation --

/// Log-only producer — emits a `tracing::debug` span instead of calling a broker.
///
/// Used in development and test environments where no Redpanda instance is
/// available.  Safe to construct anywhere; never fails.
#[derive(Debug, Clone, Default)]
pub struct LoggingProducer;

#[async_trait]
impl ProducerBackend for LoggingProducer {
    async fn produce_raw(&self, topic: &str, key: &str, payload: &[u8]) -> Result<()> {
        tracing::debug!(
            topic = %topic,
            key = %key,
            payload_len = payload.len(),
            "[LoggingProducer] would produce message (no broker configured)"
        );
        Ok(())
    }
}

// -- rdkafka-backed implementation --

/// Production Redpanda producer backed by `rdkafka::FutureProducer`.
///
/// Features:
/// - Idempotent delivery (`enable.idempotence = true`)
/// - Snappy compression
/// - 5 s produce timeout
/// - Automatic retry (up to `MAX_RETRIES`) with exponential back-off
#[cfg(feature = "rdkafka-backend")]
mod rdkafka_backend {
    use super::*;
    use rdkafka::config::ClientConfig;
    use rdkafka::producer::{FutureProducer, FutureRecord};
    use rdkafka::util::Timeout;
    use std::time::Duration;

    /// rdkafka-backed producer.
    #[derive(Clone)]
    pub struct RdkafkaProducer {
        inner: Arc<FutureProducer>,
    }

    impl std::fmt::Debug for RdkafkaProducer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RdkafkaProducer").finish()
        }
    }

    impl RdkafkaProducer {
        /// Build a connected producer.
        pub fn new(brokers: &[String]) -> Result<Self> {
            let broker_list = brokers.join(",");
            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", &broker_list)
                .set("enable.idempotence", "true")
                .set("compression.type", "snappy")
                .set("acks", "all")
                .set("max.in.flight.requests.per.connection", "5")
                .set("retries", "5")
                .set("retry.backoff.ms", "100")
                .set(
                    "message.timeout.ms",
                    &(PRODUCE_TIMEOUT_SECS * 1000).to_string(),
                )
                .create()
                .map_err(|e| {
                    ArmageddonError::Internal(format!("Redpanda producer init: {e}"))
                })?;

            Ok(Self {
                inner: Arc::new(producer),
            })
        }
    }

    #[async_trait]
    impl ProducerBackend for RdkafkaProducer {
        async fn produce_raw(&self, topic: &str, key: &str, payload: &[u8]) -> Result<()> {
            let record = FutureRecord::to(topic).key(key).payload(payload);
            let timeout = Timeout::After(Duration::from_secs(PRODUCE_TIMEOUT_SECS));
            self.inner
                .send(record, timeout)
                .await
                .map(|_| ())
                .map_err(|(e, _)| {
                    ArmageddonError::Internal(format!(
                        "Redpanda produce failed (topic={topic}): {e}"
                    ))
                })
        }
    }
}

#[cfg(feature = "rdkafka-backend")]
pub use rdkafka_backend::RdkafkaProducer;

// -- public facade --

/// Thread-safe Redpanda producer facade with metrics and retry logic.
///
/// Wraps any `ProducerBackend` implementation.  In production, construct with
/// `RedpandaProducer::new_rdkafka(brokers)` (requires feature `rdkafka-backend`).
/// In tests, construct with `RedpandaProducer::new_logging()`.
#[derive(Clone, Debug)]
pub struct RedpandaProducer {
    backend: Arc<dyn ProducerBackend>,
}

impl RedpandaProducer {
    /// Construct with the log-only backend (no broker required).
    pub fn new_logging() -> Self {
        Self {
            backend: Arc::new(LoggingProducer),
        }
    }

    /// Construct with a custom backend (e.g. test mock).
    pub fn with_backend(backend: Arc<dyn ProducerBackend>) -> Self {
        Self { backend }
    }

    /// Construct with the rdkafka backend.
    ///
    /// Only available when feature `rdkafka-backend` is enabled.
    #[cfg(feature = "rdkafka-backend")]
    pub fn new_rdkafka(brokers: &[String]) -> Result<Self> {
        let backend = RdkafkaProducer::new(brokers)?;
        Ok(Self {
            backend: Arc::new(backend),
        })
    }

    /// Produce `payload` to `topic` using `key` as the partition key.
    ///
    /// Retries up to `MAX_RETRIES` times on transient errors.
    /// Records Prometheus metrics on each attempt.
    pub async fn produce(&self, topic: &str, key: &str, payload: &[u8]) -> Result<()> {
        let mut attempt: u8 = 0;
        let mut backoff_ms = RETRY_BASE_MS;

        loop {
            let t0 = Instant::now();
            let res = self.backend.produce_raw(topic, key, payload).await;
            let elapsed = t0.elapsed().as_secs_f64();

            produce_latency()
                .with_label_values(&[topic])
                .observe(elapsed);

            match res {
                Ok(()) => {
                    produce_total()
                        .with_label_values(&[topic, "success"])
                        .inc();
                    tracing::debug!(
                        topic = %topic,
                        key = %key,
                        attempt = attempt + 1,
                        elapsed_ms = (elapsed * 1000.0) as u64,
                        "Redpanda message delivered"
                    );
                    return Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    produce_total()
                        .with_label_values(&[topic, "error"])
                        .inc();

                    if attempt > MAX_RETRIES {
                        tracing::error!(
                            topic = %topic,
                            key = %key,
                            error = %e,
                            "Redpanda produce failed after {} attempts",
                            attempt
                        );
                        return Err(e);
                    }

                    tracing::warn!(
                        topic = %topic,
                        key = %key,
                        error = %e,
                        attempt = attempt,
                        backoff_ms = backoff_ms,
                        "Redpanda produce error, retrying"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                }
            }
        }
    }
}

// -- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use armageddon_common::error::ArmageddonError;

    #[test]
    fn test_logging_producer_new() {
        let p = RedpandaProducer::new_logging();
        // Logging producer always succeeds; just ensure it's Debug-printable
        let _ = format!("{:?}", p);
    }

    #[tokio::test]
    async fn test_logging_producer_produce_ok() {
        let p = RedpandaProducer::new_logging();
        let result = p.produce("test-topic", "key-1", b"hello").await;
        assert!(result.is_ok(), "LoggingProducer should always succeed");
    }

    #[tokio::test]
    async fn test_producer_retry_on_backend_failure() {
        use std::sync::atomic::{AtomicU8, Ordering};

        // Custom backend that always fails
        #[derive(Debug)]
        struct AlwaysFailBackend {
            calls: Arc<AtomicU8>,
        }

        #[async_trait]
        impl ProducerBackend for AlwaysFailBackend {
            async fn produce_raw(
                &self,
                _topic: &str,
                _key: &str,
                _payload: &[u8],
            ) -> Result<()> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Err(ArmageddonError::Internal("simulated failure".to_string()))
            }
        }

        let calls = Arc::new(AtomicU8::new(0));
        let backend = AlwaysFailBackend { calls: calls.clone() };
        let producer = RedpandaProducer::with_backend(Arc::new(backend));

        let result = producer.produce("topic", "k", b"payload").await;
        assert!(result.is_err());
        // MAX_RETRIES is 3, so we expect 1 initial attempt + 3 retries = 4 calls
        assert_eq!(
            calls.load(Ordering::SeqCst),
            MAX_RETRIES + 1,
            "should attempt 1 + MAX_RETRIES times total"
        );
    }

    #[test]
    fn test_clone_shares_backend() {
        let p = RedpandaProducer::new_logging();
        let c = p.clone();
        assert!(Arc::ptr_eq(&p.backend, &c.backend));
    }
}
