// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #10 — admin-revoke-role
 * Modules : M16, M19 SUPER-ADMIN protection (last-SA), M22 audit
 * Assertion : revoke OK sauf last SUPER-ADMIN.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #10 — admin-revoke-role (M16+M19+M22)', () => {
  test('revoke endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.revokeRole(
      '00000000-0000-4000-8000-00000fakeuser1',
      { role: 'MANAGER', reason: 'E2E test revoke' },
    );
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — revoke last SUPER-ADMIN must be blocked (M19)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    // Tenter revoke de Aminata par elle-même → doit refuser via M19.
    const r = await ctx.admin.revokeRole(ctx.aminata.kratosId, {
      role: 'SUPER_ADMIN',
      reason: 'Tentative auto-revoke (interdit par M19 si last SA)',
    });
    expect(r.status).toBeLessThan(500);
    // 403/409/422 attendu selon impl ; pas un 200 silent.
    expect([400, 401, 403, 409, 422]).toContain(r.status);
  });
});
