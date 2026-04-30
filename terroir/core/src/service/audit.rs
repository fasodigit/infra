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
        // Schema columns: tenant_slug, actor_id (uuid), action, resource_id (uuid),
        // metadata (jsonb), trace_id. Cf. T100__audit_log.sql.tmpl.
        // actor_id and resource_id are uuid; cast text→uuid in SQL when input is
        // a string. For "anonymous" or non-uuid actors we keep NULL.
        let sql = format!(
            r#"
            INSERT INTO {audit_schema}.audit_log
              (tenant_slug, action, actor_id, resource_id, metadata, trace_id)
            VALUES ($1, $2, NULLIF($3, '')::uuid, NULLIF($4, '')::uuid, $5, $6)
            "#
        );

        let tenant_slug = audit_schema.strip_prefix("audit_t_").unwrap_or(audit_schema);
        let actor_uuid = if actor_id == "anonymous" { "" } else { actor_id };

        sqlx::query(&sql)
            .bind(tenant_slug)
            .bind(action)
            .bind(actor_uuid)
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
