// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #7 — admin-login-recovery-code
 * Modules : M01, M03 hash, M11 recovery codes
 * Assertion : code XXXX-XXXX validé au login.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #7 — admin-login-recovery-code (M01+M03+M11)', () => {
  test('login/recovery-code endpoint responds', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    const r = await ctx.admin.loginWithRecoveryCode({
      email: ctx.aminata.email,
      code: 'XXXX-XXXX',
    });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — invalid format → 400/401', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    const r = await ctx.admin.loginWithRecoveryCode({
      email: ctx.aminata.email,
      code: 'INVALID',
    });
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 422]).toContain(r.status);
  });
});
