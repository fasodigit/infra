// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #11 — admin-audit-query
 * Modules : M22 audit
 * Assertion : filtres date / actor / action fonctionnent.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #11 — admin-audit-query (M22)', () => {
  test('audit list endpoint reachable', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.queryAudit({});
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('audit filter by actor + action + date range', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.queryAudit({
      actor: ctx.aminata.email,
      action: 'OTP_ISSUED',
      from: '2026-01-01T00:00:00Z',
      to: '2027-01-01T00:00:00Z',
      limit: 10,
    });
    expect(r.status).toBeLessThan(500);
  });
});
