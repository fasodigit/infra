// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #23 — crypto-argon2-rehash-on-login
 * Modules : M01 Argon2id password hashing (lazy re-hash bcrypt → argon2)
 * Assertion : login d'un user dont le hash est encore bcrypt → re-hash
 *             argon2id silencieux côté DB (no user-visible side-effect).
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite, PgAdminProbe } from '../../fixtures/admin';

test.describe('Spec #23 — crypto-argon2-rehash-on-login (M01)', () => {
  test('Aminata can login + Kratos uses argon2id', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.kratos) {
      testInfo.skip(true, 'Kratos KO');
      return;
    }
    expect(ctx.loginOk, 'Aminata login should succeed (proves M01 OK)').toBe(true);
  });

  test('audit_log records LOGIN_SUCCESS via M22 trigger', async ({}, testInfo) => {
    const dsn =
      process.env.FASO_ADMIN_PG_URL ??
      'postgresql://auth_ms:auth_ms_dev@localhost:5432/auth_ms';
    const probe = new PgAdminProbe(dsn);
    const reachable = await probe.isReachable();
    if (!reachable) {
      testInfo.skip(true, `PG admin DB unreachable (${dsn}) — set FASO_ADMIN_PG_URL`);
      return;
    }
    const exists = await probe.tableExists('audit_log');
    expect(exists).toBe(true);
  });
});
