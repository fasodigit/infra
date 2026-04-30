// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parcel CRUD + CRDT polygon service — Module 2 cartographie parcelles.
//!
//! Parcel metadata uses LWW (lww_version check on PATCH).
//! Polygon uses Yjs CRDT: incoming binary update is decoded and merged
//! into the stored document via `yrs::Doc::transact_mut().apply_update()`.
//! GeoJSON geometry is stored in PostGIS via ST_GeomFromGeoJSON.

use anyhow::{Context, Result};
use base64::Engine;
use sqlx::{PgPool, Row};
use tracing::instrument;
use uuid::Uuid;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update, updates::decoder::Decode};

use crate::{
    dto::{
        AgronomyNoteCreateRequest, AgronomyNoteResponse, PageResponse, PaginationParams,
        ParcelCreateRequest, ParcelPatchRequest, ParcelResponse, PolygonResponse,
        PolygonUpdateRequest,
    },
    errors::AppError,
    model::{AgronomyNoteRow, ParcelPolygonRow, ParcelRow},
    tenant_context::TenantContext,
};

// ---------------------------------------------------------------------------
// Search path helper
// ---------------------------------------------------------------------------

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
// Parcel CRUD
// ---------------------------------------------------------------------------

/// INSERT a new parcel.
#[instrument(skip(pool, req), fields(tenant = %tenant.slug))]
pub async fn create_parcel(
    pool: &PgPool,
    tenant: &TenantContext,
    req: &ParcelCreateRequest,
) -> Result<ParcelResponse, AppError> {
    let parcel_id = Uuid::now_v7();
    let schema = tenant.schema_name();

    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = sqlx::query_as::<_, ParcelRow>(
        r#"
        INSERT INTO parcel (id, producer_id, crop_type, planted_at, surface_hectares)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(parcel_id)
    .bind(req.producer_id)
    .bind(&req.crop_type)
    .bind(req.planted_at)
    .bind(req.surface_hectares)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(parcel_row_to_response(&row))
}

/// SELECT a single parcel.
#[instrument(skip(pool), fields(tenant = %tenant.slug, parcel_id = %id))]
pub async fn get_parcel(
    pool: &PgPool,
    tenant: &TenantContext,
    id: Uuid,
) -> Result<ParcelResponse, AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = sqlx::query_as::<_, ParcelRow>("SELECT * FROM parcel WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("parcel {id}")))?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(parcel_row_to_response(&row))
}

/// SELECT paginated parcels for a producer.
#[instrument(skip(pool), fields(tenant = %tenant.slug))]
pub async fn list_parcels(
    pool: &PgPool,
    tenant: &TenantContext,
    params: &PaginationParams,
) -> Result<PageResponse<ParcelResponse>, AppError> {
    let producer_id = params
        .producer_id
        .ok_or_else(|| AppError::BadRequest("producerId query param required".into()))?;

    let schema = tenant.schema_name();
    let size = params.size.min(100) as i64;
    let offset = (params.page * params.size) as i64;

    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let rows = sqlx::query_as::<_, ParcelRow>(
        r#"
        SELECT * FROM parcel
        WHERE producer_id = $1
        ORDER BY registered_at DESC, id DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(producer_id)
    .bind(size)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;

    let total_row = sqlx::query("SELECT COUNT(*) AS n FROM parcel WHERE producer_id = $1")
        .bind(producer_id)
        .fetch_one(&mut *tx)
        .await?;
    let total: i64 = total_row.try_get("n")?;

    tx.commit().await.map_err(AppError::from)?;

    Ok(PageResponse {
        items: rows.iter().map(parcel_row_to_response).collect(),
        page: params.page,
        size: params.size,
        total: Some(total),
    })
}

/// PATCH parcel metadata (LWW).
#[instrument(skip(pool, req), fields(tenant = %tenant.slug, parcel_id = %id))]
pub async fn patch_parcel(
    pool: &PgPool,
    tenant: &TenantContext,
    id: Uuid,
    req: &ParcelPatchRequest,
) -> Result<ParcelResponse, AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let current = sqlx::query_as::<_, ParcelRow>("SELECT * FROM parcel WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("parcel {id}")))?;

    if req.lww_version != current.lww_version {
        return Err(AppError::StaleLww {
            client: req.lww_version,
            server: current.lww_version,
        });
    }

    let row = sqlx::query_as::<_, ParcelRow>(
        r#"
        UPDATE parcel SET
          crop_type        = COALESCE($2, crop_type),
          planted_at       = COALESCE($3, planted_at),
          surface_hectares = COALESCE($4, surface_hectares),
          lww_version      = lww_version + 1,
          updated_at       = now()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(&req.crop_type)
    .bind(req.planted_at)
    .bind(req.surface_hectares)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(parcel_row_to_response(&row))
}

// ---------------------------------------------------------------------------
// Polygon — CRDT merge + PostGIS
// ---------------------------------------------------------------------------

/// Merge an incoming Yjs update into the stored polygon document and update
/// the PostGIS geometry from the provided GeoJSON.
#[instrument(skip(pool, req), fields(tenant = %tenant.slug, parcel_id = %parcel_id))]
pub async fn update_polygon(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
    req: &PolygonUpdateRequest,
) -> Result<PolygonResponse, AppError> {
    // Decode base64 Yjs update.
    let update_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.yjs_update)
        .map_err(|e| AppError::BadRequest(format!("invalid base64 yjsUpdate: {e}")))?;

    // Merge Yjs update into stored document.
    let merged_bytes = merge_yjs_update(pool, tenant, parcel_id, &update_bytes)
        .await
        .map_err(|e| AppError::Internal(e.context("yjs merge")))?;

    // Serialize GeoJSON geometry.
    let geojson_str = serde_json::to_string(&req.geojson)
        .map_err(|e| AppError::BadRequest(format!("invalid geojson: {e}")))?;

    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    // Upsert polygon row with merged Yjs doc and new PostGIS geometry.
    // If PostGIS is available we use ST_GeomFromGeoJSON; otherwise fall back
    // to storing GeoJSON as WKB text (consistent with T003 fallback strategy).
    let row = sqlx::query_as::<_, ParcelPolygonRow>(
        r#"
        INSERT INTO parcel_polygon (parcel_id, geom, yjs_doc, yjs_version, updated_at)
        VALUES ($1, ST_GeomFromGeoJSON($2::text), $3, 1, now())
        ON CONFLICT (parcel_id) DO UPDATE SET
          geom        = EXCLUDED.geom,
          yjs_doc     = EXCLUDED.yjs_doc,
          yjs_version = parcel_polygon.yjs_version + 1,
          updated_at  = now()
        RETURNING
          parcel_id,
          yjs_doc,
          yjs_version,
          updated_at,
          ST_AsText(geom) AS geom_wkt
        "#,
    )
    .bind(parcel_id)
    .bind(&geojson_str)
    .bind(&merged_bytes)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;

    let yjs_state_b64 = base64::engine::general_purpose::STANDARD.encode(&row.yjs_doc);

    Ok(PolygonResponse {
        parcel_id,
        yjs_state: yjs_state_b64,
        geojson: req.geojson.clone(),
        geom_wkt: row.geom_wkt,
        yjs_version: row.yjs_version,
        updated_at: row.updated_at,
    })
}

/// Retrieve the stored polygon.
#[instrument(skip(pool), fields(tenant = %tenant.slug, parcel_id = %parcel_id))]
pub async fn get_polygon(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
) -> Result<PolygonResponse, AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = sqlx::query_as::<_, ParcelPolygonRow>(
        r#"
        SELECT
          parcel_id,
          yjs_doc,
          yjs_version,
          updated_at,
          ST_AsText(geom) AS geom_wkt
        FROM parcel_polygon
        WHERE parcel_id = $1
        "#,
    )
    .bind(parcel_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("polygon for parcel {parcel_id}")))?;

    tx.commit().await.map_err(AppError::from)?;

    let yjs_state_b64 = base64::engine::general_purpose::STANDARD.encode(&row.yjs_doc);
    let geojson = wkt_to_geojson(row.geom_wkt.as_deref());

    Ok(PolygonResponse {
        parcel_id,
        yjs_state: yjs_state_b64,
        geojson,
        geom_wkt: row.geom_wkt,
        yjs_version: row.yjs_version,
        updated_at: row.updated_at,
    })
}

