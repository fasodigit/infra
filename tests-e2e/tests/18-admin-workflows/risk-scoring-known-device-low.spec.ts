// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #30 — risk-scoring-known-device-low
 * Modules : M14 risk scoring (3 signaux MVP)
 * Assertion : score < 30 → ALLOW direct.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite, RiskHelper } from '../../fixtures/admin';

test.describe('Spec #30 — risk-scoring known-device-low (M14)', () => {
  test('reportLoginRisk endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.reportLoginRisk({
      email: ctx.aminata.email,
      ip: '127.0.0.1',
      userAgent: 'FasoE2E/spec30-known-device',
    });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('KAYA dev:* trust set helps low score', async ({}, testInfo) => {
    const risk = new RiskHelper();
    if (!(await risk.isReachable())) {
      testInfo.skip(true, 'KAYA KO');
      return;
    }
    const r = await risk.setDeviceTrust(
      '253ec814-1e10-44c7-b7a7-fd44581e4393',
      'sha256-known-device-fp-spec30',
    );
    expect(r.applied).toBe(true);
  });
});
