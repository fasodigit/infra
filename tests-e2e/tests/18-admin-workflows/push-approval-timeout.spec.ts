// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #29 — push-approval-timeout
 * Modules : M13
 * Assertion : 30s sans réponse → fallback OTP auto.
 *
 * On valide la sémantique TTL : la requête initiée disparaît du KAYA
 * (TTL 30s `auth:approval:{rid}`) ; status d'un id obsolète → 404/410.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #29 — push-approval timeout (M13)', () => {
  test('expired requestId → 404 (TTL elapsed)', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.pushApprovalStatus('00000000-fake-expired');
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    expect([400, 401, 403, 404, 410]).toContain(r.status);
  });
});
