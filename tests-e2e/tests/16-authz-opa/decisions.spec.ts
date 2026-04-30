// SPDX-License-Identifier: AGPL-3.0-or-later
// FASO 16-authz-opa — role × route authorization matrix.
//
// Strategy: query OPA decision API directly (POST :8181/v1/data/faso/authz/allow).
// When ARMAGEDDON's ext_authz filter is wired into the route filter chain
// (next sprint), these tests will be promoted to gateway-driven E2E.
//
// Why direct OPA testing now? — proves the policy contract is correct
// independent of gateway plumbing, isolates OPA bugs from filter bugs.

import { test, expect, request } from '@playwright/test';
import { FEATURE_MATRIX, cellsForFeature, coverageStats } from '../../fixtures/feature-matrix';
import type { ActorRole } from '../../fixtures/actors';

const OPA = process.env.OPA_URL ?? 'http://127.0.0.1:8181';

interface OpaInput {
  path:    string;
  method:  string;
  headers: Record<string, string>;
  jwt_payload?: { sub: string; roles: string[] };
}

async function decide(input: OpaInput): Promise<boolean> {
  const api = await request.newContext();
  const r = await api.post(`${OPA}/v1/data/faso/authz/allow`, {
    headers: { 'Content-Type': 'application/json' },
    data: { input },
  });
  expect(r.status()).toBe(200);
  const body = await r.json();
  await api.dispose();
  return body.result === true;
}

// ── Coverage smoke — assert matrix size matches design ────────────────
test('[@smoke] FEATURE_MATRIX has at least 150 cells across 22 features', async () => {
  expect(FEATURE_MATRIX.length).toBeGreaterThanOrEqual(150);
  const stats = coverageStats();
  expect(stats.length).toBeGreaterThanOrEqual(22);
  for (const s of stats) {
    expect(s.allow + s.deny).toBe(8); // 8 roles total
  }
});

// ── Public routes — anonymous allowed ─────────────────────────────────
test('anonymous → / → ALLOW (public landing)', async () => {
  expect(await decide({ path: '/', method: 'GET', headers: {} })).toBe(true);
});
test('anonymous → /auth/login → ALLOW (public)', async () => {
  expect(await decide({ path: '/auth/login', method: 'POST', headers: {} })).toBe(true);
});
test('anonymous → /api/annonces POST → DENY (write requires auth)', async () => {
  expect(await decide({ path: '/api/annonces', method: 'POST', headers: {} })).toBe(false);
});
test('anonymous → /admin/users → DENY', async () => {
  expect(await decide({ path: '/admin/users', method: 'GET', headers: {} })).toBe(false);
});

// ── Helper: build OPA input from a matrix cell + JWT claims ───────────
function inputFor(cell: { path?: string; method?: string }, role: ActorRole) {
  return {
    path:    cell.path    ?? '/',
    method:  cell.method  ?? 'GET',
    headers: { authorization: 'Bearer test' },
    jwt_payload: { sub: `${role}-1`, roles: [role] },
  };
}

// ── Per-feature parameterised matrix tests ────────────────────────────
//
// We exercise a subset of features known to have crisp authz contracts
// (those with explicit Rego rules). For features wired only via Keto
// relations (not yet seeded in dev), we mark fixme until the data layer
// is in place.

const FEATURES_WITH_REGO_RULES = [
  'marketplace-browse',
  'marketplace-post-offer',
  'marketplace-post-demand',
  'order-create',
  'order-accept',
  'halal-certify',
  'vaccine-record',
  'admin-dashboard',
  'admin-impersonate',
  'audit-log-read',
];

for (const feature of FEATURES_WITH_REGO_RULES) {
  for (const cell of cellsForFeature(feature as any)) {
    const expected = cell.expected;
    test(`[${feature}] ${cell.role} → ${cell.method} ${cell.path} → ${expected.toUpperCase()}`, async () => {
      const allowed = await decide(inputFor(cell, cell.role));
      if (expected === 'allow') {
        expect(allowed, `expected ALLOW for ${cell.role} on ${cell.path}`).toBe(true);
      } else {
        expect(allowed, `expected DENY for ${cell.role} on ${cell.path}`).toBe(false);
      }
    });
  }
}

// ── Negative path — no jwt_payload → always deny on /api ──────────────
test('missing jwt_payload → DENY on /api/annonces POST', async () => {
  expect(await decide({
    path: '/api/annonces',
    method: 'POST',
    headers: {}, // no Authorization
  })).toBe(false);
});

// ── Edge case — empty roles array (verified user, no role) ────────────
test('user with empty roles → DENY on POST /api/annonces', async () => {
  expect(await decide({
    path: '/api/annonces',
    method: 'POST',
    headers: { authorization: 'Bearer t' },
    jwt_payload: { sub: 'u-x', roles: [] },
  })).toBe(false);
});

// ── Latency — OPA decision p99 < 50ms (10 samples) ───────────────────
test('OPA decision p99 < 50ms (10 samples)', async () => {
  const samples: number[] = [];
  for (let i = 0; i < 10; i++) {
    const t0 = performance.now();
    await decide({
      path: '/api/annonces',
      method: 'POST',
      headers: { authorization: 'Bearer t' },
      jwt_payload: { sub: 'u', roles: ['eleveur'] },
    });
    samples.push(performance.now() - t0);
  }
  samples.sort((a, b) => a - b);
  const p99 = samples[samples.length - 1] ?? 0;
  test.info().annotations.push({ type: 'p99', description: `${p99.toFixed(1)}ms` });
  expect(p99).toBeLessThan(50);
});
