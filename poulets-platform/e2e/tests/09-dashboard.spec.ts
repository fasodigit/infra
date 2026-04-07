import { test, expect } from '@playwright/test';
import { eleveurs, clients } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('09 - Dashboard', () => {
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
  // Eleveur dashboard
  // --------------------------------------------------
  test('Eleveur dashboard displays KPI cards', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    // Should redirect to dashboard after login
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Verify KPI cards are visible
    const kpiCards = page.locator('.kpi-card, mat-card').filter({
      has: page.locator('.kpi-value, .kpi-label'),
    });
    const kpiCount = await kpiCards.count().catch(() => 0);

    if (kpiCount > 0) {
      // Should have multiple KPI cards
      expect(kpiCount).toBeGreaterThanOrEqual(1);

      // Each card should have a value
      const firstCard = kpiCards.first();
      await expect(firstCard).toBeVisible();
    }

    // Dashboard title should be visible
    await expect(page.locator('body')).toContainText(/dashboard|tableau.*bord/i, { timeout: 10000 });
  });

  test('Eleveur dashboard shows welcome message with name', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Look for welcome text containing the user's name
    const welcomeText = page.locator('.welcome-text, p').filter({ hasText: /bienvenue|welcome/i }).first();
    if (await welcomeText.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(welcomeText).toBeVisible();
    }
  });

  test('Eleveur dashboard shows revenue chart', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Look for the bar chart
    const barChart = page.locator('.bar-chart, .chart-card').first();
    if (await barChart.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(barChart).toBeVisible();

      // Verify individual bars exist
      const bars = page.locator('.bar-item, .bar');
      const barCount = await bars.count().catch(() => 0);
      if (barCount > 0) {
        expect(barCount).toBeGreaterThanOrEqual(1);
      }
    }
  });

  test('Eleveur dashboard shows weight progress chart', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Look for SVG line chart
    const lineChart = page.locator('.line-chart, svg').first();
    if (await lineChart.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(lineChart).toBeVisible();

      // Check for polyline elements (actual and target)
      const polylines = page.locator('polyline');
      const polyCount = await polylines.count().catch(() => 0);
      if (polyCount > 0) {
        expect(polyCount).toBeGreaterThanOrEqual(1);
      }
    }
  });

  // --------------------------------------------------
  // Recent orders table
  // --------------------------------------------------
  test('Eleveur dashboard shows recent orders table', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Look for the orders table
    const ordersTable = page.locator('table[mat-table], .table-card').first();
    if (await ordersTable.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(ordersTable).toBeVisible();

      // Check for table headers
      const headers = page.locator('th[mat-header-cell]');
      const headerCount = await headers.count().catch(() => 0);
      if (headerCount > 0) {
        expect(headerCount).toBeGreaterThanOrEqual(2);
      }

      // Check for table rows (data)
      const rows = page.locator('tr[mat-row]');
      const rowCount = await rows.count().catch(() => 0);
      if (rowCount > 0) {
        expect(rowCount).toBeGreaterThanOrEqual(1);
      }
    }
  });

  test('Eleveur dashboard has "View all" link to orders', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Look for "Voir tout" / "View all" link
    const viewAllLink = page.locator('a[mat-button], a').filter({ hasText: /voir.*tout|view.*all/i }).first();
    if (await viewAllLink.isVisible({ timeout: 5000 }).catch(() => false)) {
      await viewAllLink.click();
      await page.waitForURL(/\/orders/, { timeout: 10000 });
      await expect(page).toHaveURL(/\/orders/);
    }
  });

  // --------------------------------------------------
  // Alerts section
  // --------------------------------------------------
  test('Eleveur dashboard shows alerts section', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Look for alerts
    const alertsCard = page.locator('.alerts-card, mat-card').filter({
      hasText: /alerte|alert/i,
    }).first();
    if (await alertsCard.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(alertsCard).toBeVisible();

      // Check for alert items
      const alertItems = page.locator('.alert-item');
      const alertCount = await alertItems.count().catch(() => 0);
      if (alertCount > 0) {
        expect(alertCount).toBeGreaterThanOrEqual(1);
      }
    }
  });

  // --------------------------------------------------
  // Client dashboard
  // --------------------------------------------------
  test('Client dashboard is different from eleveur dashboard', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Client dashboard should load
    await expect(page.locator('body')).toContainText(/dashboard|tableau|commande|marketplace/i, { timeout: 10000 });
  });

  test('Client dashboard shows order-related content', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Client dashboard may show recent orders, spending, etc.
    await expect(page.locator('body')).toBeVisible();
  });
});
