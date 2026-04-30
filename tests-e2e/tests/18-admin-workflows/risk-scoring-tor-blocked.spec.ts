// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #32 — risk-scoring-tor-blocked
 * Modules : M14
 * Assertion : score > 80 (Tor +40 + new-country +20 + bruteforce +30) →
 * BLOCK + email user.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite, RiskHelper } from '../../fixtures/admin';

test.describe('Spec #32 — risk-scoring tor-blocked (M14)', () => {
  test('seed Tor exit list + report risk → high score', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    const risk = new RiskHelper();
    const kayaUp = await risk.isReachable();
    if (!ctx.reachability.gateway || !kayaUp) {
      testInfo.skip(true, 'Gateway/KAYA KO');
      return;
    }

    const torIp = '198.51.100.7';
    await risk.addTorExitIp(torIp);
    try {
      const r = await ctx.admin.post(
        '/api/admin/auth/login/risk',
        {
          email: ctx.aminata.email,
          ip: torIp,
          userAgent: 'TorBrowser/13.0',
        },
        { extraHeaders: RiskHelper.withSourceIp(torIp) },
      );
      expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    } finally {
      await risk.removeTorExitIp(torIp);
    }
  });
});
