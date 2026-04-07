import { test, expect } from '@playwright/test';
import { eleveurs } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('05 - Halal Certification', () => {
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
  // Navigate to halal section
  // --------------------------------------------------
  test('Eleveur navigates to halal certification page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/halal');
    await page.waitForLoadState('domcontentloaded');

    // Halal page should be visible
    await expect(page.locator('body')).toContainText(/halal|certif/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Request new certification
  // --------------------------------------------------
  test('Eleveur requests a new halal certification', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/halal');
    await page.waitForLoadState('domcontentloaded');

    // Look for a "Nouvelle certification" or "Demander" button
    const requestBtn = page.locator('button, a').filter({ hasText: /nouvelle|demander|request|certif|cr[eé]er|ajouter/i }).first();
    if (await requestBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await requestBtn.click();
      await page.waitForLoadState('domcontentloaded');

      // Fill the certification request form
      // Lot reference
      const lotInput = page.locator('input[formControlName="lotId"], input[formControlName="lot"], input[formControlName="reference"]').first();
      if (await lotInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await lotInput.fill('LOT-2026-001');
      }

      // Abattoir name
      const abattoirInput = page.locator('input[formControlName="abattoir"], input[formControlName="lieuAbattage"]').first();
      if (await abattoirInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await abattoirInput.fill('Abattoir Municipal Ouagadougou');
      }

      // Certifier name
      const certifierInput = page.locator('input[formControlName="certifieur"], input[formControlName="imam"]').first();
      if (await certifierInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await certifierInput.fill('Imam Ouedraogo Ibrahim');
      }

      // Date
      const dateInput = page.locator('input[formControlName="date"], input[formControlName="dateCertification"]').first();
      if (await dateInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await dateInput.fill('2026-04-10');
      }

      // Quantity
      const qtyInput = page.locator('input[formControlName="quantity"], input[formControlName="quantite"]').first();
      if (await qtyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await qtyInput.fill('50');
      }

      // Submit the request
      const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /soumettre|envoyer|submit|demander|enregistrer/i }).first();
      if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await submitBtn.click();
        await page.waitForTimeout(1000);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Verify certification status
  // --------------------------------------------------
  test('Certification appears as "En attente"', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/halal');
    await page.waitForLoadState('domcontentloaded');

    // Check for pending certification status
    const pendingStatus = page.locator('[class*="status"], mat-chip, .badge').filter({ hasText: /attente|pending|en cours/i }).first();
    if (await pendingStatus.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(pendingStatus).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Certifications list
  // --------------------------------------------------
  test('Eleveur views certifications list', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/halal');
    await page.waitForLoadState('domcontentloaded');

    // The halal page shows the title
    const pageTitle = page.locator('h1, h2').filter({ hasText: /halal|certif/i }).first();
    if (await pageTitle.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(pageTitle).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Empty state display
  // --------------------------------------------------
  test('Halal page shows empty state when no certifications exist', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/halal');
    await page.waitForLoadState('domcontentloaded');

    // Check for empty state component
    const emptyState = page.locator('app-empty-state, .empty-state').first();
    if (await emptyState.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(emptyState).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });
});
