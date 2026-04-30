-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V9__audit_log_extend.sql
-- Phase 4.b admin-UI: extend audit_log to support the design § 9 requirements,
-- and install an immutability trigger gated by admin_settings.audit.immutable_mode.

ALTER TABLE audit_log
    ADD COLUMN IF NOT EXISTS resource_type VARCHAR(100),
    ADD COLUMN IF NOT EXISTS old_value     JSONB,
    ADD COLUMN IF NOT EXISTS new_value     JSONB,
    ADD COLUMN IF NOT EXISTS metadata      JSONB,
    ADD COLUMN IF NOT EXISTS trace_id      VARCHAR(64),
    ADD COLUMN IF NOT EXISTS user_agent    VARCHAR(500);

CREATE INDEX IF NOT EXISTS idx_audit_log_resource_type
    ON audit_log(resource_type);

CREATE INDEX IF NOT EXISTS idx_audit_log_trace_id
    ON audit_log(trace_id);

CREATE INDEX IF NOT EXISTS idx_audit_log_action_created
    ON audit_log(action, created_at DESC);

-- ============================================================
-- Immutability trigger: blocks UPDATE / DELETE when the
-- admin_settings.audit.immutable_mode flag is true. This is the
-- "trigger" option from §17 of the gap analysis (no WORM tablespace).
-- ============================================================
CREATE OR REPLACE FUNCTION audit_log_immutable() RETURNS trigger AS $$
DECLARE
    flag_value JSONB;
    is_immutable BOOLEAN := true;
BEGIN
    -- Look up the current setting; default to immutable=true if missing.
    SELECT value INTO flag_value
    FROM admin_settings
    WHERE key = 'audit.immutable_mode';

    IF flag_value IS NOT NULL THEN
        is_immutable := COALESCE(flag_value::text::boolean, true);
    END IF;

    IF is_immutable THEN
        RAISE EXCEPTION 'audit_log is immutable (admin_settings.audit.immutable_mode = true) — % is forbidden', TG_OP
            USING ERRCODE = 'insufficient_privilege';
    END IF;

    -- When immutable_mode is explicitly disabled, allow the operation.
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS audit_log_immutable_trg ON audit_log;
CREATE TRIGGER audit_log_immutable_trg
    BEFORE UPDATE OR DELETE ON audit_log
    FOR EACH ROW
    EXECUTE FUNCTION audit_log_immutable();
