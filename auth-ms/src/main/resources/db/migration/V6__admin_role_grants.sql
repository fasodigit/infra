-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V6__admin_role_grants.sql
-- Phase 4.b admin-UI: dual-control workflow for sensitive role grants.
--
-- A SUPER-ADMIN proposes a grant (status=PENDING) — a SECOND SUPER-ADMIN must
-- approve it (status=APPROVED). Status transitions are immutable through the
-- AdminRoleGrantService state machine.

CREATE TABLE IF NOT EXISTS admin_role_grants (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    grantor_id      UUID NOT NULL REFERENCES users(id),
    grantee_id      UUID NOT NULL REFERENCES users(id),
    role_id         UUID NOT NULL REFERENCES roles(id),
    justification   TEXT NOT NULL,
    status          VARCHAR(20) NOT NULL
                    CHECK (status IN ('PENDING','APPROVED','REJECTED','EXPIRED')),
    approver_id     UUID REFERENCES users(id),
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    approved_at     TIMESTAMPTZ,
    rejected_at     TIMESTAMPTZ,
    rejection_reason TEXT
);

CREATE INDEX IF NOT EXISTS idx_admin_role_grants_pending
    ON admin_role_grants(status)
    WHERE status = 'PENDING';

CREATE INDEX IF NOT EXISTS idx_admin_role_grants_grantee
    ON admin_role_grants(grantee_id);

CREATE INDEX IF NOT EXISTS idx_admin_role_grants_grantor
    ON admin_role_grants(grantor_id);
