-- SPDX-License-Identifier: AGPL-3.0-or-later
-- V15__login_history.sql
-- Phase 4.b.6 — Risk-based scoring MVP (cf. SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 5).
--
-- Persistent log of every successful or assessed login attempt feeding the
-- RiskScoringService:
--   * IP address kept in clear (auditable, required for geo-distance signal,
--     scrubbed by the audit retention policy — Loi 010-2004 BF, 7y)
--   * GeoIP resolved fields (country / city / lat / lon) — populated when the
--     MaxMind GeoLite2-City.mmdb is loaded ; left NULL otherwise (fail-open).
--   * Device fingerprint hash (SHA-256 of UA + IP/24 + Accept-Language ; cf.
--     DeviceTrustService.computeFingerprint).
--   * risk_score (0-100) and risk_decision (ALLOW / STEP_UP / BLOCK) — the
--     decision applied to the login flow.
--   * trace_id — propagated from MDC for cross-service correlation in Tempo.
--
-- Indexes:
--   idx_login_history_user_time  — hot path for "last login per user" lookup
--                                  (geoDistanceScore retrieves the most
--                                  recent row to compute the haversine).
--   idx_login_history_high_risk  — partial index for analytics / threat-intel
--                                  queries (STEP_UP + BLOCK only).
CREATE TABLE IF NOT EXISTS login_history (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ip_address          VARCHAR(45) NOT NULL,
    ip_country_iso2     VARCHAR(2),
    ip_city             VARCHAR(120),
    ip_lat              DOUBLE PRECISION,
    ip_lon              DOUBLE PRECISION,
    user_agent          TEXT,
    device_fingerprint  VARCHAR(64),
    risk_score          SMALLINT NOT NULL DEFAULT 0,
    risk_decision       VARCHAR(30) NOT NULL,
    trace_id            VARCHAR(32),
    occurred_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (risk_decision IN ('ALLOW', 'STEP_UP', 'BLOCK')),
    CHECK (risk_score BETWEEN 0 AND 100)
);

CREATE INDEX IF NOT EXISTS idx_login_history_user_time
    ON login_history(user_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_login_history_high_risk
    ON login_history(risk_decision)
    WHERE risk_decision IN ('STEP_UP', 'BLOCK');
