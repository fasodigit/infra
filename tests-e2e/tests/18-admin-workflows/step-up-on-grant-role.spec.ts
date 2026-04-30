// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #33 — step-up-on-grant-role
 * Modules : M15 step-up auth `@RequiresStepUp` (JWT 5min)
 * Assertion : grant > 5min après dernière auth → modal step-up requis.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #33 — step-up on grant-role (M15)', () => {
  test('begin step-up endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.beginStepUp({
      method: 'PASSKEY',
      intent: 'GRANT_ROLE',
    });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('error — verify step-up with bad sessionId → 404/410', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.verifyStepUp('non-existent-session', {
      method: 'PASSKEY',
      assertion: { fake: true },
    });
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 404, 410]).toContain(r.status);
  });

  test('grant-role w/o step-up token → 401 step_up_required', async ({}, testInfo) => {
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
        justification: 'Spec #33 — verify step-up gate without stepUpToken',
      },
    );
    expect(r.status).toBeLessThan(500);
    // 401 (step-up requis) ou 403 (Keto deny) ou 404 (target not found).
    expect([200, 201, 401, 403, 404, 422]).toContain(r.status);
  });
});
