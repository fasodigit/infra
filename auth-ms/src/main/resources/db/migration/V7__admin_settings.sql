-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V7__admin_settings.sql
-- Phase 4.b admin-UI: Configuration Center (38 settings × 6 categories).
--
-- Categories: otp, device_trust, session, mfa, grant, break_glass, audit
--
-- Updates use optimistic concurrency: clients pass `version` in the PUT body
-- and AdminSettingsService refuses non-matching values (CAS).
-- Every successful update writes a row to admin_settings_history.

CREATE TABLE IF NOT EXISTS admin_settings (
    key                    VARCHAR(120) PRIMARY KEY,
    value                  JSONB NOT NULL,
    value_type             VARCHAR(20) NOT NULL
                           CHECK (value_type IN ('INT','LONG','DOUBLE','BOOLEAN','STRING','JSON')),
    category               VARCHAR(40) NOT NULL,
    min_value              JSONB,
    max_value              JSONB,
    default_value          JSONB NOT NULL,
    description            TEXT,
    required_role_to_edit  VARCHAR(40) NOT NULL DEFAULT 'SUPER_ADMIN',
    version                BIGINT NOT NULL DEFAULT 1,
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by             UUID
);

CREATE INDEX IF NOT EXISTS idx_admin_settings_category
    ON admin_settings(category);

CREATE TABLE IF NOT EXISTS admin_settings_history (
    id          BIGSERIAL PRIMARY KEY,
    key         VARCHAR(120) NOT NULL,
    version     BIGINT NOT NULL,
    old_value   JSONB,
    new_value   JSONB NOT NULL,
    motif       TEXT NOT NULL,
    changed_by  UUID NOT NULL,
    changed_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    trace_id    VARCHAR(64),
    UNIQUE (key, version)
);

CREATE INDEX IF NOT EXISTS idx_admin_settings_history_key
    ON admin_settings_history(key, changed_at DESC);

