// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-admin — tenant_service : provisioning logic.
//
// Workflow (ADR-006 §Onboarding, ULTRAPLAN P0.3):
//   (a) INSERT terroir_shared.cooperative (status=PROVISIONING)
//   (b) Load + render tenant-template SQL files (T001..T100)
//   (c) EXECUTE each template sequentially within a transaction
//   (d) UPDATE cooperative SET status='ACTIVE'
//   (e) Publish auth.terroir.tenant.provisioned to Redpanda (best-effort)
//   (f) Return TenantResponse
//
// Transaction pooling note (pgbouncer):
//   sqlx is configured with statement_cache_capacity=0 (no prepared stmts).
//   Schema-creation DDL runs inside explicit sqlx transactions; SET LOCAL
//   is used (not SET) to avoid polluting the pooled connection state.
//
// 20k+ tenant invariant: all lookups use indexed columns; no full-table scan.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use tracing::{info, warn};
use uuid::Uuid;

use crate::dto::{CreateTenantRequest, TenantListResponse, TenantResponse};
use crate::tenant_template;

// ---------------------------------------------------------------------------
// Schema naming helpers
// ---------------------------------------------------------------------------

fn schema_name(slug: &str) -> String {
    format!("terroir_t_{}", slug)
}

fn audit_schema_name(slug: &str) -> String {
    format!("audit_t_{}", slug)
}

// ---------------------------------------------------------------------------
// Keyset cursor helpers (avoids COUNT(*) over all rows)
// ---------------------------------------------------------------------------

/// Encode keyset cursor: base64(created_at_rfc3339 + "," + uuid)
fn encode_cursor(created_at: &DateTime<Utc>, id: &Uuid) -> String {
    use base64::Engine;
    let raw = format!("{},{}", created_at.to_rfc3339(), id);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes())
}

/// Decode keyset cursor into (created_at, id).
fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, Uuid)> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .context("invalid cursor base64")?;
    let s = String::from_utf8(bytes).context("cursor is not UTF-8")?;
    let (ts, id_str) = s.split_once(',').context("cursor missing separator")?;
    let created_at = ts.parse::<DateTime<Utc>>().context("cursor timestamp")?;
    let id = id_str.parse::<Uuid>().context("cursor UUID")?;
    Ok((created_at, id))
}

// ---------------------------------------------------------------------------
// Provisioning
// ---------------------------------------------------------------------------

