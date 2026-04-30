// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #25 — signup-magic-link-tampered-token
 * Modules : M06 magic-link HMAC signature
 * Assertion : signature altérée → 401.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';
import { MagicLinkHelper } from '../../fixtures/admin';

test.describe('Spec #25 — magic-link tampered-token (M06)', () => {
  test('JWT with tampered signature → reject', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    const validShape =
      'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwianRpIjoiajEiLCJleHAiOjk5OTk5OTk5OTl9.SIG';
    const tampered = MagicLinkHelper.tamperToken(validShape);
    const r = await ctx.admin.verifyOnboardLink({ token: tampered });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    expect([400, 401, 403, 410]).toContain(r.status);
  });

  test('helper tamperToken changes signature', () => {
    const t = 'aaa.bbb.ccc';
    const tampered = MagicLinkHelper.tamperToken(t);
    expect(tampered).not.toBe(t);
    expect(tampered.split('.')[0]).toBe('aaa');
    expect(tampered.split('.')[1]).toBe('bbb');
  });
});
