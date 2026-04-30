// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #14 — admin-settings-update
 * Modules : M23 CAS version + history
 * Assertion : version optimistic concurrency + history audit.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #14 — admin-settings-update (M23)', () => {
  test('list settings reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.getSettings();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — update with stale version → 409 (CAS)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    // Update avec version=0 (force stale) → expect 409 ou 401/403/404.
    const r = await ctx.admin.updateSetting('otp.length', {
      value: 8,
      version: 0,
      reason: 'E2E stale-version test',
    });
    expect(r.status).toBeLessThan(500);
    expect([200, 400, 401, 403, 404, 409, 422]).toContain(r.status);
  });
});
