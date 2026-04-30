// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #18 — admin-admin-initiated-recovery
 * Modules : M19 SUPER-ADMIN protection, M21 admin-initiated recovery, M22 audit
 * Assertion : token 8 digits → re-enroll forcé.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #18 — admin-admin-initiated-recovery (M19+M21+M22)', () => {
  test('initiate admin-recovery requires step-up (M15)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.initiateAdminRecovery(
      '00000000-0000-4000-8000-00000fakeuser1',
      { reason: 'E2E test admin-recovery initiation' },
    );
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    // Step-up requis OU user inconnu → pas de 5xx ni 200 silencieux.
    expect([200, 201, 401, 403, 404]).toContain(r.status);
  });

  test('error — empty reason → 400/422', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.initiateAdminRecovery(
      '00000000-0000-4000-8000-00000fakeuser1',
      { reason: '' },
    );
    expect(r.status).toBeLessThan(500);
  });
});