/// Create a new tenant. Returns TenantResponse on success.
/// Idempotency: if the slug already exists with status=ACTIVE the function
/// returns an error (caller should return HTTP 409).
pub async fn provision_tenant(
    pool: Arc<PgPool>,
    template_dir: Arc<PathBuf>,
    req: CreateTenantRequest,
) -> Result<TenantResponse> {
    let schema = schema_name(&req.slug);
    let audit_schema = audit_schema_name(&req.slug);

    // (a) INSERT cooperative with PROVISIONING status.
    // Unique constraint on slug / schema_name / audit_schema_name gives us
    // idempotency-by-conflict at no extra SELECT cost.
    let mut tx = pool.begin().await.context("begin transaction")?;

    let row = sqlx::query(
        r#"
        INSERT INTO terroir_shared.cooperative
          (slug, legal_name, country_iso2, region, primary_crop,
           schema_name, audit_schema_name, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'PROVISIONING')
        RETURNING id, slug, legal_name, country_iso2, region, primary_crop,
                  status, schema_name, audit_schema_name, created_at
        "#,
    )
    .bind(&req.slug)
    .bind(&req.legal_name)
    .bind(&req.country_iso2)
    .bind(&req.region)
    .bind(&req.primary_crop)
    .bind(&schema)
    .bind(&audit_schema)
    .fetch_one(&mut *tx)
    .await
    .context("insert cooperative")?;

    let tenant_id: Uuid = row.try_get("id")?;

    info!(
        tenant_id = %tenant_id,
        slug = %req.slug,
        schema = %schema,
        "cooperative row inserted, beginning schema provisioning"
    );

    // (b)+(c) Load templates and execute sequentially.
    let templates = tenant_template::load_and_render(&template_dir, &schema, &audit_schema)
        .await
        .context("load tenant templates")?;

    // Execute templates sequentially within the transaction.
    // We collect (filename, sql) tuples to avoid borrowing `templates` across await points,
    // which would prevent the future from being Send (Rust 2024 capture rules).
    let template_pairs: Vec<(String, String)> =
        templates.into_iter().map(|t| (t.filename, t.sql)).collect();

    for (filename, sql) in template_pairs {
        // DDL runs inside the outer tx so any failure rolls back the cooperative INSERT.
        // execute_ddl_batch is a named async fn to give the compiler a concrete lifetime
        // on &mut Transaction, avoiding HRTB ambiguity inside async move (Rust 2024).
        execute_ddl_batch(&mut tx, &sql)
            .await
            .with_context(|| format!("execute template {filename}"))?;

        info!(template = %filename, slug = %req.slug, "template applied");
    }

    // (d) Mark ACTIVE.
    sqlx::query("UPDATE terroir_shared.cooperative SET status = 'ACTIVE' WHERE id = $1")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .context("update cooperative status to ACTIVE")?;

    tx.commit()
        .await
        .context("commit provisioning transaction")?;

    info!(
        tenant_id = %tenant_id,
        slug = %req.slug,
        "tenant provisioning committed"
    );

    // (e) Publish Redpanda event (best-effort — do not fail the request).
    // In P0 Redpanda may not be running; log the warning and continue.
    // Full implementation in P0.5 (seed-redpanda-terroir-topics.sh).
    if let Err(e) = publish_provisioned_event(tenant_id, &req.slug).await {
        warn!(
            tenant_id = %tenant_id,
            error = %e,
            "failed to publish tenant.provisioned event (best-effort, non-fatal)"
        );
    }

    Ok(TenantResponse {
        id: tenant_id,
        slug: req.slug.clone(),
        legal_name: req.legal_name.clone(),
        country_iso2: req.country_iso2.clone(),
        region: req.region.clone(),
        primary_crop: req.primary_crop.clone(),
        status: "ACTIVE".to_string(),
        schema_name: schema,
        audit_schema_name: audit_schema,
        created_at: row.try_get::<DateTime<Utc>, _>("created_at")?,
    })
}

// ---------------------------------------------------------------------------
// Get tenant by slug
// ---------------------------------------------------------------------------

