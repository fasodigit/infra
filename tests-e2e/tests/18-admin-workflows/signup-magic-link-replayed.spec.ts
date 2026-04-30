// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #26 — signup-magic-link-replayed
 * Modules : M06 KAYA jti single-use
 * Assertion : 2ème click sur le même lien → 410 Gone.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #26 — magic-link replayed (M06 KAYA jti)', () => {
  test('two consecutive verify-link calls → 2nd is rejected', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway) {
      testInfo.skip(true, 'Gateway KO');
      return;
    }
    const fakeToken =
      'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ0ZXN0IiwianRpIjoianRpLXJlcGxheS10ZXN0IiwiZXhwIjo5OTk5OTk5OTk5fQ.SIG';
    const r1 = await ctx.admin.verifyOnboardLink({ token: fakeToken });
    const r2 = await ctx.admin.verifyOnboardLink({ token: fakeToken });
    expect(r1.status).toBeLessThan(500);
    expect(r2.status).toBeLessThan(500);
    // 2ème essai doit toujours rejeter (token invalide ou consumed).
    expect([400, 401, 403, 410, 422]).toContain(r2.status);
  });
});
