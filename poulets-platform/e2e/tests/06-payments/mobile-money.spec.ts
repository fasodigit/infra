// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { test, expect } from '@playwright/test';
import { isFrontendAvailable } from '../../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('06 - Payments - Mobile Money smoke', () => {
  let available: boolean;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    available = await isFrontendAvailable(page, BASE_URL);
    await page.close();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!available) {
      testInfo.skip();
    }
  });

  test('Initier paiement Orange Money → PENDING', async ({ page }) => {
    // The `/checkout/pay/:txId` route is registered as a PUBLIC top-level
    // route in app.routes.ts (before the /checkout guarded group) so it is
    // reachable without authentication for SMS deep-links.
    await page.goto(`${BASE_URL}/checkout/pay/test-1`);
    await page.waitForLoadState('domcontentloaded');

    // Form renders
    await expect(page.locator('app-mobile-money-form')).toBeVisible({ timeout: 10000 });

    // Select Orange Money via the mat-select
    await page.locator('mat-select[formcontrolname="provider"]').click();
    await page.locator('mat-option').filter({ hasText: /Orange Money/i }).click();

    // Phone (+226 auto-prefixed in UI; user enters 8 digits)
    await page.locator('input[formcontrolname="phone"]').fill('70123456');

    // Amount
    await page.locator('input[formcontrolname="amount"]').fill('5000');

    // Submit
    await page.getByRole('button', { name: /Initier paiement/i }).click();

    // Expect PENDING / initié / en cours
    await expect(page.locator('[data-testid="momo-result"]'))
      .toContainText(/PENDING|initié|en cours/i, { timeout: 10000 });
  });
});
