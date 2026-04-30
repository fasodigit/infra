// SPDX-License-Identifier: AGPL-3.0-or-later
//! Producer CRUD service — Module 1 registre membres.
//!
//! All PII fields are encrypted via Vault Transit before INSERT and
//! decrypted on read. LWW version is checked on PATCH.
//!
//! Tenant isolation: every query begins with a SET LOCAL to bind the
//! search_path to the correct tenant schema. Queries run inside a
//! transaction so that SET LOCAL is scoped correctly with pgbouncer.

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::{
    dto::{
        PageResponse, PaginationParams, ProducerCreateRequest, ProducerPatchRequest,
        ProducerResponse,
    },
    errors::AppError,
    model::ProducerRow,
    service::vault_transit::{EncryptedPii, VaultTransitService, pii_context},
    tenant_context::TenantContext,
};

// ---------------------------------------------------------------------------
// Tenant SET LOCAL helper
// ---------------------------------------------------------------------------

/// Execute `SET LOCAL search_path TO <schema>` inside a transaction.
/// Must be called at the start of every query transaction.
async fn set_search_path(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    schema: &str,
) -> Result<()> {
    // Build the SQL string directly — the schema name is validated upstream
    // via `is_valid_slug` so injection is not possible.
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut **tx)
        .await
        .with_context(|| format!("SET LOCAL search_path TO {schema}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Encryption helpers
// ---------------------------------------------------------------------------

/// Encrypt all 6 PII fields for a producer.
async fn encrypt_producer_pii(
    vault: &VaultTransitService,
    tenant: &TenantContext,
    producer_id: &Uuid,
    req: &ProducerCreateRequest,
) -> Result<ProducerPii> {
    let ctx_for = |field: &str| pii_context(&tenant.slug, field, producer_id);

    let full_name = vault.encrypt(&req.full_name, &ctx_for("full_name")).await?;
    let nin = vault.encrypt(&req.nin, &ctx_for("nin")).await?;
    let phone = vault.encrypt(&req.phone, &ctx_for("phone")).await?;
    let lat = vault
        .encrypt(
            &req.gps_domicile_lat.to_string(),
            &ctx_for("gps_domicile_lat"),
        )
        .await?;
    let lon = vault
        .encrypt(
            &req.gps_domicile_lon.to_string(),
            &ctx_for("gps_domicile_lon"),
        )
        .await?;
    let photo_url = match &req.photo_url {
        Some(url) => Some(vault.encrypt(url, &ctx_for("photo_url")).await?),
        None => None,
    };

    Ok(ProducerPii {
        full_name,
        nin,
        phone,
        lat,
        lon,
        photo_url,
    })
}

struct ProducerPii {
    full_name: EncryptedPii,
    nin: EncryptedPii,
    phone: EncryptedPii,
    lat: EncryptedPii,
    lon: EncryptedPii,
    photo_url: Option<EncryptedPii>,
}

// ---------------------------------------------------------------------------
// Decryption helpers
// ---------------------------------------------------------------------------

/// Decrypt a single optional bytea field.
async fn decrypt_field(
    vault: &VaultTransitService,
    bytes: &Option<Vec<u8>>,
    context: &str,
) -> Result<String> {
    match bytes {
        Some(b) => vault.decrypt(b, context).await,
        None => Ok(String::new()),
    }
}

/// Decrypt all PII fields from a `ProducerRow`.
async fn decrypt_producer_pii(
    vault: &VaultTransitService,
    tenant: &TenantContext,
    row: &ProducerRow,
) -> Result<DecryptedPii> {
    let full_name = decrypt_field(
        vault,
        &row.full_name_encrypted,
        &pii_context(&tenant.slug, "full_name", &row.id),
    )
    .await?;
    let nin = decrypt_field(
        vault,
        &row.nin_encrypted,
        &pii_context(&tenant.slug, "nin", &row.id),
    )
    .await?;
    let phone = decrypt_field(
        vault,
        &row.phone_encrypted,
        &pii_context(&tenant.slug, "phone", &row.id),
    )
    .await?;
    let lat_str = decrypt_field(
        vault,
        &row.gps_domicile_lat_encrypted,
        &pii_context(&tenant.slug, "gps_domicile_lat", &row.id),
    )
    .await?;
    let lon_str = decrypt_field(
        vault,
        &row.gps_domicile_lon_encrypted,
        &pii_context(&tenant.slug, "gps_domicile_lon", &row.id),
    )
    .await?;
    let photo_url = decrypt_field(
        vault,
        &row.photo_url_encrypted,
        &pii_context(&tenant.slug, "photo_url", &row.id),
    )
    .await?;

    let lat: f64 = if lat_str.is_empty() {
        0.0
    } else {
        lat_str.parse().context("parse lat")?
    };
    let lon: f64 = if lon_str.is_empty() {
        0.0
    } else {
        lon_str.parse().context("parse lon")?
    };

    Ok(DecryptedPii {
        full_name,
        nin,
        phone,
        lat,
        lon,
        photo_url: if photo_url.is_empty() {
            None
        } else {
            Some(photo_url)
        },
    })
}

struct DecryptedPii {
    full_name: String,
    nin: String,
    phone: String,
    lat: f64,
    lon: f64,
    photo_url: Option<String>,
}

fn to_response(row: &ProducerRow, pii: DecryptedPii) -> ProducerResponse {
    ProducerResponse {
        id: row.id,
        cooperative_id: row.cooperative_id,
        external_id: row.external_id.clone(),
        full_name: pii.full_name,
        nin: pii.nin,
        phone: pii.phone,
        photo_url: pii.photo_url,
        gps_domicile_lat: pii.lat,
        gps_domicile_lon: pii.lon,
        household_id: row.household_id,
        primary_crop: row.primary_crop.clone(),
        registered_at: row.registered_at,
        updated_at: row.updated_at,
        lww_version: row.lww_version,
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// INSERT a new producer with encrypted PII.
#[instrument(skip(pool, vault, req), fields(tenant = %tenant.slug))]
pub async fn create_producer(
    pool: &PgPool,
    vault: &VaultTransitService,
    tenant: &TenantContext,
    req: &ProducerCreateRequest,
) -> Result<ProducerResponse, AppError> {
    let producer_id = Uuid::now_v7();
    let schema = tenant.schema_name();

    let pii = encrypt_producer_pii(vault, tenant, &producer_id, req)
        .await
        .map_err(|e| AppError::Internal(e.context("encrypt producer PII")))?;

    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = sqlx::query_as::<_, ProducerRow>(
        r#"
        INSERT INTO producer (
          id, cooperative_id, external_id,
          full_name_encrypted, full_name_dek_kid,
          nin_encrypted,       nin_dek_kid,
          phone_encrypted,     phone_dek_kid,
          photo_url_encrypted, photo_url_dek_kid,
          gps_domicile_lat_encrypted, gps_domicile_lat_dek_kid,
          gps_domicile_lon_encrypted, gps_domicile_lon_dek_kid,
          household_id, primary_crop, lww_version
        ) VALUES (
          $1,  $2,  $3,
          $4,  $5,
          $6,  $7,
          $8,  $9,
          $10, $11,
          $12, $13,
          $14, $15,
          $16, $17, 1
        )
        RETURNING *
        "#,
    )
    .bind(producer_id)
    .bind(req.cooperative_id)
    .bind(&req.external_id)
    .bind(&pii.full_name.ciphertext_bytes)
    .bind(&pii.full_name.kid)
    .bind(&pii.nin.ciphertext_bytes)
    .bind(&pii.nin.kid)
    .bind(&pii.phone.ciphertext_bytes)
    .bind(&pii.phone.kid)
    .bind(pii.photo_url.as_ref().map(|p| &p.ciphertext_bytes))
    .bind(pii.photo_url.as_ref().map(|p| &p.kid))
    .bind(&pii.lat.ciphertext_bytes)
    .bind(&pii.lat.kid)
    .bind(&pii.lon.ciphertext_bytes)
    .bind(&pii.lon.kid)
    .bind(req.household_id)
    .bind(&req.primary_crop)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;

    let decrypted = decrypt_producer_pii(vault, tenant, &row)
        .await
        .map_err(|e| AppError::Internal(e.context("decrypt after insert")))?;

    debug!(producer_id = %producer_id, "producer created");
    Ok(to_response(&row, decrypted))
}

/// SELECT a single producer by id (excludes soft-deleted).
#[instrument(skip(pool, vault), fields(tenant = %tenant.slug, producer_id = %id))]
pub async fn get_producer(
    pool: &PgPool,
    vault: &VaultTransitService,
    tenant: &TenantContext,
    id: Uuid,
) -> Result<ProducerResponse, AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let row = sqlx::query_as::<_, ProducerRow>(
        "SELECT * FROM producer WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("producer {id}")))?;

    tx.commit().await.map_err(AppError::from)?;

    let decrypted = decrypt_producer_pii(vault, tenant, &row)
        .await
        .map_err(|e| AppError::Internal(e.context("decrypt producer")))?;

    Ok(to_response(&row, decrypted))
}

/// SELECT paginated producers for a cooperative.
#[instrument(skip(pool, vault), fields(tenant = %tenant.slug))]
pub async fn list_producers(
    pool: &PgPool,
    vault: &VaultTransitService,
    tenant: &TenantContext,
    params: &PaginationParams,
) -> Result<PageResponse<ProducerResponse>, AppError> {
    let schema = tenant.schema_name();
    // cooperativeId is optional — when absent, lists all producers in this
    // tenant schema (RLS already isolates per tenant). This keeps tenant-
    // isolation E2E spec working (it lists without filter from tenant B).
    let size = params.size.min(100) as i64;
    let offset = (params.page * params.size) as i64;

    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let rows = sqlx::query_as::<_, ProducerRow>(
        r#"
        SELECT * FROM producer
        WHERE deleted_at IS NULL
          AND ($1::uuid IS NULL OR cooperative_id = $1)
        ORDER BY registered_at DESC, id DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(params.cooperative_id)
    .bind(size)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;

    let total_row = sqlx::query(
        "SELECT COUNT(*) AS n FROM producer
         WHERE deleted_at IS NULL
           AND ($1::uuid IS NULL OR cooperative_id = $1)",
    )
    .bind(params.cooperative_id)
    .fetch_one(&mut *tx)
    .await?;
    let total: i64 = total_row.try_get("n")?;

    tx.commit().await.map_err(AppError::from)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        let pii = decrypt_producer_pii(vault, tenant, row)
            .await
            .map_err(|e| AppError::Internal(e.context("decrypt list")))?;
        items.push(to_response(row, pii));
    }

    Ok(PageResponse {
        items,
        page: params.page,
        size: params.size,
        total: Some(total),
    })
}

/// PATCH a producer (LWW — reject if client version is stale).
#[instrument(skip(pool, vault, req), fields(tenant = %tenant.slug, producer_id = %id))]
pub async fn patch_producer(
    pool: &PgPool,
    vault: &VaultTransitService,
    tenant: &TenantContext,
    id: Uuid,
    req: &ProducerPatchRequest,
) -> Result<ProducerResponse, AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    // Fetch current row to check LWW version.
    let current = sqlx::query_as::<_, ProducerRow>(
        "SELECT * FROM producer WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("producer {id}")))?;

    if req.lww_version != current.lww_version {
        return Err(AppError::StaleLww {
            client: req.lww_version,
            server: current.lww_version,
        });
    }

    // Encrypt only fields that are present in the patch.
    let ctx_for = |field: &str| pii_context(&tenant.slug, field, &id);

    let full_name_enc = match &req.full_name {
        Some(v) => Some(
            vault
                .encrypt(v, &ctx_for("full_name"))
                .await
                .map_err(|e| AppError::Internal(e.context("encrypt full_name")))?,
        ),
        None => None,
    };
    let nin_enc = match &req.nin {
        Some(v) => Some(
            vault
                .encrypt(v, &ctx_for("nin"))
                .await
                .map_err(|e| AppError::Internal(e.context("encrypt nin")))?,
        ),
        None => None,
    };
    let phone_enc = match &req.phone {
        Some(v) => Some(
            vault
                .encrypt(v, &ctx_for("phone"))
                .await
                .map_err(|e| AppError::Internal(e.context("encrypt phone")))?,
        ),
        None => None,
    };
    let photo_enc = match &req.photo_url {
        Some(v) => Some(
            vault
                .encrypt(v, &ctx_for("photo_url"))
                .await
                .map_err(|e| AppError::Internal(e.context("encrypt photo_url")))?,
        ),
        None => None,
    };
    let lat_enc = match &req.gps_domicile_lat {
        Some(v) => Some(
            vault
                .encrypt(&v.to_string(), &ctx_for("gps_domicile_lat"))
                .await
                .map_err(|e| AppError::Internal(e.context("encrypt lat")))?,
        ),
        None => None,
    };
    let lon_enc = match &req.gps_domicile_lon {
        Some(v) => Some(
            vault
                .encrypt(&v.to_string(), &ctx_for("gps_domicile_lon"))
                .await
                .map_err(|e| AppError::Internal(e.context("encrypt lon")))?,
        ),
        None => None,
    };

    // Build a dynamic UPDATE. We always increment lww_version.
    let row = sqlx::query_as::<_, ProducerRow>(
        r#"
        UPDATE producer SET
          full_name_encrypted        = COALESCE($2,  full_name_encrypted),
          full_name_dek_kid          = COALESCE($3,  full_name_dek_kid),
          nin_encrypted              = COALESCE($4,  nin_encrypted),
          nin_dek_kid                = COALESCE($5,  nin_dek_kid),
          phone_encrypted            = COALESCE($6,  phone_encrypted),
          phone_dek_kid              = COALESCE($7,  phone_dek_kid),
          photo_url_encrypted        = COALESCE($8,  photo_url_encrypted),
          photo_url_dek_kid          = COALESCE($9,  photo_url_dek_kid),
          gps_domicile_lat_encrypted = COALESCE($10, gps_domicile_lat_encrypted),
          gps_domicile_lat_dek_kid   = COALESCE($11, gps_domicile_lat_dek_kid),
          gps_domicile_lon_encrypted = COALESCE($12, gps_domicile_lon_encrypted),
          gps_domicile_lon_dek_kid   = COALESCE($13, gps_domicile_lon_dek_kid),
          primary_crop               = COALESCE($14, primary_crop),
          household_id               = COALESCE($15, household_id),
          lww_version                = lww_version + 1,
          updated_at                 = now()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(full_name_enc.as_ref().map(|e| &e.ciphertext_bytes))
    .bind(full_name_enc.as_ref().map(|e| &e.kid))
    .bind(nin_enc.as_ref().map(|e| &e.ciphertext_bytes))
    .bind(nin_enc.as_ref().map(|e| &e.kid))
    .bind(phone_enc.as_ref().map(|e| &e.ciphertext_bytes))
    .bind(phone_enc.as_ref().map(|e| &e.kid))
    .bind(photo_enc.as_ref().map(|e| &e.ciphertext_bytes))
    .bind(photo_enc.as_ref().map(|e| &e.kid))
    .bind(lat_enc.as_ref().map(|e| &e.ciphertext_bytes))
    .bind(lat_enc.as_ref().map(|e| &e.kid))
    .bind(lon_enc.as_ref().map(|e| &e.ciphertext_bytes))
    .bind(lon_enc.as_ref().map(|e| &e.kid))
    .bind(&req.primary_crop)
    .bind(req.household_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await.map_err(AppError::from)?;

    let decrypted = decrypt_producer_pii(vault, tenant, &row)
        .await
        .map_err(|e| AppError::Internal(e.context("decrypt after patch")))?;

    Ok(to_response(&row, decrypted))
}

/// Soft-delete a producer (sets deleted_at).
#[instrument(skip(pool), fields(tenant = %tenant.slug, producer_id = %id))]
pub async fn delete_producer(
    pool: &PgPool,
    tenant: &TenantContext,
    id: Uuid,
) -> Result<(), AppError> {
    let schema = tenant.schema_name();
    let mut tx = pool.begin().await?;
    set_search_path(&mut tx, &schema)
        .await
        .map_err(AppError::Internal)?;

    let affected = sqlx::query(
        "UPDATE producer SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await.map_err(AppError::from)?;

    if affected == 0 {
        return Err(AppError::NotFound(format!("producer {id}")));
    }
    Ok(())
}

/// Fetch a raw (non-decrypted) producer row — used by gRPC service.
#[instrument(skip(pool), fields(tenant_slug = tenant_slug, producer_id = %id))]
pub async fn get_producer_raw(
    pool: &PgPool,
    tenant_slug: &str,
    id: Uuid,
) -> Result<ProducerRow, AppError> {
    let schema = format!("terroir_t_{tenant_slug}");
    let mut tx = pool.begin().await?;
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    let row = sqlx::query_as::<_, ProducerRow>(
        "SELECT * FROM producer WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("producer {id}")))?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(row)
}

/// List producer IDs for a cooperative — used by gRPC parcel streaming.
pub async fn list_producer_ids_for_coop(
    pool: &PgPool,
    tenant_slug: &str,
    coop_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let schema = format!("terroir_t_{tenant_slug}");
    let mut tx = pool.begin().await?;
    let sql = format!("SET LOCAL search_path TO {schema}");
    sqlx::query(&sql)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    let rows =
        sqlx::query("SELECT id FROM producer WHERE cooperative_id = $1 AND deleted_at IS NULL")
            .bind(coop_id)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await.map_err(AppError::from)?;

    Ok(rows.iter().map(|r| r.get("id")).collect())
}
