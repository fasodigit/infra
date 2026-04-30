-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V11__super_admin_protection.sql
-- Delta amendment 2026-04-30: hard DB-level guard against destructive
-- operations on SUPER_ADMIN accounts.
--
-- Invariants enforced (cf. delta §2):
--   1. A SUPER_ADMIN account can NEVER be DELETEd.
--   2. A SUPER_ADMIN account can NEVER be suspended (suspended toggled
--      from false → true).
-- Service-level guards (SuperAdminProtectionService) enforce the
-- "last SUPER_ADMIN" rule and 403 mapping; this trigger is the
-- defense-in-depth backstop in case a row is touched directly via SQL.

CREATE OR REPLACE FUNCTION prevent_super_admin_delete() RETURNS trigger AS $$
DECLARE
    is_super BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1
        FROM user_roles ur
        JOIN roles r ON r.id = ur.role_id
        WHERE ur.user_id = OLD.id
          AND r.name = 'SUPER_ADMIN'
    ) INTO is_super;

    IF NOT is_super THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'SUPER_ADMIN_PROTECTION: cannot delete SUPER_ADMIN account %', OLD.id
            USING ERRCODE = '42501';
    END IF;

    IF TG_OP = 'UPDATE' AND NEW.suspended = true AND OLD.suspended = false THEN
        RAISE EXCEPTION 'SUPER_ADMIN_PROTECTION: cannot suspend SUPER_ADMIN account %', OLD.id
            USING ERRCODE = '42501';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_prevent_super_admin_destruction ON users;
CREATE TRIGGER trg_prevent_super_admin_destruction
    BEFORE DELETE OR UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION prevent_super_admin_delete();

-- Speeds up the "is there at least one active SUPER_ADMIN?" check in
-- SuperAdminProtectionService.assertNotLastSuperAdmin().
CREATE INDEX IF NOT EXISTS idx_users_super_admin_active
    ON users(id) WHERE suspended = false;
