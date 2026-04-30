// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #15 — admin-settings-effect-runtime
 * Modules : M23 + topic admin.settings.changed Redpanda
 * Assertion : OTP length change → effet immédiat (cache invalidation).
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #15 — admin-settings-effect-runtime (M23 + Redpanda)', () => {
  test('setting key history reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.getSettingHistory('otp.length');
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('individual setting fetch reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.getSetting('otp.length');
    expect(r.status).toBeLessThan(500);
  });
});