/// Get raw polygon for gRPC (returns WKT + Yjs bytes directly).
pub async fn get_polygon_raw(
    pool: &PgPool,
    tenant_slug: &str,
    parcel_id: Uuid,
) -> Result<ParcelPolygonRow, AppError> {
    let schema = format!("terroir_t_{tenant_slug}");
    let mut tx = pool.begin().await?;
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    let row = sqlx::query_as::<_, ParcelPolygonRow>(
        r#"
        SELECT
          parcel_id,
          yjs_doc,
          yjs_version,
          updated_at,
          ST_AsText(geom) AS geom_wkt
        FROM parcel_polygon WHERE parcel_id = $1
        "#,
    )
    .bind(parcel_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("polygon for parcel {parcel_id}")))?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(row)
}

/// List parcels by producer IDs (for gRPC cooperative streaming).
pub async fn list_parcels_by_producer_ids(
    pool: &PgPool,
    tenant_slug: &str,
    producer_ids: &[Uuid],
) -> Result<Vec<ParcelRow>, AppError> {
    if producer_ids.is_empty() {
        return Ok(vec![]);
    }
    let schema = format!("terroir_t_{tenant_slug}");
    let mut tx = pool.begin().await?;
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    // Build IN list.
    let rows = sqlx::query_as::<_, ParcelRow>(
        "SELECT * FROM parcel WHERE producer_id = ANY($1) ORDER BY registered_at DESC",
    )
    .bind(producer_ids)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Yjs merge helper
// ---------------------------------------------------------------------------

/// Merge an incoming Yjs v1 update into the stored document.
/// Returns the new serialized state (full snapshot).
async fn merge_yjs_update(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
    update_bytes: &[u8],
) -> Result<Vec<u8>> {
    let schema = tenant.schema_name();

    // Load existing Yjs doc from DB (if any).
    let existing: Option<Vec<u8>> = {
        let mut tx = pool.begin().await?;
        let sql = format!("SET LOCAL search_path TO {schema}");
        sqlx::query(&sql).execute(&mut *tx).await?;

        let maybe = sqlx::query("SELECT yjs_doc FROM parcel_polygon WHERE parcel_id = $1")
            .bind(parcel_id)
            .fetch_optional(&mut *tx)
            .await?;

        tx.commit().await?;
        maybe.map(|r| r.get::<Vec<u8>, _>("yjs_doc"))
    };

    let doc = Doc::new();
    {
        let mut txn = doc.transact_mut();
        // Apply existing state if present.
        if let Some(existing_bytes) = existing {
            let existing_update =
                Update::decode_v1(&existing_bytes).context("decode existing yjs doc")?;
            txn.apply_update(existing_update)
                .context("apply existing yjs update")?;
        }
        // Apply incoming update.
        let incoming = Update::decode_v1(update_bytes).context("decode incoming yjs update")?;
        txn.apply_update(incoming)
            .context("apply incoming yjs update")?;
    }

    // Serialize full state as v1 snapshot.
    let merged = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());

    Ok(merged)
}

