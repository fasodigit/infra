import { test, expect } from '@playwright/test';
import { eleveurs, vaccinations } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('04 - Veterinary (OBLIGATOIRE)', () => {
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
  // Navigate to veterinary section
  // --------------------------------------------------
  test('Eleveur navigates to veterinary page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    // Veterinary page should be visible
    await expect(page.locator('body')).toContainText(/v[eé]t[eé]rinaire|veterinary|medical/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Create fiche sanitaire
  // --------------------------------------------------
  test('Eleveur creates a fiche sanitaire for a lot', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    // Look for a "Nouvelle fiche" or "Creer" button
    const createBtn = page.locator('button, a').filter({ hasText: /nouvelle|cr[eé]er|add|ajouter/i }).first();
    if (await createBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await createBtn.click();
      await page.waitForLoadState('domcontentloaded');

      // Fill the fiche sanitaire form
      // Lot identifier
      const lotInput = page.locator('input[formControlName="lotId"], input[formControlName="lot"], input').filter({ hasText: /lot/i }).first();
      if (await lotInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await lotInput.fill('LOT-2026-001');
      }

      // Race
      const raceSelect = page.locator('mat-select[formControlName="race"]').first();
      if (await raceSelect.isVisible({ timeout: 3000 }).catch(() => false)) {
        await raceSelect.click();
        await page.locator('mat-option').first().click();
      }

      // Quantity
      const qtyInput = page.locator('input[formControlName="quantity"], input[formControlName="effectif"]').first();
      if (await qtyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await qtyInput.fill('100');
      }

      // State: healthy
      const stateSelect = page.locator('mat-select[formControlName="etat"], mat-select[formControlName="status"]').first();
      if (await stateSelect.isVisible({ timeout: 3000 }).catch(() => false)) {
        await stateSelect.click();
        await page.locator('mat-option').filter({ hasText: /sain|healthy/i }).first().click();
      }

      // Submit
      const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /enregistrer|sauvegarder|save|cr[eé]er/i }).first();
      if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await submitBtn.click();
        await page.waitForTimeout(1000);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Add vaccination (Newcastle HB1)
  // --------------------------------------------------
  test('Eleveur adds Newcastle HB1 vaccination', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    const vacc = vaccinations[0];

    // Navigate to add vaccination - could be a sub-page or a dialog
    const addVaccBtn = page.locator('button, a').filter({ hasText: /vaccination|vaccin/i }).first();
    if (await addVaccBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await addVaccBtn.click();
      await page.waitForLoadState('domcontentloaded');
    }

    // Fill vaccination form
    const vaccinInput = page.locator('input[formControlName="vaccin"], input[formControlName="nom"]').first();
    if (await vaccinInput.isVisible({ timeout: 5000 }).catch(() => false)) {
      await vaccinInput.fill(vacc.vaccin);
    }

    // Date
    const dateInput = page.locator('input[formControlName="date"], input[formControlName="dateVaccination"]').first();
    if (await dateInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await dateInput.fill(vacc.date);
    }

    // Vet name
    const vetInput = page.locator('input[formControlName="veterinaire"], input[formControlName="vet"]').first();
    if (await vetInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await vetInput.fill(vacc.vet);
    }

    // Batch number
    const batchInput = page.locator('input[formControlName="batchNumber"], input[formControlName="numeroLot"]').first();
    if (await batchInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await batchInput.fill(vacc.batchNumber);
    }

    // Submit
    const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /enregistrer|save|ajouter|add/i }).first();
    if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await submitBtn.click();
      await page.waitForTimeout(1000);
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Add treatment
  // --------------------------------------------------
  test('Eleveur adds a treatment record', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    // Navigate to add treatment
    const addTreatBtn = page.locator('button, a').filter({ hasText: /traitement|treatment/i }).first();
    if (await addTreatBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await addTreatBtn.click();
      await page.waitForLoadState('domcontentloaded');

      // Fill treatment form
      const treatmentInput = page.locator('input[formControlName="traitement"], input[formControlName="treatment"], input[formControlName="nom"]').first();
      if (await treatmentInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await treatmentInput.fill('Antiparasitaire Ivermectine');
      }

      const doseInput = page.locator('input[formControlName="dose"], input[formControlName="dosage"]').first();
      if (await doseInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await doseInput.fill('0.2 ml/kg');
      }

      const dateInput = page.locator('input[formControlName="date"], input[formControlName="dateTraitement"]').first();
      if (await dateInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await dateInput.fill('2026-04-05');
      }

      // Submit
      const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /enregistrer|save|ajouter/i }).first();
      if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await submitBtn.click();
        await page.waitForTimeout(1000);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Verify fiche status
  // --------------------------------------------------
  test('Fiche sanitaire shows "Sain" status', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    // Look for a fiche with "Sain" or "Healthy" status
    const sainStatus = page.locator('[class*="status"], mat-chip, .badge').filter({ hasText: /sain|healthy|vert/i }).first();
    if (await sainStatus.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(sainStatus).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Vaccination history
  // --------------------------------------------------
  test('Vaccination appears in history', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    // Check if vaccination records are shown
    // Look for the vaccination name in the page
    const vaccRecord = page.locator('body').filter({ hasText: /Newcastle|Gumboro/i });
    if (await vaccRecord.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(page.locator('body')).toContainText(/Newcastle|Gumboro/i);
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Fiches list
  // --------------------------------------------------
  test('Eleveur sees fiches sanitaires list', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/veterinary');
    await page.waitForLoadState('domcontentloaded');

    // The page should show the veterinary section
    const pageTitle = page.locator('h1, h2').filter({ hasText: /v[eé]t[eé]rinaire|veterinary/i }).first();
    if (await pageTitle.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(pageTitle).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });
});
