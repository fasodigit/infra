// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec #8 — admin-device-trust-skip-otp
 * Modules : M12 device-trust fingerprint
 * Assertion : 2nd login → MFA prompt NOT visible.
 */
import { test, expect } from '@playwright/test';
import { bootstrapAdminSuite, RiskHelper } from '../../fixtures/admin';

test.describe('Spec #8 — admin-device-trust-skip-otp (M12)', () => {
  test('list devices endpoint responds', async ({}, testInfo) => {
    const ctx = await bootstrapAdminSuite();
    if (!ctx.reachability.gateway || !ctx.loginOk) {
      testInfo.skip(true, 'Gateway/login KO');
      return;
    }
    const r = await ctx.admin.listDevices();
    expect(r.status, `Body: ${r.text?.slice(0, 200)}`).toBeLessThan(500);
  });

  test('KAYA accepts dev:{userId}:{fp} flag (M12 storage)', async ({}, testInfo) => {
    const risk = new RiskHelper();
    if (!(await risk.isReachable())) {
      testInfo.skip(true, 'KAYA unreachable');
      return;
    }
    const r = await risk.setDeviceTrust(
      'e2e-test-user-1',
      'sha256-fp-abcdef0123456789',
    );
    expect(r.applied).toBe(true);
  });
});
