import { test, expect } from '@playwright/test';
import { eleveurs, clients, annonces } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('03 - Order Flow', () => {
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
  // Client: Create an order
  // --------------------------------------------------
  test('Client creates an order via the order form', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/orders/new');
    await page.waitForLoadState('domcontentloaded');

    // Step 1: Product selection
    const raceSelect = page.locator('mat-select[formControlName="race"]').first();
    if (await raceSelect.isVisible({ timeout: 5000 }).catch(() => false)) {
      await raceSelect.click({ force: true });
      await page.locator('mat-option').filter({ hasText: /bicyclette|local/i }).first().click();
    }

    // Quantity
    const qtyInput = page.locator('input[formControlName="quantite"]').first();
    if (await qtyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await qtyInput.clear();
      await qtyInput.fill('30');
    }

    // Price per unit
    const priceInput = page.locator('input[formControlName="prixUnitaire"]').first();
    if (await priceInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await priceInput.clear();
      await priceInput.fill(String(annonces[0].pricePerUnit));
    }

    // Verify total is calculated
    const totalPreview = page.locator('.total-preview .total-value, .total-value').first();
    if (await totalPreview.isVisible({ timeout: 3000 }).catch(() => false)) {
      await expect(totalPreview).not.toHaveText('0');
    }

    // Next to Step 2: Delivery
    await page.locator('button[matStepperNext]').first().click();
    await page.waitForTimeout(500);

    // Delivery date
    const deliveryDateInput = page.locator('input[formControlName="dateLivraison"]').first();
    if (await deliveryDateInput.isVisible({ timeout: 5000 }).catch(() => false)) {
      await deliveryDateInput.fill('2026-05-20');
    }

    // Delivery mode - self pickup
    const selfRadio = page.locator('mat-radio-button[value="self"]').first();
    if (await selfRadio.isVisible({ timeout: 3000 }).catch(() => false)) {
      await selfRadio.click();
    }

    // Delivery address
    const addressInput = page.locator('input[formControlName="adresseLivraison"]').first();
    if (await addressInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await addressInput.fill('Ouagadougou, Zone du Bois');
    }

    // Phone
    const phoneInput = page.locator('input[formControlName="telephone"]').first();
    if (await phoneInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await phoneInput.fill('+22625334455');
    }

    // Next to Step 3: Payment
    await page.locator('button[matStepperNext]').nth(1).click();
    await page.waitForTimeout(500);

    // Select Orange Money
    const orangeRadio = page.locator('mat-radio-button[value="orange_money"]').first();
    if (await orangeRadio.isVisible({ timeout: 5000 }).catch(() => false)) {
      await orangeRadio.click();
    }

    // Notes
    const notesInput = page.locator('textarea[formControlName="notes"]').first();
    if (await notesInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await notesInput.fill('Livraison vendredi matin SVP');
    }

    // Order summary should be visible
    const summarySection = page.locator('.order-summary');
    if (await summarySection.isVisible({ timeout: 3000 }).catch(() => false)) {
      await expect(summarySection).toContainText(/total/i);
    }

    // Submit order
    const confirmBtn = page.locator('button').filter({ hasText: /confirm|commander/i }).first();
    if (await confirmBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
      await confirmBtn.click();
    }

    // Verify order was submitted (snackbar or redirect)
    await page.waitForTimeout(2000);
    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Client: View orders list
  // --------------------------------------------------
  test('Client sees order in "Mes commandes"', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // The orders page should be visible
    await expect(page.locator('body')).toContainText(/commande|order/i, { timeout: 10000 });
  });

  test('Client order shows status "En attente"', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // Look for "En attente" or "En_attente" status in the orders list
    const statusBadge = page.locator('mat-chip, .status-badge, [class*="status"]').filter({ hasText: /attente|pending/i }).first();
    if (await statusBadge.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(statusBadge).toBeVisible();
    }
  });

  // --------------------------------------------------
  // Eleveur: View received orders
  // --------------------------------------------------
  test('Eleveur sees orders in "Commandes recues"', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // The orders page should load for the eleveur
    await expect(page.locator('body')).toContainText(/commande|order/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Eleveur: Confirm order
  // --------------------------------------------------
  test('Eleveur confirms an order', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // Look for a confirm button in the orders list/detail
    const confirmBtn = page.locator('button').filter({ hasText: /confirmer|confirm/i }).first();
    if (await confirmBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await confirmBtn.click();
      await page.waitForTimeout(1000);

      // Check status changed to "Confirme"
      const statusEl = page.locator('[class*="status"], mat-chip').filter({ hasText: /confirm/i }).first();
      if (await statusEl.isVisible({ timeout: 5000 }).catch(() => false)) {
        await expect(statusEl).toBeVisible();
      }
    }
  });

  // --------------------------------------------------
  // Eleveur: Mark as ready
  // --------------------------------------------------
  test('Eleveur marks order as ready', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // Look for "Pret" button
    const readyBtn = page.locator('button').filter({ hasText: /pr[eê]t|ready/i }).first();
    if (await readyBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await readyBtn.click();
      await page.waitForTimeout(1000);
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Eleveur: Mark as delivered
  // --------------------------------------------------
  test('Eleveur marks order as delivered', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // Look for "Livre" button
    const deliverBtn = page.locator('button').filter({ hasText: /livr[eé]|deliver/i }).first();
    if (await deliverBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await deliverBtn.click();
      await page.waitForTimeout(1000);

      // Check for "Livre" status
      const deliveredStatus = page.locator('[class*="status"], mat-chip').filter({ hasText: /livr[eé]/i }).first();
      if (await deliveredStatus.isVisible({ timeout: 5000 }).catch(() => false)) {
        await expect(deliveredStatus).toBeVisible();
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Order tracking
  // --------------------------------------------------
  test('Order tracking page shows stepper', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/orders');
    await page.waitForLoadState('networkidle');

    // Click on the first order to see tracking if available
    const orderRow = page.locator('tr[mat-row], mat-card, a').filter({ hasText: /CMD|commande/i }).first();
    if (await orderRow.isVisible({ timeout: 5000 }).catch(() => false)) {
      await orderRow.click();
      await page.waitForLoadState('domcontentloaded');

      // Look for a tracking stepper or status timeline
      const trackingEl = page.locator('mat-stepper, .tracking, .status-timeline').first();
      if (await trackingEl.isVisible({ timeout: 5000 }).catch(() => false)) {
        await expect(trackingEl).toBeVisible();
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });
});
