-- SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
-- SPDX-License-Identifier: AGPL-3.0-or-later

CREATE SCHEMA IF NOT EXISTS audit;

CREATE TABLE audit.audit_log (
    id              BIGSERIAL PRIMARY KEY,
    event_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_id        TEXT,
    actor_type      TEXT NOT NULL CHECK (actor_type IN ('USER', 'SERVICE', 'SYSTEM', 'ANONYMOUS')),
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT,
    ip_address      INET,
    user_agent      TEXT,
    result          TEXT NOT NULL CHECK (result IN ('SUCCESS', 'FAILURE', 'DENIED')),
    metadata        JSONB DEFAULT '{}',
    trace_id        TEXT,
    service_name    TEXT NOT NULL
);

-- Partition by month for efficient retention management
-- (In production, use declarative partitioning)
CREATE INDEX idx_audit_log_event_time ON audit.audit_log (event_time);
CREATE INDEX idx_audit_log_actor_id ON audit.audit_log (actor_id);
CREATE INDEX idx_audit_log_action ON audit.audit_log (action);
CREATE INDEX idx_audit_log_resource ON audit.audit_log (resource_type, resource_id);
CREATE INDEX idx_audit_log_trace_id ON audit.audit_log (trace_id);

-- Prevent mutations (append-only)
CREATE OR REPLACE FUNCTION audit.prevent_audit_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Audit log records cannot be modified or deleted';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_log_no_update
    BEFORE UPDATE ON audit.audit_log
    FOR EACH ROW EXECUTE FUNCTION audit.prevent_audit_mutation();

CREATE TRIGGER audit_log_no_delete
    BEFORE DELETE ON audit.audit_log
    FOR EACH ROW EXECUTE FUNCTION audit.prevent_audit_mutation();

COMMENT ON TABLE audit.audit_log IS 'Append-only audit trail — Loi 010-2004 compliance (5 year retention)';
