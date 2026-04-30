-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V5__device_registrations.sql
-- Phase 4.b admin-UI: trusted device registry (device_trust feature).
--
-- fingerprint = SHA-256(UA + IP/24 + Accept-Language).
-- public_key_pem is OPTIONAL (used by passkey-bound device attestation).

CREATE TABLE IF NOT EXISTS device_registrations (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    fingerprint     VARCHAR(128) NOT NULL,
    device_type     VARCHAR(50),
    public_key_pem  TEXT,
    ua_string       VARCHAR(500),
    ip_address      VARCHAR(45),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at    TIMESTAMPTZ,
    trusted_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    UNIQUE (user_id, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_device_reg_user
    ON device_registrations(user_id)
    WHERE revoked_at IS NULL;
