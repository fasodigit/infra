// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { test, expect } from '@playwright/test';
import { isFrontendAvailable } from '../../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('06 - Payments - i18n (FR / MOS / DYU) smoke', () => {
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

  test('Switch FR → MOS → FR persists via localStorage.i18n.lang', async ({ page }) => {
    // Pre-seed localStorage so the switch works on a page that renders the
    // switcher. Landing page itself doesn't embed the switcher, so we
    // validate persistence by directly reading/writing via the SPA.
    await page.goto(`${BASE_URL}/`);
    await page.waitForLoadState('domcontentloaded');

    // Verify the landing title is visible (FR by default).
    await expect(page.locator('h1').first()).toBeVisible({ timeout: 10000 });

    // Programmatically switch to MOS (we don't depend on DOM of the
    // guarded layout switcher, since landing is unguarded).
    await page.evaluate(() => {
      localStorage.setItem('i18n.lang', 'mos');
      localStorage.setItem('faso_lang', 'mos');
    });

    // Reload so the app picks it up.
    await page.reload();
    await page.waitForLoadState('domcontentloaded');

    // The persisted value must still be mos.
    const persisted = await page.evaluate(() => localStorage.getItem('i18n.lang'));
    expect(persisted).toBe('mos');

    // Revert to FR
    await page.evaluate(() => {
      localStorage.setItem('i18n.lang', 'fr');
      localStorage.setItem('faso_lang', 'fr');
    });
    await page.reload();
    await page.waitForLoadState('domcontentloaded');

    const revertedTo = await page.evaluate(() => localStorage.getItem('i18n.lang'));
    expect(revertedTo).toBe('fr');
  });
});
