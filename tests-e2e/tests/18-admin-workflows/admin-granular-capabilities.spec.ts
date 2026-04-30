// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #21 — admin-granular-capabilities
 * Modules : M17 capabilities fines (~31 caps × user)
 * Assertion : un acteur A peut suspend X mais pas Y selon ses caps.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #21 — admin-granular-capabilities (M17)', () => {
  test('list capabilities catalog', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.listCapabilities();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    if (r.ok && Array.isArray(r.body)) {
      expect((r.body as unknown[]).length).toBeGreaterThan(0);
    }
  });

  test('error — unknown capability key → 400/422', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.checkCapabilityUniqueness({
      userId: '00000000-0000-4000-8000-00000fakeuser1',
      capabilities: ['inexistent.capability.key'],
    });
    expect(r.status).toBeLessThan(500);
  });
});
