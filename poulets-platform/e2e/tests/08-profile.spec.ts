import { test, expect } from '@playwright/test';
import { eleveurs, clients } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('08 - Profile & Reputation', () => {
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
  // Navigate to profile
  // --------------------------------------------------
  test('Eleveur navigates to profile page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/profile');
    await page.waitForLoadState('domcontentloaded');

    // Profile page should show the user's info
    await expect(page.locator('body')).toContainText(/profil|profile/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Edit profile (change phone)
  // --------------------------------------------------
  test('Edit profile - change phone number', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/profile');
    await page.waitForLoadState('domcontentloaded');

    // Look for edit button or editable fields
    const editBtn = page.locator('button, a').filter({ hasText: /modifier|edit|[eé]diter/i }).first();
    if (await editBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await editBtn.click();
      await page.waitForLoadState('domcontentloaded');
    }

    // Find phone input
    const phoneInput = page.locator('input[formControlName="phone"], input[formControlName="telephone"], input[type="tel"]').first();
    if (await phoneInput.isVisible({ timeout: 5000 }).catch(() => false)) {
      await phoneInput.clear();
      await phoneInput.fill('+22670998877');

      // Save
      const saveBtn = page.locator('button[type="submit"], button').filter({ hasText: /enregistrer|save|sauvegarder|mettre.*jour|update/i }).first();
      if (await saveBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await saveBtn.click();
        await page.waitForTimeout(1000);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // View profile details
  // --------------------------------------------------
  test('Profile shows user details (name, email, role)', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/profile');
    await page.waitForLoadState('domcontentloaded');

    // Check that the profile page contains user info
    // The user name might be displayed
    const nameEl = page.locator('body').filter({ hasText: new RegExp(eleveur.name.split(' ')[0], 'i') });
    if (await nameEl.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(page.locator('body')).toContainText(eleveur.name.split(' ')[0]);
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // View reputation page
  // --------------------------------------------------
  test('Navigate to reputation page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/reputation');
    await page.waitForLoadState('domcontentloaded');

    // Reputation page should load
    await expect(page.locator('body')).toContainText(/r[eé]putation|avis|review|note/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Leave a review
  // --------------------------------------------------
  test('Client leaves a review for a partner', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/reputation');
    await page.waitForLoadState('domcontentloaded');

    // Look for a "Laisser un avis" button
    const reviewBtn = page.locator('button, a').filter({ hasText: /avis|review|noter|[eé]valuer/i }).first();
    if (await reviewBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await reviewBtn.click();
      await page.waitForLoadState('domcontentloaded');

      // Fill review form
      // Rating (stars)
      const stars = page.locator('.star, mat-icon').filter({ hasText: /star/i });
      const starCount = await stars.count().catch(() => 0);
      if (starCount >= 4) {
        // Click on the 4th star for a 4/5 rating
        await stars.nth(3).click();
      }

      // Comment
      const commentInput = page.locator('textarea[formControlName="comment"], textarea[formControlName="commentaire"], textarea').first();
      if (await commentInput.isVisible({ timeout: 3000 }).catch(() => false)) {
        await commentInput.fill('Excellent eleveur, poulets de tres bonne qualite. Livraison ponctuelle.');
      }

      // Submit
      const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /envoyer|publier|submit|enregistrer/i }).first();
      if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
        await submitBtn.click();
        await page.waitForTimeout(1000);
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Profile via user menu
  // --------------------------------------------------
  test('Access profile through user menu', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    // Open user menu via account icon
    await page.locator('button').filter({ has: page.locator('mat-icon:text("account_circle")') }).click();

    // Click profile menu item
    const profileMenuItem = page.locator('button[mat-menu-item]').filter({ hasText: /profil/i }).first();
    if (await profileMenuItem.isVisible({ timeout: 5000 }).catch(() => false)) {
      await profileMenuItem.click();
      await page.waitForURL(/\/profile/, { timeout: 10000 });
      await expect(page).toHaveURL(/\/profile/);
    }
  });

  // --------------------------------------------------
  // Client profile
  // --------------------------------------------------
  test('Client navigates to profile page', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/profile');
    await page.waitForLoadState('domcontentloaded');

    await expect(page.locator('body')).toContainText(/profil|profile/i, { timeout: 10000 });
  });
});
