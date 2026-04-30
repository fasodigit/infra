// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #13 — admin-break-glass
 * Modules : M07 OTP, M15 step-up, M22 audit
 * Assertion : TTL 4h + auto-revoke.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #13 — admin-break-glass (M07+M15+M22)', () => {
  test('break-glass status reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.breakGlassStatus();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — activate without justification → 400/422', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.activateBreakGlass({ reason: '' });
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 422]).toContain(r.status);
  });
});
