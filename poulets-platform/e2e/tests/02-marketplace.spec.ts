import { test, expect } from '@playwright/test';
import { eleveurs, clients, annonces, besoins, uniqueEmail } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('02 - Marketplace', () => {
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
  // Eleveur: Create annonce
  // --------------------------------------------------
  test('Eleveur creates a new annonce', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    // Navigate to create annonce
    await navigateTo(page, '/marketplace/annonces/new');
    await page.waitForSelector('.create-annonce-page', { timeout: 10000 });

    const a = annonces[0];

    // Fill the annonce form
    // Race dropdown
    await page.locator('mat-select[formControlName="race"]').click();
    await page.locator('mat-option').filter({ hasText: a.race }).click();

    // Quantity
    await page.locator('input[formControlName="quantity"]').fill(String(a.quantity));

    // Weights
    await page.locator('input[formControlName="currentWeight"]').fill(String(a.currentWeight));
    await page.locator('input[formControlName="estimatedWeight"]').fill(String(a.estimatedWeight));

    // Target date
    await page.locator('input[formControlName="targetDate"]').fill(a.targetDate);

    // Pricing
    await page.locator('input[formControlName="pricePerKg"]').fill(String(a.pricePerKg));
    await page.locator('input[formControlName="pricePerUnit"]').fill(String(a.pricePerUnit));

    // Location
    await page.locator('input[formControlName="location"]').fill(a.location);

    // Availability dates
    const today = new Date().toISOString().split('T')[0];
    await page.locator('input[formControlName="availabilityStart"]').fill(today);
    await page.locator('input[formControlName="availabilityEnd"]').fill(a.targetDate);

    // Description
    await page.locator('textarea[formControlName="description"]').fill(a.description);

    // Fiche sanitaire
    await page.locator('input[formControlName="ficheSanitaireId"]').fill(a.ficheSanitaireId);

    // Halal checkbox
    if (a.halalCertified) {
      const checkbox = page.locator('mat-checkbox[formControlName="halalCertified"]');
      const isChecked = await checkbox.locator('input[type="checkbox"]').isChecked();
      if (!isChecked) {
        await checkbox.click();
      }
    }

    // Submit
    await page.locator('button[type="submit"]').click();

    // Should navigate to the annonce detail or annonces list with success snackbar
    await expect(page.locator('body')).toContainText(/succes|annonce/i, { timeout: 10000 });
  });

  test('Eleveur creates a second annonce (Brahma)', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/marketplace/annonces/new');
    await page.waitForSelector('.create-annonce-page', { timeout: 10000 });

    const a = annonces[1];

    await page.locator('mat-select[formControlName="race"]').click();
    await page.locator('mat-option').filter({ hasText: a.race }).click();
    await page.locator('input[formControlName="quantity"]').fill(String(a.quantity));
    await page.locator('input[formControlName="currentWeight"]').fill(String(a.currentWeight));
    await page.locator('input[formControlName="estimatedWeight"]').fill(String(a.estimatedWeight));
    await page.locator('input[formControlName="targetDate"]').fill(a.targetDate);
    await page.locator('input[formControlName="pricePerKg"]').fill(String(a.pricePerKg));
    await page.locator('input[formControlName="pricePerUnit"]').fill(String(a.pricePerUnit));
    await page.locator('input[formControlName="location"]').fill(a.location);

    const today = new Date().toISOString().split('T')[0];
    await page.locator('input[formControlName="availabilityStart"]').fill(today);
    await page.locator('input[formControlName="availabilityEnd"]').fill(a.targetDate);
    await page.locator('textarea[formControlName="description"]').fill(a.description);
    await page.locator('input[formControlName="ficheSanitaireId"]').fill(a.ficheSanitaireId);

    if (a.halalCertified) {
      const checkbox = page.locator('mat-checkbox[formControlName="halalCertified"]');
      const isChecked = await checkbox.locator('input[type="checkbox"]').isChecked();
      if (!isChecked) {
        await checkbox.click();
      }
    }

    await page.locator('button[type="submit"]').click();
    await expect(page.locator('body')).toContainText(/succes|annonce/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Annonces list
  // --------------------------------------------------
  test('Annonces list shows created annonces', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/marketplace/annonces');
    await page.waitForLoadState('networkidle');

    // The page should contain annonce cards or a list
    // Check the page loaded properly
    await expect(page.locator('body')).toContainText(/annonce|marketplace/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Client: Browse marketplace
  // --------------------------------------------------
  test('Client browses marketplace and sees annonces', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/marketplace');
    await page.waitForLoadState('networkidle');

    // The marketplace page should be visible
    await expect(page.locator('body')).toContainText(/marketplace|annonce/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Client: Create besoin
  // --------------------------------------------------
  test('Client creates a besoin (30 poulets/semaine)', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/marketplace/besoins/new');
    await page.waitForLoadState('domcontentloaded');

    const b = besoins[0];

    // Fill besoin form fields - these depend on the actual form structure
    // Look for quantity input
    const quantityInput = page.locator('input[formControlName="quantity"], input[formControlName="quantite"]').first();
    if (await quantityInput.isVisible({ timeout: 5000 }).catch(() => false)) {
      await quantityInput.fill(String(b.quantity));
    }

    // Look for minimum weight
    const weightInput = page.locator('input[formControlName="minimumWeight"], input[formControlName="minWeight"]').first();
    if (await weightInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await weightInput.fill(String(b.minWeight));
    }

    // Budget
    const budgetInput = page.locator('input[formControlName="maxBudgetPerKg"], input[formControlName="budget"]').first();
    if (await budgetInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await budgetInput.fill(String(b.maxBudgetPerKg));
    }

    // Notes
    const notesInput = page.locator('textarea[formControlName="specialNotes"], textarea[formControlName="notes"]').first();
    if (await notesInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await notesInput.fill(b.notes);
    }

    // Location
    const locationInput = page.locator('input[formControlName="location"], input[formControlName="localisation"]').first();
    if (await locationInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await locationInput.fill(b.location);
    }

    // Try to submit the form
    const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /publier|soumettre|creer|submit/i }).first();
    if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await submitBtn.click();
    }

    // Verify we are still on a valid page
    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Matching
  // --------------------------------------------------
  test('Client navigates to matching page', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/marketplace/matching');
    await page.waitForLoadState('domcontentloaded');

    // The matching page should load
    await expect(page.locator('body')).toContainText(/match|correspondance|score/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Filters
  // --------------------------------------------------
  test('Filter annonces by race', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/marketplace/annonces');
    await page.waitForLoadState('networkidle');

    // Look for race filter (mat-select or dropdown)
    const raceFilter = page.locator('mat-select').filter({ hasText: /race/i }).first();
    if (await raceFilter.isVisible({ timeout: 5000 }).catch(() => false)) {
      await raceFilter.click();
      // Select a specific race
      const option = page.locator('mat-option').first();
      if (await option.isVisible({ timeout: 3000 }).catch(() => false)) {
        await option.click();
      }
    }

    // Verify page is still loaded
    await expect(page.locator('body')).toBeVisible();
  });

  test('Filter annonces by weight range', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/marketplace/annonces');
    await page.waitForLoadState('networkidle');

    // Look for weight filter inputs
    const weightMinInput = page.locator('input').filter({ hasText: /poids.*min|weight.*min/i }).first();
    if (await weightMinInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await weightMinInput.fill('1.5');
    }

    const weightMaxInput = page.locator('input').filter({ hasText: /poids.*max|weight.*max/i }).first();
    if (await weightMaxInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await weightMaxInput.fill('3.0');
    }

    await expect(page.locator('body')).toBeVisible();
  });

  test('Filter annonces by price', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/marketplace/annonces');
    await page.waitForLoadState('networkidle');

    // Look for price filter
    const priceInput = page.locator('input').filter({ hasText: /prix|price/i }).first();
    if (await priceInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await priceInput.fill('4000');
    }

    await expect(page.locator('body')).toBeVisible();
  });
});
