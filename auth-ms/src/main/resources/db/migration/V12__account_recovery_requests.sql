-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V12__account_recovery_requests.sql
-- Delta amendment 2026-04-30: account recovery flows (cf. delta §5).
--
--   SELF             — user lost MFA, requested via /admin/auth/recovery/initiate
--                      with magic-link JWT 30 min, single-use.
--   ADMIN_INITIATED  — SUPER_ADMIN reset target user's MFA + token 8 chiffres
--                      TTL 1h. Target user receives token by email.
--
-- After a successful recovery, the user must re-enrol MFA before any other
-- privileged action. The `must_reenroll_mfa` boolean on `users` materialises
-- that requirement (see AccountRecoveryService.completeRecovery).

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS must_reenroll_mfa BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE IF NOT EXISTS account_recovery_requests (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id),
    initiated_by    UUID REFERENCES users(id),
    recovery_type   VARCHAR(20) NOT NULL,
    token_hash      VARCHAR(255) NOT NULL UNIQUE,
    motif           TEXT,
    status          VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    used_at         TIMESTAMPTZ,
    expires_at      TIMESTAMPTZ NOT NULL,
    trace_id        VARCHAR(32),
    CHECK (recovery_type IN ('SELF', 'ADMIN_INITIATED')),
    CHECK (status IN ('PENDING', 'USED', 'EXPIRED', 'REJECTED'))
);

CREATE INDEX IF NOT EXISTS idx_recovery_pending
    ON account_recovery_requests(user_id, status)
    WHERE status = 'PENDING';

CREATE INDEX IF NOT EXISTS idx_recovery_user_created
    ON account_recovery_requests(user_id, created_at DESC);
