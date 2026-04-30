-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V4__recovery_codes.sql
-- Phase 4.b admin-UI: single-use MFA recovery codes (10 per user / motif).
--
-- code_hash is bcrypt($2a$ ≥ cost 12) of the human-presentable XXXX-XXXX code.
-- Once code is "used" (used_at IS NOT NULL) it is permanently invalidated.

CREATE TABLE IF NOT EXISTS recovery_codes (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash     VARCHAR(120) NOT NULL,
    motif         VARCHAR(255),
    generated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    used_at       TIMESTAMPTZ,
    expires_at    TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '365 days')
);

-- Partial index: only un-used codes participate in lookup.
CREATE INDEX IF NOT EXISTS idx_recovery_codes_user_unused
    ON recovery_codes(user_id)
    WHERE used_at IS NULL;
