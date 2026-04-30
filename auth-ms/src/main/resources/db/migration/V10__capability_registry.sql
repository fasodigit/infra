-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V10__capability_registry.sql
-- Delta amendment 2026-04-30: fine-grained capability registry & per-account
-- grants. Two ADMIN (or two MANAGER) accounts cannot share the exact same
-- set of capabilities at the UI/BFF level (soft uniqueness — see
-- CapabilityService.checkUniqueness). SUPER-ADMIN bypasses the rule.

-- ============================================================
-- 1. Add a `level` column to roles + insert the MANAGER role.
--    Backward-compat: keep OPERATOR / VIEWER (used by poulets-domain).
--    AuthZ aliasing : OPERATOR ~ VIEWER for the admin plane (no priv ops).
-- ============================================================
ALTER TABLE roles
    ADD COLUMN IF NOT EXISTS level INTEGER NOT NULL DEFAULT 0;

UPDATE roles SET level = 100 WHERE name = 'SUPER_ADMIN';
UPDATE roles SET level = 50  WHERE name = 'ADMIN';
UPDATE roles SET level = 5   WHERE name = 'OPERATOR';
UPDATE roles SET level = 1   WHERE name = 'VIEWER';

INSERT INTO roles (name, description, level) VALUES
    ('MANAGER', 'Manager — capacités fines (sous-ensemble distinct par compte)', 2)
ON CONFLICT (name) DO NOTHING;

-- ============================================================
-- 2. Capability registry — static catalogue of fine-grained capabilities.
--    Seeded with the ~30 caps from delta §1. `applicable_to_roles` is
--    a postgres TEXT[] (subset of {SUPER_ADMIN, ADMIN, MANAGER}).
-- ============================================================
CREATE TABLE IF NOT EXISTS capability_registry (
    key                    VARCHAR(80) PRIMARY KEY,
    category               VARCHAR(40) NOT NULL,
    description_i18n_key   VARCHAR(120) NOT NULL,
    applicable_to_roles    TEXT[] NOT NULL,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_capability_registry_category
    ON capability_registry(category);

INSERT INTO capability_registry (key, category, description_i18n_key, applicable_to_roles) VALUES
    -- Domain Users
    ('users:invite',              'users',     'capability.users.invite',              ARRAY['SUPER_ADMIN','ADMIN']),
    ('users:suspend',             'users',     'capability.users.suspend',             ARRAY['SUPER_ADMIN','ADMIN']),
    ('users:reactivate',          'users',     'capability.users.reactivate',          ARRAY['SUPER_ADMIN','ADMIN']),
    ('users:manage:any_dept',     'users',     'capability.users.manage_any_dept',     ARRAY['SUPER_ADMIN','ADMIN']),
    ('users:manage:own_dept',     'users',     'capability.users.manage_own_dept',     ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('users:mfa:reset',           'users',     'capability.users.mfa_reset',           ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Roles
    ('roles:grant_admin',         'roles',     'capability.roles.grant_admin',         ARRAY['SUPER_ADMIN']),
    ('roles:grant_manager',       'roles',     'capability.roles.grant_manager',       ARRAY['SUPER_ADMIN','ADMIN']),
    ('roles:revoke',              'roles',     'capability.roles.revoke',              ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Sessions
    ('sessions:list',             'sessions',  'capability.sessions.list',             ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('sessions:revoke',           'sessions',  'capability.sessions.revoke',           ARRAY['SUPER_ADMIN','ADMIN']),
    ('sessions:revoke_all',       'sessions',  'capability.sessions.revoke_all',       ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Devices
    ('devices:list',              'devices',   'capability.devices.list',              ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('devices:revoke',            'devices',   'capability.devices.revoke',            ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Audit
    ('audit:view',                'audit',     'capability.audit.view',                ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('audit:export',              'audit',     'capability.audit.export',              ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Settings
    ('settings:read',             'settings',  'capability.settings.read',             ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('settings:write_otp',        'settings',  'capability.settings.write_otp',        ARRAY['SUPER_ADMIN']),
    ('settings:write_device_trust','settings', 'capability.settings.write_device_trust',ARRAY['SUPER_ADMIN']),
    ('settings:write_session',    'settings',  'capability.settings.write_session',    ARRAY['SUPER_ADMIN']),
    ('settings:write_mfa',        'settings',  'capability.settings.write_mfa',        ARRAY['SUPER_ADMIN']),
    ('settings:write_grant',      'settings',  'capability.settings.write_grant',      ARRAY['SUPER_ADMIN']),
    ('settings:write_break_glass','settings',  'capability.settings.write_break_glass',ARRAY['SUPER_ADMIN']),
    ('settings:write_audit',      'settings',  'capability.settings.write_audit',      ARRAY['SUPER_ADMIN']),

    -- Domain Break-Glass
    ('break_glass:activate',      'break_glass','capability.break_glass.activate',     ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Recovery
    ('recovery:initiate_for_user','recovery',  'capability.recovery.initiate_for_user',ARRAY['SUPER_ADMIN']),
    ('recovery:complete',         'recovery',  'capability.recovery.complete',         ARRAY['SUPER_ADMIN','ADMIN']),

    -- Domain Self (any authenticated admin)
    ('self:password_change',      'self',      'capability.self.password_change',      ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('self:passkey_manage',       'self',      'capability.self.passkey_manage',       ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('self:totp_manage',          'self',      'capability.self.totp_manage',          ARRAY['SUPER_ADMIN','ADMIN','MANAGER']),
    ('self:recovery_codes_regenerate','self',  'capability.self.recovery_codes_regen', ARRAY['SUPER_ADMIN','ADMIN','MANAGER'])
ON CONFLICT (key) DO NOTHING;

-- ============================================================
-- 3. Per-user capability grants — append-only audit-friendly model.
--    Active grants = revoked_at IS NULL. The partial index speeds up
--    "list active capabilities for user X" lookups (hot path).
-- ============================================================
CREATE TABLE IF NOT EXISTS account_capability_grants (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    capability_key      VARCHAR(80) NOT NULL,
    scope               JSONB,
    granted_by          UUID REFERENCES users(id),
    granted_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at          TIMESTAMPTZ,
    revoked_by          UUID REFERENCES users(id),
    granted_for_role    VARCHAR(20),
    motif               TEXT
);

CREATE INDEX IF NOT EXISTS idx_acg_user_active
    ON account_capability_grants(user_id, capability_key)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_acg_capability_active
    ON account_capability_grants(capability_key)
    WHERE revoked_at IS NULL;
