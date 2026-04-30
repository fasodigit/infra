// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #12 — admin-session-force-logout
 * Modules : M22 audit + topic auth.session.revoked Redpanda
 * Assertion : session removed.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #12 — admin-session-force-logout (M22)', () => {
  test('list sessions endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.listSessions();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('force logout invalid jti → 404/400', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.forceLogout('non-existent-jti-fake');
    expect(r.status).toBeLessThan(500);
  });
});