pub async fn get_tenant(pool: &PgPool, slug: &str) -> Result<Option<TenantResponse>> {
    // Uses idx_cooperative_active partial index when slug is the predicate.
    let row = sqlx::query(
        r#"
        SELECT id, slug, legal_name, country_iso2, region, primary_crop,
               status, schema_name, audit_schema_name, created_at
        FROM terroir_shared.cooperative
        WHERE slug = $1
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .context("get_tenant query")?;

    Ok(row.map(|r| TenantResponse {
        id: r.get("id"),
        slug: r.get("slug"),
        legal_name: r.get("legal_name"),
        country_iso2: r.get("country_iso2"),
        region: r.get("region"),
        primary_crop: r.get("primary_crop"),
        status: r.get("status"),
        schema_name: r.get("schema_name"),
        audit_schema_name: r.get("audit_schema_name"),
        created_at: r.get("created_at"),
    }))
}

// ---------------------------------------------------------------------------
// List tenants — keyset pagination, O(page_size) not O(N_total)
// ---------------------------------------------------------------------------

pub async fn list_tenants(
    pool: &PgPool,
    limit: i64,
    cursor: Option<&str>,
) -> Result<TenantListResponse> {
    let limit = limit.clamp(1, 200);

    let rows = match cursor {
        None => sqlx::query(
            r#"
                SELECT id, slug, legal_name, country_iso2, region, primary_crop,
                       status, schema_name, audit_schema_name, created_at
                FROM terroir_shared.cooperative
                ORDER BY created_at DESC, id DESC
                LIMIT $1
                "#,
        )
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .context("list_tenants first page")?,
        Some(c) => {
            let (after_ts, after_id) = decode_cursor(c).context("decode cursor")?;
            sqlx::query(
                r#"
                SELECT id, slug, legal_name, country_iso2, region, primary_crop,
                       status, schema_name, audit_schema_name, created_at
                FROM terroir_shared.cooperative
                WHERE (created_at, id) < ($1, $2)
                ORDER BY created_at DESC, id DESC
                LIMIT $3
                "#,
            )
            .bind(after_ts)
            .bind(after_id)
            .bind(limit + 1)
            .fetch_all(pool)
            .await
            .context("list_tenants paginated")?
        }
    };

    let has_next = rows.len() as i64 > limit;
    let rows = if has_next {
        &rows[..limit as usize]
    } else {
        &rows[..]
    };

    let items: Vec<TenantResponse> = rows
        .iter()
        .map(|r| TenantResponse {
            id: r.get("id"),
            slug: r.get("slug"),
            legal_name: r.get("legal_name"),
            country_iso2: r.get("country_iso2"),
            region: r.get("region"),
            primary_crop: r.get("primary_crop"),
            status: r.get("status"),
            schema_name: r.get("schema_name"),
            audit_schema_name: r.get("audit_schema_name"),
            created_at: r.get("created_at"),
        })
        .collect();

    let next_cursor = if has_next {
        items
            .last()
            .map(|last| encode_cursor(&last.created_at, &last.id))
    } else {
        None
    };

    Ok(TenantListResponse {
        items,
        next_cursor,
        limit,
    })
}

// ---------------------------------------------------------------------------
// Suspend tenant
// ---------------------------------------------------------------------------

/// Set cooperative.status = 'SUSPENDED'. Idempotent.
pub async fn suspend_tenant(pool: &PgPool, slug: &str) -> Result<Option<TenantResponse>> {
    let row = sqlx::query(
        r#"
        UPDATE terroir_shared.cooperative
        SET status = 'SUSPENDED'
        WHERE slug = $1 AND status NOT IN ('ARCHIVED', 'SUSPENDED')
        RETURNING id, slug, legal_name, country_iso2, region, primary_crop,
                  status, schema_name, audit_schema_name, created_at
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .context("suspend_tenant")?;

    if row.is_none() {
        // Check if tenant simply doesn't exist vs already suspended.
        return get_tenant(pool, slug).await;
    }

    Ok(row.map(|r| TenantResponse {
        id: r.get("id"),
        slug: r.get("slug"),
        legal_name: r.get("legal_name"),
        country_iso2: r.get("country_iso2"),
        region: r.get("region"),
        primary_crop: r.get("primary_crop"),
        status: r.get("status"),
        schema_name: r.get("schema_name"),
        audit_schema_name: r.get("audit_schema_name"),
        created_at: r.get("created_at"),
    }))
}

// ---------------------------------------------------------------------------
// DDL batch executor
// ---------------------------------------------------------------------------

/// Execute a block of DDL SQL within an existing transaction.
///
/// Splits the input into individual statements on `;` boundaries — but
/// **respects PostgreSQL dollar-quoted strings** (`$$ ... $$` and tagged
/// `$tag$ ... $tag$`). This is required because PL/pgSQL function bodies
/// (used in T100__audit_log.sql.tmpl trigger functions) contain `;`
/// characters inside the body that must NOT split the statement.
///
/// Single-line `--` comments and standalone `/* ... */` comments are
/// stripped before split. Each statement is executed via `sqlx::query`
/// which with `statement_cache_capacity=0` is compatible with pgbouncer
/// transaction pooling.
async fn execute_ddl_batch(tx: &mut Transaction<'_, Postgres>, sql: &str) -> Result<()> {
    let statements = split_sql_statements(sql);

    for stmt in statements {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt)
            .execute(&mut **tx)
            .await
            .with_context(|| format!("execute DDL statement: {}", &stmt[..stmt.len().min(80)]))?;
    }
    Ok(())
}

/// Split a SQL block into individual statements respecting:
///   - dollar-quoted strings: `$$ ... $$` and tagged `$tag$ ... $tag$`
///   - single-quoted strings: `'...''...'`
///   - line comments: `-- ...\n`
///   - block comments: `/* ... */`
///
/// Returns each statement WITHOUT its trailing `;`.
fn split_sql_statements(sql: &str) -> Vec<String> {
    #[derive(PartialEq)]
    enum State {
        Normal,
        SingleQuote,
        LineComment,
        BlockComment,
        DollarQuote, // inside $tag$ ... $tag$
    }

    let bytes = sql.as_bytes();
    let mut state = State::Normal;
    let mut current = String::new();
    let mut out: Vec<String> = Vec::new();
    let mut dollar_tag: String = String::new(); // current $tag$ (incl. $)
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        match state {
            State::Normal => {
                // Detect line comment "--"
                if b == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
                    state = State::LineComment;
                    i += 2;
                    continue;
                }
                // Detect block comment "/*"
                if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                    state = State::BlockComment;
                    i += 2;
                    continue;
                }
                // Detect single quote
                if b == b'\'' {
                    current.push('\'');
                    state = State::SingleQuote;
                    i += 1;
                    continue;
                }
                // Detect dollar-quoted string opening: $tag$ where tag is empty or
                // starts with letter/underscore and contains alphanum/underscore.
                if b == b'$' {
                    // Find the closing $
                    let mut j = i + 1;
                    let mut tag_valid = true;
                    while j < bytes.len() && bytes[j] != b'$' {
                        let c = bytes[j];
                        if !(c.is_ascii_alphanumeric() || c == b'_') {
                            tag_valid = false;
                            break;
                        }
                        j += 1;
                    }
                    if tag_valid && j < bytes.len() && bytes[j] == b'$' {
                        // Found $tag$ (possibly $$). Enter DollarQuote state.
                        dollar_tag = String::from_utf8_lossy(&bytes[i..=j]).to_string();
                        current.push_str(&dollar_tag);
                        state = State::DollarQuote;
                        i = j + 1;
                        continue;
                    }
                    // Otherwise treat $ as literal
                    current.push('$');
                    i += 1;
                    continue;
                }
                // Statement terminator
                if b == b';' {
                    out.push(std::mem::take(&mut current));
                    i += 1;
                    continue;
                }
                current.push(b as char);
                i += 1;
            }
            State::SingleQuote => {
                current.push(b as char);
                if b == b'\'' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::LineComment => {
                if b == b'\n' {
                    current.push('\n');
                    state = State::Normal;
                }
                i += 1;
            }
            State::BlockComment => {
                if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    state = State::Normal;
                    i += 2;
                    continue;
                }
                i += 1;
            }
            State::DollarQuote => {
                // Look for closing tag at current position.
                let tag_bytes = dollar_tag.as_bytes();
                if i + tag_bytes.len() <= bytes.len() && &bytes[i..i + tag_bytes.len()] == tag_bytes
                {
                    current.push_str(&dollar_tag);
                    i += tag_bytes.len();
                    state = State::Normal;
                    continue;
                }
                current.push(b as char);
                i += 1;
            }
        }
    }

    // Flush any trailing statement (no terminating ';').
    if !current.trim().is_empty() {
        out.push(current);
    }

    out
}

