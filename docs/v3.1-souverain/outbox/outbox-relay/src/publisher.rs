//! publisher.rs — Séquence de publication KAYA → Redpanda (voir SPEC §7).
//!
//! Ordre strict :
//!   1. XADD  <module>.events.<aggregate_type>   (hot path, KAYA, < 1 ms, best-effort)
//!   2. PRODUCE <module>.events.v1               (durabilité légale, Redpanda, 2-15 ms)
//!   3. (le worker fera l'UPDATE status='SENT')
//!
//! Idempotence :
//!   - idempotency_key propagé en header Kafka et en champ KAYA Stream.
//!   - clé de partition Kafka = aggregate_id → ordering per-agrégat.

use std::{sync::Arc, time::Duration};

use rdkafka::{
    ClientConfig,
    producer::{FutureProducer, FutureRecord, Producer},
};
use redis::{AsyncCommands, aio::ConnectionManager};
use thiserror::Error;
use tracing::{debug, instrument, warn};

use crate::{Config, worker::OutboxRow};

// -----------------------------------------------------------------------------
// Errors
// -----------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PublishError {
    /// Erreur retryable : broker down, timeout, network partition.
    #[error("transient: {0}")]
    Transient(String),

    /// Erreur non retryable : schema invalide, ACL refusée, topic inexistant.
    #[error("fatal: {0}")]
    Fatal(String),
}

// -----------------------------------------------------------------------------
// Publisher
// -----------------------------------------------------------------------------

pub struct Publisher {
    producer: FutureProducer,
    kaya:     ConnectionManager,
    cfg:      Arc<Config>,
}

impl Publisher {
    pub fn new(producer: FutureProducer, kaya: ConnectionManager, cfg: Arc<Config>) -> Self {
        Self { producer, kaya, cfg }
    }

    /// Publie l'événement : KAYA puis Redpanda. Retourne Ok si Redpanda OK
    /// (KAYA best-effort, voir SPEC §13.1).
    #[instrument(skip(self, row), fields(event_id = %row.id))]
    pub async fn publish(&self, row: &OutboxRow) -> Result<(), PublishError> {
        // ----- Étape 1 : XADD KAYA (best-effort) -----
        if let Err(e) = self.xadd_kaya(row).await {
            warn!(error = %e, "KAYA XADD failed — degraded mode, continuing with Redpanda");
            // On ne retourne PAS d'erreur : Redpanda est la vérité légale.
            // Les consumers KAYA rattraperont via Redpanda si nécessaire.
        }

        // ----- Étape 2 : PRODUCE Redpanda (obligatoire) -----
        self.produce_redpanda(row).await?;

        Ok(())
    }

    /// XADD vers le stream KAYA. Best-effort : une erreur n'échoue pas la
    /// publication globale.
    async fn xadd_kaya(&self, row: &OutboxRow) -> Result<(), redis::RedisError> {
        let stream_key = format!(
            "{}.events.{}",
            self.cfg.module_name,
            row.aggregate_type,
        );

        let mut conn = self.kaya.clone();
        let fields: Vec<(&str, String)> = vec![
            ("event_id",        row.id.to_string()),
            ("event_type",      row.event_type.clone()),
            ("aggregate_id",    row.aggregate_id.to_string()),
            ("idempotency_key", row.idempotency_key.to_string()),
            ("payload_b64",     base64_encode(&row.payload)),
        ];

        // XADD <stream> NOMKSTREAM * field1 val1 field2 val2 ...
        // `NOMKSTREAM` : ne crée pas le stream s'il n'existe pas (laissé à l'admin).
        // Ici on laisse KAYA auto-créer pour la simplicité du squelette.
        tokio::time::timeout(
            Duration::from_millis(500),
            conn.xadd::<_, _, _, _, String>(&stream_key, "*", &fields),
        )
        .await
        .map_err(|_| redis::RedisError::from((redis::ErrorKind::IoError, "KAYA XADD timeout")))??;

        debug!(stream = %stream_key, "XADD ok");
        Ok(())
    }

    /// PRODUCE vers Redpanda avec idempotence + acks=all.
    async fn produce_redpanda(&self, row: &OutboxRow) -> Result<(), PublishError> {
        let topic = format!("{}.events.v1", self.cfg.module_name);
        let key   = row.aggregate_id.to_string();

        let headers = rdkafka::message::OwnedHeaders::new()
            .insert(rdkafka::message::Header {
                key: "event_type",
                value: Some(row.event_type.as_bytes()),
            })
            .insert(rdkafka::message::Header {
                key: "idempotency_key",
                value: Some(row.idempotency_key.as_bytes().as_slice()),
            })
            .insert(rdkafka::message::Header {
                key: "aggregate_type",
                value: Some(row.aggregate_type.as_bytes()),
            });

        let record: FutureRecord<String, Vec<u8>> = FutureRecord::to(&topic)
            .key(&key)
            .payload(&row.payload)
            .headers(headers);

        match self.producer.send(record, Duration::from_secs(15)).await {
            Ok((_partition, _offset)) => {
                debug!(topic = %topic, "Redpanda PRODUCE ok");
                Ok(())
            }
            Err((kafka_err, _owned_msg)) => {
                // Classification : certaines erreurs Kafka sont fatales.
                use rdkafka::error::{KafkaError, RDKafkaErrorCode};
                match kafka_err {
                    KafkaError::MessageProduction(RDKafkaErrorCode::UnknownTopicOrPartition)
                    | KafkaError::MessageProduction(RDKafkaErrorCode::TopicAuthorizationFailed)
                    | KafkaError::MessageProduction(RDKafkaErrorCode::InvalidRecord) => {
                        Err(PublishError::Fatal(format!("{kafka_err}")))
                    }
                    _ => Err(PublishError::Transient(format!("{kafka_err}"))),
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Constructeurs de clients
// -----------------------------------------------------------------------------

pub fn build_redpanda_producer(cfg: &Config) -> anyhow::Result<FutureProducer> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.redpanda_brokers)
        .set("enable.idempotence", "true")
        .set("acks", "all")
        .set("max.in.flight.requests.per.connection", "1")
        .set("compression.type", "zstd")
        .set("linger.ms", "5")
        .set("message.send.max.retries", "10")
        // Sécurité : mTLS + SASL SCRAM (secrets injectés via env ou SPIRE).
        .set("security.protocol", "SASL_SSL")
        .set("sasl.mechanism", "SCRAM-SHA-512")
        .set("client.id", format!("outbox-relay-{}-{}", cfg.module_name, cfg.instance_id))
        .create()?;
    Ok(producer)
}

pub async fn build_kaya_client(cfg: &Config) -> anyhow::Result<ConnectionManager> {
    let client = redis::Client::open(cfg.kaya_url.as_str())?;
    let mgr = ConnectionManager::new(client).await?;
    Ok(mgr)
}

// -----------------------------------------------------------------------------
// Utils
// -----------------------------------------------------------------------------

fn base64_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    // Encodage base64 minimal inline pour éviter la dépendance base64 dans
    // le squelette. En prod, utiliser le crate `base64` (cf. Cargo.toml à étendre).
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let _ = write!(
            out,
            "{}{}{}{}",
            T[(b0 >> 2) as usize] as char,
            T[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char,
            if chunk.len() > 1 { T[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char } else { '=' },
            if chunk.len() > 2 { T[(b2 & 0x3f) as usize] as char } else { '=' },
        );
    }
    out
}
