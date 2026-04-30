// SPDX-License-Identifier: AGPL-3.0-or-later
//! Audit logging service (ADR-006, ULTRAPLAN §P1.1).
//!
//! Every write operation appends a row to `audit_t_<slug>.audit_log`.
//! Additionally, the event is published to the Redpanda topic
//! `terroir.audit.event` (best-effort; failures are logged but do not
//! abort the main transaction).
//!
//! The audit schema and table are created by template
//! `T100__audit_log.sql.tmpl` during tenant provisioning.

use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use tracing::{instrument, warn};

// ---------------------------------------------------------------------------
// AuditService
// ---------------------------------------------------------------------------

/// Lightweight audit service — INSERT into tenant audit schema.
pub struct AuditService;

impl AuditService {
    /// Log an audit event.
    ///
    /// `audit_schema`: e.g. `audit_t_t_pilot`
    /// `action`:       e.g. `"producer.created"`, `"parcel.updated"`
    /// `actor_id`:     UUID of the agent/user performing the action
    /// `target_id`:    UUID of the resource being mutated
    /// `metadata`:     arbitrary JSON payload (non-PII only)
    /// `trace_id`:     distributed trace ID from request header, if any
    #[instrument(skip(pool, metadata))]
    pub async fn log(
        pool: &PgPool,
        audit_schema: &str,
        action: &str,
        actor_id: &str,
        target_id: &str,
        metadata: Value,
        trace_id: Option<&str>,
    ) -> Result<()> {
        // The audit_log table is in `audit_t_<slug>` schema — we qualify fully.
        // Using `query` (not `query!`) because the schema name is dynamic.
        let sql = format!(
            r#"
            INSERT INTO {audit_schema}.audit_log
              (action, actor_id, target_id, metadata, trace_id)
            VALUES ($1, $2, $3, $4, $5)
            "#
        );

        sqlx::query(&sql)
            .bind(action)
            .bind(actor_id)
            .bind(target_id)
            .bind(&metadata)
            .bind(trace_id)
            .execute(pool)
            .await
            .map_err(|e| {
                warn!(
                    error = %e,
                    action = action,
                    "audit INSERT failed (non-fatal, continuing)"
                );
                anyhow::anyhow!("audit log insert: {e}")
            })?;

        Ok(())
    }
}
