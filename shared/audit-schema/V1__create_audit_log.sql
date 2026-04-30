-- SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- audit.audit_log — append-only audit trail (Loi 010-2004, 5 ans rétention).
--
-- Partitionnement déclaratif PARTITION BY RANGE(event_time) en mensuel :
--   - élimine les bloats d'index sur > 10M lignes,
--   - permet DROP PARTITION pour la rétention (pas de DELETE lock pressure),
--   - permet pg_partman ultérieurement sans migration disruptive.
--
-- Pas de dépendance externe : uniquement PostgreSQL natif (>= 12). pg_partman
-- est OPTIONNEL et peut prendre le relais via partman.create_parent() si
-- déployé plus tard.
--
-- Migration entièrement idempotente : peut être exécutée plusieurs fois sans
-- erreur. Les CREATE sont protégés par IF NOT EXISTS, les triggers font
-- DROP IF EXISTS avant de recréer.

CREATE SCHEMA IF NOT EXISTS audit;

-- ─────────────────────────────────────────────────────────────────────
-- Table partitionnée parente. La PRIMARY KEY doit inclure la clé de
-- partitionnement (event_time) pour respecter la contrainte de PG.
-- ─────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS audit.audit_log (
    id              BIGSERIAL,
    event_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_id        TEXT,
    actor_type      TEXT NOT NULL CHECK (actor_type IN ('USER', 'SERVICE', 'SYSTEM', 'ANONYMOUS')),
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT,
    ip_address      TEXT,
    user_agent      TEXT,
    result          TEXT NOT NULL CHECK (result IN ('SUCCESS', 'FAILURE', 'DENIED')),
    metadata        JSONB DEFAULT '{}',
    trace_id        TEXT,
    service_name    TEXT NOT NULL,
    PRIMARY KEY (id, event_time)
) PARTITION BY RANGE (event_time);

-- ip_address stored as TEXT (not INET) to keep the JPA mapping simple:
-- AuditEvent.ipAddress is a String, and Hibernate's default String→varchar
-- mapping is incompatible with PostgreSQL's INET type without a custom
-- UserType. TEXT preserves IPv6/IPv4 strings; INET range/CIDR features
-- aren't needed for an append-only audit log.
ALTER TABLE audit.audit_log ALTER COLUMN ip_address TYPE TEXT USING ip_address::TEXT;

-- ─────────────────────────────────────────────────────────────────────
-- Index globaux (propagés à toutes les partitions enfants par PG)
-- ─────────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_audit_log_event_time ON audit.audit_log (event_time);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor_id   ON audit.audit_log (actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_action     ON audit.audit_log (action);
CREATE INDEX IF NOT EXISTS idx_audit_log_resource   ON audit.audit_log (resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_trace_id   ON audit.audit_log (trace_id);

-- ─────────────────────────────────────────────────────────────────────
-- Append-only : interdire UPDATE et DELETE.
-- ─────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION audit.prevent_audit_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Audit log records cannot be modified or deleted';
END;
$$ LANGUAGE plpgsql;

-- ─────────────────────────────────────────────────────────────────────
-- Helper : créer la partition pour un mois donné si elle n'existe pas.
-- Idempotent. Attache les triggers append-only sur la nouvelle partition.
-- ─────────────────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION audit.ensure_audit_log_partition(p_month_start DATE)
RETURNS TEXT AS $$
DECLARE
    v_partition_name TEXT;
    v_start          DATE := DATE_TRUNC('month', p_month_start)::DATE;
    v_end            DATE := (v_start + INTERVAL '1 month')::DATE;
BEGIN
    v_partition_name := FORMAT('audit_log_%s', TO_CHAR(v_start, 'YYYY_MM'));

    EXECUTE FORMAT(
        'CREATE TABLE IF NOT EXISTS audit.%I PARTITION OF audit.audit_log FOR VALUES FROM (%L) TO (%L)',
        v_partition_name, v_start, v_end
    );

    EXECUTE FORMAT(
        'DROP TRIGGER IF EXISTS audit_log_no_update ON audit.%I',
        v_partition_name
    );
    EXECUTE FORMAT(
        'CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit.%I FOR EACH ROW EXECUTE FUNCTION audit.prevent_audit_mutation()',
        v_partition_name
    );
    EXECUTE FORMAT(
        'DROP TRIGGER IF EXISTS audit_log_no_delete ON audit.%I',
        v_partition_name
    );
    EXECUTE FORMAT(
        'CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit.%I FOR EACH ROW EXECUTE FUNCTION audit.prevent_audit_mutation()',
        v_partition_name
    );

    RETURN v_partition_name;
END;
$$ LANGUAGE plpgsql;

-- ─────────────────────────────────────────────────────────────────────
-- Pré-créer 13 partitions : mois courant + 12 mois à venir.
-- Le service Spring fait tourner @Scheduled quotidien pour glisser la fenêtre.
-- ─────────────────────────────────────────────────────────────────────
DO $$
DECLARE
    i INT;
BEGIN
    FOR i IN 0..12 LOOP
        PERFORM audit.ensure_audit_log_partition(
            (DATE_TRUNC('month', NOW()) + (i || ' months')::INTERVAL)::DATE
        );
    END LOOP;
END
$$;

-- ─────────────────────────────────────────────────────────────────────
-- Triggers parents (idempotent via DROP IF EXISTS).
-- Bloquent UPDATE/DELETE même si quelqu'un attaque audit.audit_log
-- directement plutôt que via une partition.
-- ─────────────────────────────────────────────────────────────────────
DROP TRIGGER IF EXISTS audit_log_parent_no_update ON audit.audit_log;
CREATE TRIGGER audit_log_parent_no_update
    BEFORE UPDATE ON audit.audit_log
    FOR EACH ROW EXECUTE FUNCTION audit.prevent_audit_mutation();

DROP TRIGGER IF EXISTS audit_log_parent_no_delete ON audit.audit_log;
CREATE TRIGGER audit_log_parent_no_delete
    BEFORE DELETE ON audit.audit_log
    FOR EACH ROW EXECUTE FUNCTION audit.prevent_audit_mutation();

COMMENT ON TABLE audit.audit_log
    IS 'Append-only audit trail — Loi 010-2004 (5 ans). Partitionnement mensuel par event_time. DROP partition pour la rétention.';
COMMENT ON FUNCTION audit.ensure_audit_log_partition(DATE)
    IS 'Crée une partition mensuelle si absente + triggers append-only. Idempotent.';
