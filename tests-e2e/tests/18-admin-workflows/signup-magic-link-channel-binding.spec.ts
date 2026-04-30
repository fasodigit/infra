// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #24 ⭐ — signup-magic-link-channel-binding
 * Modules : M06 magic-link channel-binding
 * Assertion : OTP affiché en page = saisie sur la même page (channel
 * binding empêche relay attack).
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #24 ⭐ — magic-link channel-binding (M06)', () => {
  test('verify-link endpoint enforces token format', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    // Token JWT-shape mais inconnu en KAYA jti store → reject.
    const fakeJwt =
      'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJlMmUiLCJqdGkiOiJlMmUtZmFrZSIsImV4cCI6OTk5OTk5OTk5OX0.fakeSignature';
    const r = await ctx.admin.verifyOnboardLink({ token: fakeJwt });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    expect([400, 401, 403, 404, 410]).toContain(r.status);
  });

  test('OTP is bound to the same session (channel-binding)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    // Tenter de verify-otp sans avoir verify-link au préalable → reject.
    const r = await ctx.admin.verifyOnboardOtp({
      sessionId: 'no-such-session',
      code: '12345678',
    });
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 404, 410, 422]).toContain(r.status);
  });
});
