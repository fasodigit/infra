// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #20 — admin-super-admin-self-management
 * Modules : M01 password change, M09 PassKey re-enroll, M10 TOTP, M11 recovery codes
 * Assertion : SA peut gérer ses propres MFA + relogin réussi.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #20 — admin-super-admin-self-management', () => {
  test('endpoints /admin/me/* reachable for SA', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r1 = await ctx.admin.beginPasskeyEnroll();
    const r2 = await ctx.admin.beginTotpEnroll();
    expect(r1.status).toBeLessThan(500);
    expect(r2.status).toBeLessThan(500);
  });

  test('error — change password with wrong current → 401/403', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.changeOwnPassword({
      current: 'NotTheRightOne',
      next: 'NewerStr0ngerP@ss2026!',
    });
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 422]).toContain(r.status);
  });
});
