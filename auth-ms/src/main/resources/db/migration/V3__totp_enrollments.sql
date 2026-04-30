-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V3__totp_enrollments.sql
-- Phase 4.b admin-UI: TOTP (RFC 6238) enrolment table.
--
-- secret_encrypted is the base32 TOTP shared secret protected with AES-256-GCM
-- via EncryptedStringConverter (uses JWT_KEY_ENCRYPTION_KEY).

CREATE TABLE IF NOT EXISTS totp_enrollments (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    secret_encrypted  TEXT NOT NULL,
    enrolled_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    disabled_at       TIMESTAMPTZ,
    last_used_at      TIMESTAMPTZ,
    UNIQUE (user_id)
);

CREATE INDEX IF NOT EXISTS idx_totp_enrollments_user
    ON totp_enrollments(user_id)
    WHERE disabled_at IS NULL;
