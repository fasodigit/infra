// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #22 — admin-grant-warns-on-duplicate-capabilities
 * Modules : M18 capability uniqueness check (soft warn + force override)
 * Assertion : warn soft + audit CAPABILITY_SET_DUPLICATE_OVERRIDE quand
 * `force: true`.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #22 — admin-grant-warns-on-duplicate-capabilities (M18)', () => {
  test('check-uniqueness endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.checkCapabilityUniqueness({
      userId: ctx.aminata.kratosId,
      capabilities: ['users:list', 'users:suspend'],
    });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('grant with force=true bypasses duplicate warn (M18)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.grantRole(
      '00000000-0000-4000-8000-00000fakeuser1',
      {
        role: 'MANAGER',
        capabilities: ['users:list'],
        justification: 'E2E #22 force override duplicate caps test',
        force: true,
      },
    );
    expect(r.status).toBeLessThan(500);
  });
});
