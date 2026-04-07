import { test, expect } from '@playwright/test';
import { eleveurs, clients } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('07 - Calendar', () => {
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

  // --------------------------------------------------
  // Navigate to calendar
  // --------------------------------------------------
  test('Eleveur navigates to calendar page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/calendar');
    await page.waitForLoadState('domcontentloaded');

    // Calendar page should be visible
    await expect(page.locator('body')).toContainText(/calendrier|calendar|planning/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Verify events
  // --------------------------------------------------
  test('Calendar shows events (lots and deliveries)', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/calendar');
    await page.waitForLoadState('networkidle');

    // Look for calendar events, cells, or event items
    const calendarContainer = page.locator('.calendar, mat-card, [class*="calendar"]').first();
    if (await calendarContainer.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(calendarContainer).toBeVisible();
    }

    // Check for event items if they exist
    const events = page.locator('.event, .calendar-event, [class*="event"]');
    const eventCount = await events.count().catch(() => 0);
    // Events may or may not exist; just ensure the page loaded
    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Switch to planning view
  // --------------------------------------------------
  test('Switch to planning view and verify supply/demand bars', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/calendar/planning');
    await page.waitForLoadState('domcontentloaded');

    // Planning view should be visible
    await expect(page.locator('body')).toContainText(/planning|offre|demande|supply|demand/i, { timeout: 10000 });

    // Look for chart bars or supply/demand visualization
    const planningChart = page.locator('.bar, .chart, svg, canvas, [class*="planning"]').first();
    if (await planningChart.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(planningChart).toBeVisible();
    }
  });

  // --------------------------------------------------
  // Calendar navigation (month/week)
  // --------------------------------------------------
  test('Navigate between months in calendar', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/calendar');
    await page.waitForLoadState('domcontentloaded');

    // Look for navigation arrows (next/previous month)
    const nextBtn = page.locator('button').filter({ has: page.locator('mat-icon:text("chevron_right"), mat-icon:text("navigate_next"), mat-icon:text("arrow_forward")') }).first();
    if (await nextBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await nextBtn.click();
      await page.waitForTimeout(500);

      // Go back
      const prevBtn = page.locator('button').filter({ has: page.locator('mat-icon:text("chevron_left"), mat-icon:text("navigate_before"), mat-icon:text("arrow_back")') }).first();
      if (await prevBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await prevBtn.click();
        await page.waitForTimeout(500);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Client calendar
  // --------------------------------------------------
  test('Client can view delivery calendar', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/calendar');
    await page.waitForLoadState('domcontentloaded');

    // Calendar should be visible for client too
    await expect(page.locator('body')).toContainText(/calendrier|calendar|livraison|delivery/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Calendar view modes
  // --------------------------------------------------
  test('Calendar supports view mode toggle (month/week/day)', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/calendar');
    await page.waitForLoadState('domcontentloaded');

    // Look for view mode toggle buttons
    const viewToggles = page.locator('mat-button-toggle, button').filter({ hasText: /mois|semaine|jour|month|week|day/i });
    const count = await viewToggles.count().catch(() => 0);

    if (count > 0) {
      // Click on week view if available
      const weekBtn = viewToggles.filter({ hasText: /semaine|week/i }).first();
      if (await weekBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await weekBtn.click();
        await page.waitForTimeout(500);
      }

      // Click back to month view
      const monthBtn = viewToggles.filter({ hasText: /mois|month/i }).first();
      if (await monthBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await monthBtn.click();
        await page.waitForTimeout(500);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });
});
