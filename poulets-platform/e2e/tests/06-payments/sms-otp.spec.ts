// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { test, expect, request } from '@playwright/test';

// Default BFF URL — can be overridden via env (read through globalThis for
// tsconfig-agnostic access, since the e2e tsconfig omits @types/node).
const BFF_URL =
  (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env?.['BFF_URL'] ??
  'http://localhost:4800';

test.describe('06 - Payments - SMS OTP smoke', () => {
  let bffUp = false;

  test.beforeAll(async () => {
    const ctx = await request.newContext();
    try {
      const health = await ctx.get(`${BFF_URL}/api/health`, { timeout: 3000 });
      bffUp = health.ok();
    } catch {
      bffUp = false;
    }
  });

  test('POST /api/auth/sms-otp returns {sent:true}', async ({}, testInfo) => {
    if (!bffUp) {
      testInfo.skip();
      return;
    }

    const ctx = await request.newContext();
    const res = await ctx.post(`${BFF_URL}/api/auth/sms-otp`, {
      data: { phone: '+22670123456' },
    });

    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body).toHaveProperty('sent', true);
  });
});
