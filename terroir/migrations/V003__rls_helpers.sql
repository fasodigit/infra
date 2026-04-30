-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V003__rls_helpers.sql
-- Application role + Row-Level Security helpers for TERROIR.
-- See ADR-006 §Security : SET search_path server-side only ; pgbouncer
-- rejects client-side search_path manipulation.
--
-- Pattern Rust (sqlx, transaction pooling) :
--   sqlx::query("SET LOCAL app.current_tenant_slug = $1")
--       .bind(&slug)
--       .execute(&mut *tx)
--       .await?;

-- ---------------------------------------------------------------------------
-- Role terroir_app : applicative read/write (no login)
-- Connection user (e.g. terroir_svc) is granted terroir_app at login.
-- ---------------------------------------------------------------------------
DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'terroir_app') THEN
    CREATE ROLE terroir_app NOLOGIN;
  END IF;
END $$;

GRANT USAGE ON SCHEMA terroir_shared TO terroir_app;
GRANT SELECT, INSERT, UPDATE, DELETE
  ON ALL TABLES IN SCHEMA terroir_shared TO terroir_app;

-- Ensure future tables in terroir_shared inherit grant automatically.
ALTER DEFAULT PRIVILEGES IN SCHEMA terroir_shared
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO terroir_app;

-- ---------------------------------------------------------------------------
-- current_tenant_slug() : read the session GUC set by the application.
-- Usage: SET LOCAL app.current_tenant_slug = 'terroir_t_uph_hounde';
-- Never accepted from the client directly (pgbouncer config blocks it).
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION terroir_shared.current_tenant_slug()
RETURNS TEXT
LANGUAGE sql STABLE AS $$
  SELECT current_setting('app.current_tenant_slug', true);
$$;

-- ---------------------------------------------------------------------------
-- current_agent_user_id() : UUID of the currently authenticated agent.
-- Usage: SET LOCAL app.current_user_id = '<uuid>';
-- Returns NULL when GUC is unset (NULLIF guards against empty string).
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION terroir_shared.current_agent_user_id()
RETURNS UUID
LANGUAGE sql STABLE AS $$
  SELECT NULLIF(current_setting('app.current_user_id', true), '')::uuid;
$$;

-- ---------------------------------------------------------------------------
-- validate_tenant_slug(slug) : boolean guard for application-level checks.
-- Used in stored procedures that need to assert slug format without relying
-- on the cooperative table index scan.
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION terroir_shared.validate_tenant_slug(p_slug TEXT)
RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE AS $$
  SELECT p_slug ~ '^[a-z0-9_]{3,50}$';
$$;
