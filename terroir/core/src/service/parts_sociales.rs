// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parts sociales service — Module 1, ACID strategy.
//!
//! Capital coopératif : mutations are strictly transactional with
//! SERIALIZABLE isolation. No LWW merge — conflict = rejection.

use anyhow::Result;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    dto::{PartsSocialesCreateRequest, PartsSocialesResponse},
    errors::AppError,
    model::PartsSocialesRow,
    tenant_context::TenantContext,
};

async fn set_search_path(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    schema: &str,
) -> Result<()> {
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut **tx)
        .await
        .map_err(|e| anyhow::anyhow!("SET LOCAL search_path: {e}"))?;
    Ok(())
}

fn row_to_response(row: &PartsSocialesRow) -> PartsSocialesResponse {
    PartsSocialesResponse {
        id: row.id,
        producer_id: row.producer_id,
        cooperative_id: row.cooperative_id,
        nb_parts: row.nb_parts,
        valeur_nominale_xof: row.valeur_nominale_xof,
        adhesion_date: row.adhesion_date,
        ag_reference: row.ag_reference.clone(),
        registered_at: row.registered_at,
        updated_at: row.updated_at,
        lww_version: row.lww_version,
    }
}

/// INSERT a new parts_sociales record.
/// Uses SERIALIZABLE isolation (ACID per ADR-002).
#[instrument(skip(pool, req), fields(tenant = %tenant.slug))]
pub async fn create_parts_sociales(
    pool: &PgPool,
    tenant: &TenantContext,
    req: &PartsSocialesCreateRequest,
) -> Result<PartsSocialesResponse, AppError> {
    let id = Uuid::now_v7();
    let schema = tenant.schema_name();

    let mut tx = pool.begin().await.map_err(AppError::from)?;

    // Upgrade to SERIALIZABLE for ACID capital operations.
    sqlx::query("SET LOCAL TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    // Check uniqueness (producer_id, cooperative_id) — the DB constraint
    // also enforces this but we return a clean 409 instead of a DB error.
    let exists =
        sqlx::query("SELECT 1 FROM parts_sociales WHERE producer_id = $1 AND cooperative_id = $2")
            .bind(req.producer_id)
            .bind(req.cooperative_id)
            .fetch_optional(&mut *tx)
            .await?;

    if exists.is_some() {
        return Err(AppError::Conflict(format!(
            "parts_sociales already exists for producer {} in cooperative {}",
            req.producer_id, req.cooperative_id
        )));
    }

    let row = sqlx::query_as::<_, PartsSocialesRow>(
        r#"
        INSERT INTO parts_sociales
          (id, producer_id, cooperative_id, nb_parts, valeur_nominale_xof, adhesion_date, ag_reference)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(req.producer_id)
    .bind(req.cooperative_id)
    .bind(req.nb_parts)
    .bind(req.valeur_nominale_xof)
    .bind(req.adhesion_date)
    .bind(&req.ag_reference)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(row_to_response(&row))
}
