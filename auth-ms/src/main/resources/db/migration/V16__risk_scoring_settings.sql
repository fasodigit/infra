-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V16__risk_scoring_settings.sql
-- Phase 4.b.6 — Risk-based scoring MVP : seed the 2 thresholds that drive the
-- decision tree of `RiskScoringService.decide(score)` in the `mfa` category
-- of the Configuration Center (rendered by frontend pages-v2/settings.page.ts).
--
--   risk.score_threshold_step_up  — score >= this triggers STEP_UP MFA
--                                    (default 30, range 0-100)
--   risk.score_threshold_block    — score >= this triggers BLOCK + alert
--                                    (default 80, range 0-100)
--
-- Both are SUPER_ADMIN-only edits (`required_role_to_edit = 'SUPER_ADMIN'`).
-- Values are stored as JSONB ints to remain compatible with the existing
-- AdminSettingsService.validateValue (case 'INT'/'LONG').

INSERT INTO admin_settings (
    key, value, value_type, category,
    min_value, max_value, default_value,
    required_role_to_edit, version, updated_at, description
) VALUES
    ('risk.score_threshold_step_up', '30', 'INT', 'mfa',
     '0', '100', '30',
     'SUPER_ADMIN', 1, now(),
     'Risk score (0-100) above which a step-up MFA is required (forces MFA even on a trusted device). MVP signals: device fingerprint match, geo-IP distance, recent brute-force.'),
    ('risk.score_threshold_block', '80', 'INT', 'mfa',
     '0', '100', '80',
     'SUPER_ADMIN', 1, now(),
     'Risk score (0-100) above which the login is blocked outright (HTTP 403, audit LOGIN_BLOCKED_HIGH_RISK, email user).')
ON CONFLICT (key) DO NOTHING;
