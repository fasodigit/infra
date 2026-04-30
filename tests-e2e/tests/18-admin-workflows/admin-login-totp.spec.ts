// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #6 — admin-login-totp
 * Modules : M01 password, M04 AES-256-GCM TOTP secret, M10 RFC 6238 TOTP
 * Assertion : code 6 digits otplib match.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';
import { TotpGen } from '../../fixtures/totp';

test.describe('Spec #6 — admin-login-totp (M01+M04+M10)', () => {
  test('TotpGen produces 6-digit code (M10 RFC 6238)', async () => {
    const t = TotpGen.random();
    const c = t.code();
    expect(c).toMatch(/^\d{6}$/);
    expect(t.verify(c)).toBe(true);
  });

  test('TOTP enroll begin endpoint responds', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.beginTotpEnroll();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — finish enroll with wrong code → reject', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.finishTotpEnroll({ code: '000000' });
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 422]).toContain(r.status);
  });
});
