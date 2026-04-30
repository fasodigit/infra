// SPDX-License-Identifier: AGPL-3.0-or-later
//! sqlx persistence layer for terroir-eudr.
//!
//! Tables in `terroir_t_<slug>`:
//!   - `eudr_validation` (append-only)
//!   - `dds`             (ACID with status transitions)
//!   - `dds_submission`  (append-only)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::dto::{ValidationResponse, ValidationStatus};
use crate::tenant_context::TenantContext;

async fn set_search_path(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    schema: &str,
) -> Result<()> {
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut **tx)
        .await
        .with_context(|| format!("SET LOCAL search_path TO {schema}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// eudr_validation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EudrValidationRow {
    pub id: Uuid,
    pub parcel_id: Uuid,
    pub status: String,
    pub polygon_hash: String,
    pub deforestation_overlap_ha: f64,
    pub dataset_version: String,
    pub evidence_url: Option<String>,
    pub hansen_pixels_loss: i32,
    pub jrc_pixels_loss: i32,
    pub reason: Option<String>,
    pub computed_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_validation(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
    status: ValidationStatus,
    polygon_hash: &str,
    overlap_ha: f64,
    dataset_version: &str,
    evidence_url: Option<&str>,
    hansen_pixels: i32,
    jrc_pixels: i32,
    reason: Option<&str>,
) -> Result<EudrValidationRow> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    let id = Uuid::now_v7();
    let now = Utc::now();
    sqlx::query(
        r#"INSERT INTO eudr_validation
           (id, parcel_id, status, polygon_hash, deforestation_overlap_ha,
            dataset_version, evidence_url, hansen_pixels_loss, jrc_pixels_loss,
            reason, computed_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
    )
    .bind(id)
    .bind(parcel_id)
    .bind(status.as_db_str())
    .bind(polygon_hash)
    .bind(overlap_ha)
    .bind(dataset_version)
    .bind(evidence_url)
    .bind(hansen_pixels)
    .bind(jrc_pixels)
    .bind(reason)
    .bind(now)
    .execute(&mut *tx)
    .await
    .context("INSERT eudr_validation")?;

    tx.commit().await?;

    Ok(EudrValidationRow {
        id,
        parcel_id,
        status: status.as_db_str().to_owned(),
        polygon_hash: polygon_hash.to_owned(),
        deforestation_overlap_ha: overlap_ha,
        dataset_version: dataset_version.to_owned(),
        evidence_url: evidence_url.map(ToOwned::to_owned),
        hansen_pixels_loss: hansen_pixels,
        jrc_pixels_loss: jrc_pixels,
        reason: reason.map(ToOwned::to_owned),
        computed_at: now,
    })
}

pub async fn list_validations(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
) -> Result<Vec<ValidationResponse>> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    let rows = sqlx::query(
        r#"SELECT id, parcel_id, status, polygon_hash, deforestation_overlap_ha,
                  dataset_version, evidence_url, computed_at
           FROM eudr_validation
           WHERE parcel_id = $1
           ORDER BY computed_at DESC"#,
    )
    .bind(parcel_id)
    .fetch_all(&mut *tx)
    .await
    .context("SELECT eudr_validation list")?;

    tx.commit().await?;

    let out = rows
        .into_iter()
        .map(|r| ValidationResponse {
            validation_id: r.get("id"),
            parcel_id: r.get("parcel_id"),
            status: ValidationStatus::from_db_str(r.get::<&str, _>("status")),
            evidence_url: r.get("evidence_url"),
            dds_draft_id: None,
            deforestation_overlap_ha: r.get("deforestation_overlap_ha"),
            dataset_version: r.get("dataset_version"),
            polygon_hash: r.get("polygon_hash"),
            cache_status: String::new(),
            computed_at: r.get("computed_at"),
        })
        .collect();

    Ok(out)
}

pub async fn get_validation(
    pool: &PgPool,
    tenant: &TenantContext,
    validation_id: Uuid,
) -> Result<Option<EudrValidationRow>> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    let row = sqlx::query(
        r#"SELECT id, parcel_id, status, polygon_hash, deforestation_overlap_ha,
                  dataset_version, evidence_url, hansen_pixels_loss,
                  jrc_pixels_loss, reason, computed_at
           FROM eudr_validation WHERE id = $1"#,
    )
    .bind(validation_id)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(row.map(|r| EudrValidationRow {
        id: r.get("id"),
        parcel_id: r.get("parcel_id"),
        status: r.get("status"),
        polygon_hash: r.get("polygon_hash"),
        deforestation_overlap_ha: r.get("deforestation_overlap_ha"),
        dataset_version: r.get("dataset_version"),
        evidence_url: r.get("evidence_url"),
        hansen_pixels_loss: r.get("hansen_pixels_loss"),
        jrc_pixels_loss: r.get("jrc_pixels_loss"),
        reason: r.get("reason"),
        computed_at: r.get("computed_at"),
    }))
}

