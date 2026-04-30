-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V001__shared_extensions.sql
-- Shared PostgreSQL extensions for TERROIR multi-tenant platform.
-- Apply once on the target database (faso_terroir / postgres).
-- Requires superuser or a role with CREATE EXTENSION privilege.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS btree_gin;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- postgis : required production, optional in dev (Alpine PG image may not
-- ship a matching server-version postgis package). Non-fatal if absent —
-- the tenant-template T003 detects pg_extension and falls back to BYTEA.
DO $$ BEGIN
  CREATE EXTENSION IF NOT EXISTS postgis;
EXCEPTION WHEN OTHERS THEN
  RAISE NOTICE 'postgis not available — T003 will use BYTEA fallback (production must install postgis)';
END $$;

-- pg_partman : optional for P0, required from P3+ for hot-table partitioning
-- by tenant. Graceful no-op if unavailable on the host PG instance.
DO $$ BEGIN
  CREATE EXTENSION IF NOT EXISTS pg_partman;
EXCEPTION WHEN OTHERS THEN
  RAISE NOTICE 'pg_partman not available — skip (optional for P0, required P3+)';
END $$;
