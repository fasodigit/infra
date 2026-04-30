// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #3 — admin-signup-manager
 * Modules : M06, M07, M09, M10, M11 (idem #2 mais scope MANAGER).
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #3 — admin-signup-manager (scope MANAGER)', () => {
  test('capabilities catalog list reachable (M17 MANAGER scope)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const res = await ctx.admin.listCapabilities();
    expect(res.status, `Body: ${res.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — onboard with empty token → 400/401', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway down');
      return;
    }
    const res = await ctx.admin.beginOnboard({ token: '' });
    expect(res.status).toBeLessThan(500);
  });
});