// ---------------------------------------------------------------------------
// dds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DdsRow {
    pub id: Uuid,
    pub validation_id: Uuid,
    pub parcel_id: Uuid,
    pub status: String,
    pub operator_eori: Option<String>,
    pub hs_code: Option<String>,
    pub country_iso2: Option<String>,
    pub payload_json: serde_json::Value,
    pub payload_sha256: String,
    pub signature_fingerprint: Option<String>,
    pub evidence_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub lww_version: i64,
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_dds(
    pool: &PgPool,
    tenant: &TenantContext,
    validation_id: Uuid,
    parcel_id: Uuid,
    operator_eori: &str,
    hs_code: &str,
    quantity: f64,
    unit: &str,
    country_iso2: &str,
    harvest_period: &str,
    payload: serde_json::Value,
    payload_sha256: &str,
    evidence_url: Option<&str>,
) -> Result<DdsRow> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    let id = Uuid::now_v7();
    let now = Utc::now();

    sqlx::query(
        r#"INSERT INTO dds
           (id, validation_id, parcel_id, status, operator_eori, hs_code,
            quantity, unit, country_iso2, harvest_period, payload_json,
            payload_sha256, evidence_url, created_at, updated_at, lww_version)
           VALUES ($1,$2,$3,'draft',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$13,1)"#,
    )
    .bind(id)
    .bind(validation_id)
    .bind(parcel_id)
    .bind(operator_eori)
    .bind(hs_code)
    .bind(quantity)
    .bind(unit)
    .bind(country_iso2)
    .bind(harvest_period)
    .bind(&payload)
    .bind(payload_sha256)
    .bind(evidence_url)
    .bind(now)
    .execute(&mut *tx)
    .await
    .context("INSERT dds")?;

    tx.commit().await?;

    Ok(DdsRow {
        id,
        validation_id,
        parcel_id,
        status: "draft".into(),
        operator_eori: Some(operator_eori.to_owned()),
        hs_code: Some(hs_code.to_owned()),
        country_iso2: Some(country_iso2.to_owned()),
        payload_json: payload,
        payload_sha256: payload_sha256.to_owned(),
        signature_fingerprint: None,
        evidence_url: evidence_url.map(ToOwned::to_owned),
        created_at: now,
        lww_version: 1,
    })
}

pub async fn get_dds(
    pool: &PgPool,
    tenant: &TenantContext,
    dds_id: Uuid,
) -> Result<Option<DdsRow>> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    let row = sqlx::query(
        r#"SELECT id, validation_id, parcel_id, status, operator_eori, hs_code,
                  country_iso2, payload_json, payload_sha256, signature_fingerprint,
                  evidence_url, created_at, lww_version
           FROM dds WHERE id = $1"#,
    )
    .bind(dds_id)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(row.map(|r| DdsRow {
        id: r.get("id"),
        validation_id: r.get("validation_id"),
        parcel_id: r.get("parcel_id"),
        status: r.get("status"),
        operator_eori: r.get("operator_eori"),
        hs_code: r.get("hs_code"),
        country_iso2: r.get("country_iso2"),
        payload_json: r.get("payload_json"),
        payload_sha256: r.get("payload_sha256"),
        signature_fingerprint: r.get("signature_fingerprint"),
        evidence_url: r.get("evidence_url"),
        created_at: r.get("created_at"),
        lww_version: r.get("lww_version"),
    }))
}

pub async fn update_dds_signature(
    pool: &PgPool,
    tenant: &TenantContext,
    dds_id: Uuid,
    fingerprint: &str,
    cert_pem: &str,
) -> Result<()> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    sqlx::query(
        r#"UPDATE dds
           SET status = 'signed',
               signature_fingerprint = $2,
               signature_cert_pem = $3,
               updated_at = now(),
               lww_version = lww_version + 1
           WHERE id = $1"#,
    )
    .bind(dds_id)
    .bind(fingerprint)
    .bind(cert_pem)
    .execute(&mut *tx)
    .await
    .context("UPDATE dds signature")?;

    tx.commit().await?;
    Ok(())
}

pub async fn update_dds_status(
    pool: &PgPool,
    tenant: &TenantContext,
    dds_id: Uuid,
    new_status: &str,
) -> Result<()> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    sqlx::query(
        r#"UPDATE dds
           SET status = $2, updated_at = now(), lww_version = lww_version + 1
           WHERE id = $1"#,
    )
    .bind(dds_id)
    .bind(new_status)
    .execute(&mut *tx)
    .await
    .context("UPDATE dds status")?;

    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// dds_submission
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn insert_submission(
    pool: &PgPool,
    tenant: &TenantContext,
    dds_id: Uuid,
    attempt_no: i32,
    outcome: &str,
    traces_nt_ref: Option<&str>,
    http_status: Option<i32>,
    response_body: Option<&str>,
) -> Result<()> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    sqlx::query(
        r#"INSERT INTO dds_submission
           (id, dds_id, attempt_no, outcome, traces_nt_ref, http_status,
            response_body, submitted_at)
           VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,now())"#,
    )
    .bind(dds_id)
    .bind(attempt_no)
    .bind(outcome)
    .bind(traces_nt_ref)
    .bind(http_status)
    .bind(response_body)
    .execute(&mut *tx)
    .await
    .context("INSERT dds_submission")?;

    tx.commit().await?;
    Ok(())
}

pub async fn next_attempt_no(pool: &PgPool, tenant: &TenantContext, dds_id: Uuid) -> Result<i32> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema).await?;

    let row = sqlx::query(
        r#"SELECT COALESCE(MAX(attempt_no), 0) + 1 AS next FROM dds_submission WHERE dds_id = $1"#,
    )
    .bind(dds_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(row.get::<i32, _>("next"))
}
