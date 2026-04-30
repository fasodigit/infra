// SPDX-License-Identifier: AGPL-3.0-or-later
//! Household CRDT service — Module 1.
//!
//! A household groups producers sharing resources. Its composition is
//! encoded as a Yjs document (member list + roles).

use anyhow::Result;
use base64::Engine;
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update, updates::decoder::Decode};

use crate::{
    dto::{HouseholdCreateRequest, HouseholdResponse},
    errors::AppError,
    model::HouseholdRow,
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

fn row_to_response(row: &HouseholdRow) -> HouseholdResponse {
    HouseholdResponse {
        id: row.id,
        cooperative_id: row.cooperative_id,
        head_producer_id: row.head_producer_id,
        yjs_state: base64::engine::general_purpose::STANDARD.encode(&row.yjs_doc),
        yjs_version: row.yjs_version,
        registered_at: row.registered_at,
        updated_at: row.updated_at,
    }
}

/// INSERT a new household with optional initial Yjs update.
#[instrument(skip(pool, req), fields(tenant = %tenant.slug))]
pub async fn create_household(
    pool: &PgPool,
    tenant: &TenantContext,
    req: &HouseholdCreateRequest,
) -> Result<HouseholdResponse, AppError> {
    let household_id = Uuid::now_v7();
    let schema = tenant.schema_name();

    // Build initial Yjs doc.
    let initial_bytes: Vec<u8> = match &req.yjs_update {
        Some(b64) => {
            let update_bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| AppError::BadRequest(format!("invalid base64 yjsUpdate: {e}")))?;
            let doc = Doc::new();
            {
                let mut txn = doc.transact_mut();
                let update = Update::decode_v1(&update_bytes)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("decode yjs update: {e}")))?;
                txn.apply_update(update)
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("apply yjs update: {e}")))?;
            }
            doc.transact()
                .encode_state_as_update_v1(&StateVector::default())
        }
        None => {
            // Empty doc — serialize empty state.
            let doc = Doc::new();
            doc.transact()
                .encode_state_as_update_v1(&StateVector::default())
        }
    };

    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = sqlx::query_as::<_, HouseholdRow>(
        r#"
        INSERT INTO household (id, cooperative_id, head_producer_id, yjs_doc, yjs_version)
        VALUES ($1, $2, $3, $4, 1)
        RETURNING *
        "#,
    )
    .bind(household_id)
    .bind(req.cooperative_id)
    .bind(req.head_producer_id)
    .bind(&initial_bytes)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(row_to_response(&row))
}
