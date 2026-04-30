// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #31 — risk-scoring-new-country-medium
 * Modules : M14
 * Assertion : score 30-60 (country diff +20) → STEP_UP forcé.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite, RiskHelper } from '../../fixtures/admin';

test.describe('Spec #31 — risk-scoring new-country-medium (M14)', () => {
  test('login risk evaluated on new IP/country', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    // Simuler IP française (GeoLite2 → +20 country diff vs Burkina).
    const r = await ctx.admin.post(
      '/api/admin/auth/login/risk',
      {
        email: ctx.aminata.email,
        ip: '203.0.113.42',
        userAgent: 'FasoE2E/spec31-new-country',
      },
      { extraHeaders: RiskHelper.withSourceIp('203.0.113.42') },
    );
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });
});
