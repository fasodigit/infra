-- SPDX-License-Identifier: AGPL-3.0-only
-- Copyright (C) 2026 FASO DIGITALISATION - Ministère du Numérique, Burkina Faso
-- ============================================================
-- V1__init.sql — notifier-ms initial schema
-- ============================================================

-- Extension for JSONB support (already available on PostgreSQL 17)
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- ── notification_templates ────────────────────────────────────────────────────
CREATE TABLE notification_templates (
    id                BIGSERIAL PRIMARY KEY,
    name              VARCHAR(128)  NOT NULL UNIQUE,
    subject_template  VARCHAR(512)  NOT NULL,
    body_hbs          TEXT          NOT NULL,
    context_rules_json JSONB,
    created_at        TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE notification_templates IS
    'Handlebars email templates with optional JSON-Logic routing rules';

CREATE INDEX idx_templates_name ON notification_templates USING btree (name);

-- ── notification_deliveries ───────────────────────────────────────────────────
CREATE TABLE notification_deliveries (
    delivery_id   VARCHAR(64)   PRIMARY KEY,
    recipient     VARCHAR(320)  NOT NULL,
    template_name VARCHAR(128)  NOT NULL,
    status        VARCHAR(16)   NOT NULL DEFAULT 'PENDING'
                  CHECK (status IN ('PENDING','SENT','FAILED','DLQ')),
    attempts      INTEGER       NOT NULL DEFAULT 0,
    last_error    TEXT,
    sent_at       TIMESTAMPTZ,
    event_payload TEXT,
    created_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE notification_deliveries IS
    'Delivery tracking: one row per (event × recipient), idempotent via delivery_id';

CREATE INDEX idx_delivery_status    ON notification_deliveries (status);
CREATE INDEX idx_delivery_template  ON notification_deliveries (template_name);
CREATE INDEX idx_delivery_recipient ON notification_deliveries (recipient);
CREATE INDEX idx_delivery_created   ON notification_deliveries (created_at DESC);

-- ── notification_recipients ───────────────────────────────────────────────────
-- Lookup table for recipient lists referenced by context rules.
-- Rules can use DB recipients or inline lists in context-rules.json.
CREATE TABLE notification_recipients (
    id          BIGSERIAL     PRIMARY KEY,
    group_name  VARCHAR(128)  NOT NULL,
    email       VARCHAR(320)  NOT NULL,
    label       VARCHAR(256),
    active      BOOLEAN       NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE notification_recipients IS
    'Named recipient groups for dynamic rule-based dispatching';

CREATE UNIQUE INDEX idx_recipients_group_email ON notification_recipients (group_name, email);

-- ── Seed: default templates ───────────────────────────────────────────────────
-- Templates are seeded here for bootstrapping; overridable via /api/templates.

INSERT INTO notification_recipients (group_name, email, label) VALUES
    ('devops',           'devops@faso.gov.bf',         'DevOps Team'),
    ('agriculture-metier','agriculture@faso.gov.bf',   'Agriculture Business Team'),
    ('etat-civil',       'etatcivil@faso.gov.bf',      'État Civil Administrators'),
    ('poulets-team',     'poulets@faso.gov.bf',         'Poulets Platform Team'),
    ('sogesy-team',      'sogesy@faso.gov.bf',          'SOGESY Team'),
    ('hospital-team',    'hospital@faso.gov.bf',        'Hospital System Team'),
    ('escool-team',      'escool@faso.gov.bf',          'ESCOOL Team'),
    ('eticket-team',     'eticket@faso.gov.bf',         'E-Ticket Team'),
    ('altmission-team',  'altmission@faso.gov.bf',      'AltMission Team'),
    ('fasokalan-team',   'fasokalan@faso.gov.bf',       'FasoKalan Team');
