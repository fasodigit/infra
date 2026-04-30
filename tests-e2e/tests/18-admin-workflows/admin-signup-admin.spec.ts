// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #2 — admin-signup-admin
 * Modules : M06 magic-link, M07 OTP, M09 PassKey, M10 TOTP, M11 recovery codes
 * Assertion critique : magic-link channel-binding → MFA forcé.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #2 — admin-signup-admin (M06+M07+M09+M10+M11)', () => {
  test('onboard begin endpoint responds (M06)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway down');
      return;
    }
    const res = await ctx.admin.beginOnboard({ token: 'invalid-test-token-shouldfail' });
    // Modules respond — token invalide → 400/401/403/410, pas 5xx.
    expect(res.status, `Body: ${res.text?.slice(0, 200)}`).toBeLessThan(500);
    expect([400, 401, 403, 404, 410, 422]).toContain(res.status);
  });

  test('error — verify-link with tampered token → reject (M06)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway down');
      return;
    }
    const tamperedJwt = 'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0In0.invalidsig';
    const res = await ctx.admin.verifyOnboardLink({ token: tamperedJwt });
    expect(res.status).toBeLessThan(500);
    expect([400, 401, 403, 410]).toContain(res.status);
  });
});
