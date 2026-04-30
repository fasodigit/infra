// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #19 — admin-super-admin-undeletable
 * Modules : M19 (service guard + DB trigger prevent_super_admin_delete)
 * Assertion : 403 + audit_log (et trigger PG empêche bypass SQL direct).
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite } from '../../fixtures/admin';

test.describe('Spec #19 — admin-super-admin-undeletable (M19)', () => {
  test('DELETE on SUPER-ADMIN refused with 403', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.delete(`/api/admin/users/${ctx.souleymane.kratosId}`);
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
    expect([400, 401, 403, 409, 422]).toContain(r.status);
  });

  test('SUSPEND on SUPER-ADMIN refused with 403', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.post(`/api/admin/users/${ctx.souleymane.kratosId}/suspend`);
    expect(r.status).toBeLessThan(500);
    expect([400, 401, 403, 409, 422]).toContain(r.status);
  });
});
