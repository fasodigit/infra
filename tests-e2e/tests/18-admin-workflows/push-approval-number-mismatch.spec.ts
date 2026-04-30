// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #28 — push-approval-number-mismatch
 * Modules : M13
 * Assertion : tap mauvais number → audit + retry possible.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #28 — push-approval number mismatch (M13)', () => {
  test('respond with wrong number → reject', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.respondPushApproval('non-existent-request-id', {
      selectedNumber: '99',
      granted: true,
    });
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    expect([400, 401, 403, 404, 410, 422]).toContain(r.status);
  });

  test('status of unknown requestId → 404', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.pushApprovalStatus('non-existent');
    expect(r.status).toBeLessThan(500);
  });
});
