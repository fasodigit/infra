-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V13__crypto_argon2_columns.sql
-- Phase 4.b.3 — Crypto upgrade Argon2id + HMAC pepper.
--
-- Adds the metadata columns required by `CryptographicHashService` so we can:
--   * Identify which hash algorithm produced each stored hash (lazy re-hash on
--     login when the algo is legacy bcrypt / SHA-* / etc.).
--   * Track which Vault pepper version was used (HMAC key rotation: v1 → v2
--     coexists during rotation windows; new writes always use the latest).
--   * Persist the Argon2id parameters (m, t, p, version) so verifications
--     stay deterministic even after defaults move (cf. plan §1).
--
-- Tables touched:
--   recovery_codes        (XXXX-XXXX, hashed by RecoveryCodeService)
--   admin_otp_requests    (created lazily on first OTP-persistent flow; we
--                          guard with DO/IF EXISTS so this migration is safe
--                          even when the table is not yet present).
--   users                 (Kratos owns the Argon2 hash today — we still keep
--                          the columns so a future swap to auth-ms-managed
--                          hashes is a flag-flip, not a schema change).

-- ── recovery_codes ─────────────────────────────────────────────────────────
ALTER TABLE recovery_codes ADD COLUMN IF NOT EXISTS pepper_version SMALLINT DEFAULT 1;
ALTER TABLE recovery_codes ADD COLUMN IF NOT EXISTS hash_algo VARCHAR(16) DEFAULT 'argon2id';

-- ── admin_otp_requests (optional; created in a future stream) ──────────────
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables
               WHERE table_name = 'admin_otp_requests') THEN
        EXECUTE 'ALTER TABLE admin_otp_requests
                 ADD COLUMN IF NOT EXISTS pepper_version SMALLINT DEFAULT 1';
        EXECUTE 'ALTER TABLE admin_otp_requests
                 ADD COLUMN IF NOT EXISTS hash_algo VARCHAR(16) DEFAULT ''argon2id''';
    END IF;
END
$$;

-- ── users ──────────────────────────────────────────────────────────────────
ALTER TABLE users ADD COLUMN IF NOT EXISTS hash_algo VARCHAR(16) DEFAULT 'bcrypt';
ALTER TABLE users ADD COLUMN IF NOT EXISTS hash_params JSONB;
ALTER TABLE users ADD COLUMN IF NOT EXISTS hash_pepper_version SMALLINT DEFAULT 0;

COMMENT ON COLUMN users.hash_algo IS 'argon2id (recommended) | bcrypt (legacy, lazy-rehashed on login)';
COMMENT ON COLUMN users.hash_params IS 'Argon2 parameters used for the current hash, JSON: {m,t,p,version}';
COMMENT ON COLUMN users.hash_pepper_version IS '0 = no pepper (legacy) ; ≥1 = HMAC pepper version stored in Vault';
