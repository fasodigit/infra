-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V002__shared_schema.sql
-- Creates the terroir_shared schema with cross-tenant reference tables.
-- See ULTRAPLAN §3 (40 entities × sync strategy) and ADR-006.
--
-- Tables created here:
--   cooperative        — tenant registry (ACID)
--   agent_session      — JWT session tracking (LWW)
--   geo_check_cache    — Hansen GFC + JRC TMF results (append-only)
--   indicator_value    — M&E metrics (append-only)
--   mooc_module        — training content (LWW)
--   input_catalog      — M5 shared input reference (ACID)
--   account_chart      — SYSCOHADA chart of accounts (ACID)

CREATE SCHEMA IF NOT EXISTS terroir_shared;
SET search_path TO terroir_shared, public;

-- ---------------------------------------------------------------------------
-- cooperative — tenant entity, ACID
-- One row per cooperative (= one tenant).
-- schema_name : terroir_t_<slug>
-- audit_schema_name : audit_t_<slug>
-- ---------------------------------------------------------------------------
CREATE TABLE cooperative (
  id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  slug              VARCHAR(60) NOT NULL UNIQUE,
  legal_name        VARCHAR(200) NOT NULL,
  country_iso2      CHAR(2)     NOT NULL DEFAULT 'BF',
  region            VARCHAR(80),
  primary_crop      VARCHAR(40) NOT NULL,  -- coton/sesame/karite/anacarde/...
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  status            VARCHAR(20) NOT NULL DEFAULT 'PROVISIONING',
  schema_name       VARCHAR(80) NOT NULL UNIQUE,   -- terroir_t_<slug>
  audit_schema_name VARCHAR(80) NOT NULL UNIQUE,   -- audit_t_<slug>
  CONSTRAINT ck_cooperative_slug   CHECK (slug ~ '^[a-z0-9_]{3,50}$'),
  CONSTRAINT ck_cooperative_status CHECK (status IN ('PROVISIONING','ACTIVE','SUSPENDED','ARCHIVED'))
);

-- Partial index for active tenants — O(active) not O(N total).
-- Avoids full-scan on cooperative at every request (20k+ tenant target).
CREATE INDEX idx_cooperative_active ON cooperative(slug) WHERE status = 'ACTIVE';
CREATE INDEX idx_cooperative_created ON cooperative(created_at DESC);

-- ---------------------------------------------------------------------------
-- agent_session — LWW per ULTRAPLAN §3
-- ---------------------------------------------------------------------------
CREATE TABLE agent_session (
  id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  agent_user_id      UUID        NOT NULL,
  cooperative_id     UUID        NOT NULL REFERENCES cooperative(id),
  jwt_jti            VARCHAR(64) NOT NULL,
  issued_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at         TIMESTAMPTZ NOT NULL,
  last_sync_at       TIMESTAMPTZ,
  revoked_at         TIMESTAMPTZ,
  device_fingerprint VARCHAR(128),
  CONSTRAINT uq_agent_session_jti UNIQUE(jwt_jti)
);
CREATE INDEX idx_agent_session_active
  ON agent_session(agent_user_id)
  WHERE revoked_at IS NULL;

-- ---------------------------------------------------------------------------
-- geo_check_cache — Hansen GFC + JRC TMF, append-only
-- polygon_hash : SHA-256 of normalised GeoJSON
-- ---------------------------------------------------------------------------
CREATE TABLE geo_check_cache (
  id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  polygon_hash    VARCHAR(64) NOT NULL UNIQUE,
  dataset         VARCHAR(20) NOT NULL,   -- 'hansen-gfc' | 'jrc-tmf'
  dataset_version VARCHAR(20) NOT NULL,   -- e.g. 'v1.11'
  result          JSONB       NOT NULL,
  computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at      TIMESTAMPTZ
);
CREATE INDEX idx_geo_cache_lookup
  ON geo_check_cache(polygon_hash, dataset, dataset_version);

-- ---------------------------------------------------------------------------
-- indicator_value — M&E append-only (M9)
-- Partitioned by month from P3+ using pg_partman on measured_at.
-- ---------------------------------------------------------------------------
CREATE TABLE indicator_value (
  id             UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
  cooperative_id UUID          NOT NULL REFERENCES cooperative(id),
  indicator_key  VARCHAR(80)   NOT NULL,
  value_numeric  DOUBLE PRECISION,
  value_text     TEXT,
  measured_at    TIMESTAMPTZ   NOT NULL,
  inserted_at    TIMESTAMPTZ   NOT NULL DEFAULT now(),
  trace_id       VARCHAR(32)
);
CREATE INDEX idx_indicator_coop_key
  ON indicator_value(cooperative_id, indicator_key, measured_at DESC);

-- ---------------------------------------------------------------------------
-- mooc_module — LWW (M10)
-- ---------------------------------------------------------------------------
CREATE TABLE mooc_module (
  id               UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
  title            VARCHAR(200) NOT NULL,
  language         VARCHAR(10)  NOT NULL DEFAULT 'fr',
  duration_minutes INTEGER      NOT NULL,
  content_url      TEXT         NOT NULL,
  published_at     TIMESTAMPTZ,
  updated_at       TIMESTAMPTZ  NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- input_catalog — M5 shared reference, ACID
-- ---------------------------------------------------------------------------
CREATE TABLE input_catalog (
  id               UUID           PRIMARY KEY DEFAULT gen_random_uuid(),
  reference_code   VARCHAR(40)    NOT NULL UNIQUE,
  category         VARCHAR(40)    NOT NULL,   -- semence/engrais/vaccin/alevin
  brand            VARCHAR(80),
  unit             VARCHAR(20)    NOT NULL,
  unit_price_xof   DECIMAL(12,2),
  active           BOOLEAN        NOT NULL DEFAULT true,
  updated_at       TIMESTAMPTZ    NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- account_chart — SYSCOHADA (M7), ACID, shared across tenants
-- ---------------------------------------------------------------------------
CREATE TABLE account_chart (
  account_code  VARCHAR(8)   PRIMARY KEY,   -- 411, 6011, etc.
  label         VARCHAR(200) NOT NULL,
  account_class CHAR(1)      NOT NULL,       -- 1-9
  parent_code   VARCHAR(8)   REFERENCES account_chart(account_code)
);