#[cfg(test)]
mod ddl_split_tests {
    use super::split_sql_statements;

    #[test]
    fn splits_simple_statements() {
        let sql = "CREATE TABLE a (id int); CREATE TABLE b (id int);";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn preserves_dollar_quoted_function_body() {
        let sql = r#"
            CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN
              RAISE EXCEPTION 'nope; never';
              RETURN NULL;
            END;
            $$;
            CREATE TRIGGER t BEFORE UPDATE ON x EXECUTE FUNCTION f();
        "#;
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2, "got: {stmts:?}");
        assert!(stmts[0].contains("RAISE EXCEPTION"));
        assert!(stmts[0].contains("RETURN NULL"));
        assert!(stmts[1].contains("CREATE TRIGGER"));
    }

    #[test]
    fn ignores_semicolon_in_string_literal() {
        let sql = "INSERT INTO t VALUES ('a;b'); SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("'a;b'"));
    }

    #[test]
    fn ignores_line_comment() {
        let sql = "-- a; b\nSELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("SELECT 1"));
    }
}

// ---------------------------------------------------------------------------
// Redpanda event publisher (stub for P0 — full impl in P0.5)
// ---------------------------------------------------------------------------

async fn publish_provisioned_event(tenant_id: Uuid, slug: &str) -> Result<()> {
    // P0 stub: the topic `terroir.tenant.provisioned` will be created by
    // INFRA/scripts/seed-redpanda-terroir-topics.sh (P0.5).
    // Full Redpanda producer (rdkafka or fluvio) wired in P0.5.
    info!(
        tenant_id = %tenant_id,
        slug = %slug,
        topic = "auth.terroir.tenant.provisioned",
        "TODO(P0.5): publish tenant.provisioned event"
    );
    Ok(())
}
