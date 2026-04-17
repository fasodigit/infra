//! worker.rs — Boucle d'un worker outbox-relay (1 par shard).
//!
//! Séquence (voir SPEC §7) :
//!   1. BEGIN
//!   2. SELECT ... FOR UPDATE SKIP LOCKED LIMIT N
//!   3. Pour chaque ligne : publisher.publish() → XADD KAYA + PRODUCE Redpanda
//!   4. UPDATE status = 'SENT' (ou retry/dead_letter)
//!   5. COMMIT
//!   6. Boucle (idle backoff si batch vide)

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use crate::{
    Config,
    metrics,
    publisher::{PublishError, Publisher},
};

/// Délais de backoff exponentiel (SPEC §9). Index = retry_count déjà effectué.
/// Après 6 tentatives → DEAD_LETTER.
const BACKOFF_MS: &[u64] = &[200, 400, 800, 1_600, 3_200];
const MAX_RETRIES: i16 = 6;

// -----------------------------------------------------------------------------
// Structure ligne outbox
// -----------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct OutboxRow {
    pub id:                Uuid,
    pub aggregate_id:      Uuid,
    pub aggregate_type:    String,
    pub event_type:        String,
    pub payload:           Vec<u8>,
    pub idempotency_key:   Uuid,
    pub partition_key:     String,
    pub partition_shard:   i16,
    pub retry_count:       i16,
    pub created_at:        DateTime<Utc>,
}

// -----------------------------------------------------------------------------
// Worker
// -----------------------------------------------------------------------------

pub struct Worker {
    shard_id:  u8,
    pool:      PgPool,
    publisher: Arc<Publisher>,
    cfg:       Arc<Config>,
}

impl Worker {
    pub fn new(shard_id: u8, pool: PgPool, publisher: Arc<Publisher>, cfg: Arc<Config>) -> Self {
        Self { shard_id, pool, publisher, cfg }
    }

    /// Boucle principale : tourne jusqu'à annulation de la task.
    #[instrument(skip(self), fields(shard = self.shard_id))]
    pub async fn run(self) -> Result<()> {
        info!("worker starting");
        metrics::worker_up(self.shard_id, true);

        loop {
            match self.process_batch().await {
                Ok(0) => {
                    // Batch vide : petite pause idle
                    tokio::time::sleep(Duration::from_millis(self.cfg.poll_idle_ms)).await;
                }
                Ok(n) => {
                    debug!(processed = n, "batch done");
                }
                Err(e) => {
                    error!(error = ?e, "batch failed, backing off 1s");
                    metrics::inc_batch_failure(self.shard_id);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Traite un batch : retourne le nombre de lignes traitées (0 = rien à faire).
    async fn process_batch(&self) -> Result<usize> {
        let mut tx: Transaction<'_, Postgres> = self.pool.begin().await?;

        // 1. SELECT FOR UPDATE SKIP LOCKED — n'attend pas les lignes déjà lockées
        //    par un autre worker (ce qui, par hash, ne devrait pas arriver, mais
        //    garantit la correction si ré-équilibrage en cours).
        let rows: Vec<OutboxRow> = sqlx::query_as(
            r#"
            SELECT id, aggregate_id, aggregate_type, event_type, payload,
                   idempotency_key, partition_key, partition_shard,
                   retry_count, created_at
              FROM outbox
             WHERE status = 'PENDING'
               AND partition_shard = $1
             ORDER BY created_at ASC
             LIMIT $2
             FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(self.shard_id as i16)
        .bind(self.cfg.batch_size)
        .fetch_all(&mut *tx)
        .await?;

        if rows.is_empty() {
            tx.commit().await?;
            return Ok(0);
        }

        let n = rows.len();
        metrics::set_batch_size(self.shard_id, n as f64);

        for row in rows {
            self.process_row(&mut tx, row).await?;
        }

        tx.commit().await?;
        Ok(n)
    }

    /// Publie une ligne. MAJ status = SENT / retry / DEAD_LETTER.
    #[instrument(skip(self, tx, row), fields(event_id = %row.id, aggregate = %row.aggregate_id))]
    async fn process_row(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        row: OutboxRow,
    ) -> Result<()> {
        let t0 = std::time::Instant::now();

        match self.publisher.publish(&row).await {
            Ok(()) => {
                let elapsed = t0.elapsed();
                metrics::record_publish_duration(self.shard_id, elapsed);
                metrics::record_lag(row.created_at);

                sqlx::query(
                    r#"
                    UPDATE outbox
                       SET status = 'SENT',
                           updated_at = NOW()
                     WHERE id = $1 AND created_at = $2
                    "#,
                )
                .bind(row.id)
                .bind(row.created_at)
                .execute(&mut **tx)
                .await?;

                metrics::inc_sent(self.shard_id);
                Ok(())
            }
            Err(PublishError::Fatal(err)) => {
                warn!(error = %err, "fatal publish error, marking DEAD_LETTER");
                self.mark_dead_letter(tx, &row, &err.to_string()).await
            }
            Err(PublishError::Transient(err)) => {
                let next_retry = row.retry_count + 1;
                if next_retry >= MAX_RETRIES {
                    warn!(retries = next_retry, "max retries reached, DEAD_LETTER");
                    self.mark_dead_letter(tx, &row, &err.to_string()).await
                } else {
                    let delay = BACKOFF_MS
                        .get(next_retry as usize)
                        .copied()
                        .unwrap_or(3_200);
                    debug!(retries = next_retry, delay_ms = delay, "transient failure, will retry");

                    sqlx::query(
                        r#"
                        UPDATE outbox
                           SET retry_count = $3,
                               error_reason = $4,
                               updated_at = NOW()
                         WHERE id = $1 AND created_at = $2
                        "#,
                    )
                    .bind(row.id)
                    .bind(row.created_at)
                    .bind(next_retry)
                    .bind(truncate(&err.to_string(), 512))
                    .execute(&mut **tx)
                    .await?;

                    metrics::inc_retry(self.shard_id);
                    // Note : on ne sleep PAS ici (on libère le lock via COMMIT).
                    // Le délai réel vient du fait qu'au prochain SELECT, la ligne
                    // sera re-lue. Pour un vrai delay par ligne, il faudrait un
                    // champ `visible_after TIMESTAMPTZ` (extension v3.2).
                    Ok(())
                }
            }
        }
    }

    async fn mark_dead_letter(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        row: &OutboxRow,
        reason: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE outbox
               SET status = 'DEAD_LETTER',
                   error_reason = $3,
                   updated_at = NOW()
             WHERE id = $1 AND created_at = $2
            "#,
        )
        .bind(row.id)
        .bind(row.created_at)
        .bind(truncate(reason, 512))
        .execute(&mut **tx)
        .await?;

        metrics::inc_dead_letter(self.shard_id);
        error!(
            event_id = %row.id,
            aggregate = %row.aggregate_id,
            event_type = %row.event_type,
            reason = %reason,
            "DEAD_LETTER — on-call page triggered by Prometheus alert"
        );
        Ok(())
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { s.chars().take(max).collect() }
}
