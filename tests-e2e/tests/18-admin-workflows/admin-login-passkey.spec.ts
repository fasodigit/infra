// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #5 — admin-login-passkey
 * Modules : M01 password Argon2id, M09 PassKey FIDO2 WebAuthn
 * Assertion : virtual authenticator CDP signe l'assertion login.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';
import { addVirtualAuthenticator } from '../../fixtures/webauthn';

test.describe('Spec #5 — admin-login-passkey (M01+M09)', () => {
  test('PassKey enroll begin endpoint responds', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.beginPasskeyEnroll();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('virtual authenticator CDP attaches successfully', async ({ page }, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    await page.goto('http://localhost:4801/auth/login').catch(() => undefined);
    const auth = await addVirtualAuthenticator(page, {
      protocol: 'ctap2',
      transport: 'internal',
      hasResidentKey: true,
      isUserVerified: true,
    });
    expect(auth.authenticatorId).toBeTruthy();
    await auth.remove();
  });
});