-- ============================================================
-- SEED: 38 baseline parameters spread over 6 categories.
-- ============================================================
INSERT INTO admin_settings (key, value, value_type, category, min_value, max_value, default_value, description) VALUES
    -- otp (5)
    ('otp.length',                '8',     'INT',     'otp',         '6',     '10',    '8',    'Number of digits in OTP codes'),
    ('otp.ttl_seconds',           '300',   'INT',     'otp',         '60',    '900',   '300',  'OTP validity window in seconds'),
    ('otp.rate_limit_per_5min',   '3',     'INT',     'otp',         '1',     '10',    '3',    'Max OTP issues per user per 5 min'),
    ('otp.lock_after_fails',      '5',     'INT',     'otp',         '3',     '20',    '5',    'Lock OTP after N consecutive verify failures'),
    ('otp.lock_duration_seconds', '900',   'INT',     'otp',         '60',    '7200',  '900',  'Lock duration after threshold reached'),

    -- device_trust (5)
    ('device_trust.enabled',           'true',    'BOOLEAN', 'device_trust', null,  null,    'true',     'Globally enable trusted-device feature'),
    ('device_trust.ttl_days',          '30',      'INT',     'device_trust', '1',   '365',   '30',       'How long a device stays trusted'),
    ('device_trust.max_per_user',      '10',      'INT',     'device_trust', '1',   '50',    '10',       'Max trusted devices per user'),
    ('device_trust.fingerprint_strict','false',   'BOOLEAN', 'device_trust', null,  null,    'false',    'Strict fingerprint matching (no /24 grace)'),
    ('device_trust.require_mfa_first', 'true',    'BOOLEAN', 'device_trust', null,  null,    'true',     'Device cannot be trusted before first MFA'),

    -- session (6)
    ('session.max_per_user',           '3',       'INT',     'session', '1',   '20',    '3',     'Max concurrent sessions per user'),
    ('session.access_token_ttl_minutes','15',     'INT',     'session', '5',   '120',   '15',    'Access JWT TTL minutes'),
    ('session.refresh_token_ttl_days', '7',       'INT',     'session', '1',   '30',    '7',     'Refresh JWT TTL days'),
    ('session.idle_timeout_minutes',   '30',      'INT',     'session', '5',   '480',   '30',    'Idle timeout before force-logout'),
    ('session.absolute_timeout_hours', '12',      'INT',     'session', '1',   '24',    '12',    'Absolute session lifetime'),
    ('session.kill_on_password_change','true',    'BOOLEAN', 'session', null,  null,    'true',  'Revoke all sessions when password changes'),

    -- mfa (8)
    ('mfa.required_for_admin',         'true',    'BOOLEAN', 'mfa', null,  null,    'true',  'MFA mandatory for ADMIN+ roles'),
    ('mfa.required_for_super_admin',   'true',    'BOOLEAN', 'mfa', null,  null,    'true',  'MFA mandatory for SUPER_ADMIN role'),
    ('mfa.totp_window',                '1',       'INT',     'mfa', '0',   '5',     '1',     'TOTP code drift window (steps)'),
    ('mfa.totp_step_seconds',          '30',      'INT',     'mfa', '15',  '60',    '30',    'TOTP code generation interval'),
    ('mfa.passkey_enabled',            'true',    'BOOLEAN', 'mfa', null,  null,    'true',  'Enable WebAuthn / PassKey enrollment'),
    ('mfa.recovery_codes_count',       '10',      'INT',     'mfa', '5',   '20',    '10',    'Count of recovery codes to generate'),
    ('mfa.recovery_codes_ttl_days',    '365',     'INT',     'mfa', '30',  '730',   '365',   'Recovery code lifetime'),
    ('mfa.enforce_passkey_for_super_admin','false','BOOLEAN','mfa', null,  null,    'false', 'Force PassKey (no TOTP) for SUPER_ADMIN'),

    -- grant (5)
    ('grant.dual_control_required',    'true',    'BOOLEAN', 'grant', null,  null,    'true',  'Require 2 SA approvals for sensitive grants'),
    ('grant.expiry_default_days',      '90',      'INT',     'grant', '1',   '365',   '90',    'Default grant expiry in days'),
    ('grant.expiry_max_days',          '180',     'INT',     'grant', '30',  '365',   '180',   'Maximum grant expiry'),
    ('grant.require_otp_on_request',   'true',    'BOOLEAN', 'grant', null,  null,    'true',  'OTP needed on grant request'),
    ('grant.require_otp_on_approval',  'true',    'BOOLEAN', 'grant', null,  null,    'true',  'OTP needed on grant approval'),

    -- break_glass (5)
    ('break_glass.enabled',            'true',    'BOOLEAN', 'break_glass', null,  null,    'true',   'Enable break-glass elevation flow'),
    ('break_glass.ttl_seconds',        '14400',   'INT',     'break_glass', '900', '28800', '14400',  'Break-glass elevation TTL (4h default)'),
    ('break_glass.require_justification','true',  'BOOLEAN', 'break_glass', null,  null,    'true',   'Justification text mandatory'),
    ('break_glass.notify_all_super_admins','true','BOOLEAN', 'break_glass', null,  null,    'true',   'Email all SAs on activation'),
    ('break_glass.max_per_user_per_month','3',    'INT',     'break_glass', '1',   '20',    '3',      'Per-user activation cap (per month)'),

    -- audit (4)
    ('audit.immutable_mode',           'true',    'BOOLEAN', 'audit', null,  null,    'true',  'Block UPDATE/DELETE on audit_log via trigger'),
    ('audit.retention_days',           '2555',    'INT',     'audit', '365', '3650',  '2555',  'Loi 010-2004 BF retention period (7y)'),
    ('audit.export_csv_enabled',       'true',    'BOOLEAN', 'audit', null,  null,    'true',  'Allow CSV export endpoint'),
    ('audit.publish_to_redpanda',      'true',    'BOOLEAN', 'audit', null,  null,    'true',  'Async publish audit events to Redpanda')
ON CONFLICT (key) DO NOTHING;
