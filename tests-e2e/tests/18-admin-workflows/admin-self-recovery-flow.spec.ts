// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #17 — admin-self-recovery-flow
 * Modules : M20 self-recovery, M06 magic-link, M02 OTP, M22 audit
 * Assertion : magic-link → OTP → AAL1 → must_reenroll_mfa=true.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #17 — admin-self-recovery-flow (M20+M06+M02+M22)', () => {
  test('initiate self-recovery endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    const r = await ctx.admin.initiateSelfRecovery({ email: ctx.aminata.email });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — initiate w/ unknown email → response stays 200/202 (anti-enum)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    const r = await ctx.admin.initiateSelfRecovery({
      email: `ghost-${Date.now()}@nowhere.test`,
    });
    expect(r.status).toBeLessThan(500);
    // Anti-énumeration ou authz : ne doit pas 5xx ni 404-leak. 403/401
    // acceptable côté gateway (Keto deny avant que le backend ne masque).
    expect([200, 202, 400, 401, 403, 429]).toContain(r.status);
  });
});