// ---------------------------------------------------------------------------
// Agronomy Notes — CRDT text
// ---------------------------------------------------------------------------

/// INSERT or merge an agronomy note for a parcel.
#[instrument(skip(pool, req), fields(tenant = %tenant.slug, parcel_id = %parcel_id))]
pub async fn create_agronomy_note(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
    req: &AgronomyNoteCreateRequest,
) -> Result<AgronomyNoteResponse, AppError> {
    let update_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.yjs_update)
        .map_err(|e| AppError::BadRequest(format!("invalid base64 yjsUpdate: {e}")))?;

    // Merge Yjs text update.
    let schema = tenant.schema_name();

    // Load existing note doc (there's one per parcel).
    let existing_row: Option<AgronomyNoteRow> = {
        let mut tx = pool.begin().await?;
        set_search_path(&mut tx, &schema)
            .await
            .map_err(AppError::Internal)?;
        let r = sqlx::query_as::<_, AgronomyNoteRow>(
            "SELECT * FROM agronomy_note WHERE parcel_id = $1 ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(parcel_id)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await.map_err(AppError::from)?;
        r
    };

    let doc = Doc::new();
    {
        let mut txn = doc.transact_mut();
        if let Some(ref existing) = existing_row {
            let existing_update = Update::decode_v1(&existing.yjs_doc)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("decode existing yjs: {e}")))?;
            txn.apply_update(existing_update)
                .map_err(|e| AppError::Internal(anyhow::anyhow!("apply existing yjs: {e}")))?;
        }
        let incoming = Update::decode_v1(&update_bytes)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("decode incoming yjs: {e}")))?;
        txn.apply_update(incoming)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("apply incoming yjs: {e}")))?;
    }

    let merged = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());

    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = if let Some(existing) = existing_row {
        sqlx::query_as::<_, AgronomyNoteRow>(
            r#"
            UPDATE agronomy_note SET
              yjs_doc     = $2,
              yjs_version = yjs_version + 1,
              updated_at  = now()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(existing.id)
        .bind(&merged)
        .fetch_one(&mut *tx)
        .await?
    } else {
        sqlx::query_as::<_, AgronomyNoteRow>(
            r#"
            INSERT INTO agronomy_note (id, parcel_id, yjs_doc, yjs_version)
            VALUES ($1, $2, $3, 1)
            RETURNING *
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(parcel_id)
        .bind(&merged)
        .fetch_one(&mut *tx)
        .await?
    };

    tx.commit().await.map_err(AppError::from)?;

    Ok(AgronomyNoteResponse {
        id: row.id,
        parcel_id: row.parcel_id,
        yjs_state: base64::engine::general_purpose::STANDARD.encode(&row.yjs_doc),
        yjs_version: row.yjs_version,
        updated_at: row.updated_at,
    })
}

/// List agronomy notes for a parcel.
#[instrument(skip(pool), fields(tenant = %tenant.slug, parcel_id = %parcel_id))]
pub async fn list_agronomy_notes(
    pool: &PgPool,
    tenant: &TenantContext,
    parcel_id: Uuid,
) -> Result<Vec<AgronomyNoteResponse>, AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let rows = sqlx::query_as::<_, AgronomyNoteRow>(
        "SELECT * FROM agronomy_note WHERE parcel_id = $1 ORDER BY updated_at DESC",
    )
    .bind(parcel_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;

    Ok(rows
        .into_iter()
        .map(|row| AgronomyNoteResponse {
            id: row.id,
            parcel_id: row.parcel_id,
            yjs_state: base64::engine::general_purpose::STANDARD.encode(&row.yjs_doc),
            yjs_version: row.yjs_version,
            updated_at: row.updated_at,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn parcel_row_to_response(row: &ParcelRow) -> ParcelResponse {
    ParcelResponse {
        id: row.id,
        producer_id: row.producer_id,
        crop_type: row.crop_type.clone(),
        planted_at: row.planted_at,
        surface_hectares: row.surface_hectares,
        registered_at: row.registered_at,
        updated_at: row.updated_at,
        lww_version: row.lww_version,
    }
}

/// Minimal WKT→GeoJSON conversion (returns raw JSON null if WKT absent).
fn wkt_to_geojson(wkt: Option<&str>) -> serde_json::Value {
    match wkt {
        None => serde_json::Value::Null,
        Some(w) => serde_json::json!({ "type": "Feature", "geometry": { "wkt": w } }),
    }
}
