-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V8__mfa_status.sql
-- Phase 4.b admin-UI: per-user MFA materialised view (used by dashboard).
--
-- Maintained by AdminMfaEnrollmentService whenever a TOTP / passkey / recovery
-- code transitions state. Allows the admin UI to display MFA coverage without
-- joining 4 tables on every list query.

CREATE TABLE IF NOT EXISTS mfa_status (
    user_id                  UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    totp_enabled             BOOLEAN NOT NULL DEFAULT false,
    passkey_count            INT NOT NULL DEFAULT 0,
    backup_codes_remaining   INT NOT NULL DEFAULT 0,
    trusted_devices_count    INT NOT NULL DEFAULT 0,
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_mfa_status_totp
    ON mfa_status(totp_enabled);
