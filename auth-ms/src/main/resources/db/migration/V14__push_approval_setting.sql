-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Phase 4.b.5 — Seed mfa.push_approval_enabled in admin_settings
-- (Configuration Center entry for sovereign WebSocket push-approval MFA)

INSERT INTO admin_settings(key, value, value_type, category, default_value, required_role_to_edit, version, updated_at, updated_by)
VALUES ('mfa.push_approval_enabled', 'true', 'BOOLEAN', 'mfa', 'true', 'SUPER-ADMIN', 1, now(), NULL)
ON CONFLICT (key) DO NOTHING;
