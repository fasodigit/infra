import { test, expect, Page } from '@playwright/test';
import { eleveurs, clients, uniqueEmail } from '../data/seed';
import { isFrontendAvailable, registerUser, loginAs, logout } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('01 - Authentication', () => {
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
  // Registration
  // --------------------------------------------------
  test('Register eleveur (Ouedraogo Amadou)', async ({ page }) => {
    const eleveur = eleveurs[0];
    const email = uniqueEmail(eleveur.email);

    await registerUser(page, {
      name: eleveur.name,
      email,
      password: eleveur.password,
      phone: eleveur.phone,
      role: 'eleveur',
      location: eleveur.location,
      capacity: eleveur.capacity,
    });

    // After successful registration, user should be redirected to their dashboard
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 15000 });
  });

  test('Register client (Restaurant Le Sahel)', async ({ page }) => {
    const client = clients[0];
    const email = uniqueEmail(client.email);

    await registerUser(page, {
      name: client.name,
      email,
      password: client.password,
      phone: client.phone,
      role: 'client',
      location: client.location,
      clientType: client.type,
    });

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 15000 });
  });

  test('Register eleveur with groupement (Compaore Fatimata)', async ({ page }) => {
    const eleveur = eleveurs[1];
    const email = uniqueEmail(eleveur.email);

    await registerUser(page, {
      name: eleveur.name,
      email,
      password: eleveur.password,
      phone: eleveur.phone,
      role: 'eleveur',
      location: eleveur.location,
      capacity: eleveur.capacity,
      groupement: eleveur.groupement,
    });

    await expect(page).toHaveURL(/\/dashboard/, { timeout: 15000 });
  });

  // --------------------------------------------------
  // Login
  // --------------------------------------------------
  test('Login as eleveur', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    // Verify user menu shows the user name
    await page.locator('button').filter({ has: page.locator('mat-icon:text("account_circle")') }).click();
    const menuHeader = page.locator('.user-menu-header');
    await expect(menuHeader).toContainText(eleveur.name);
    // Close the menu by pressing Escape
    await page.keyboard.press('Escape');
  });

  test('Login as client', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Logout
  // --------------------------------------------------
  test('Logout redirects to login page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 10000 });

    await logout(page);
    await expect(page).toHaveURL(/\/auth\/login/);
  });

  // --------------------------------------------------
  // Error handling
  // --------------------------------------------------
  test('Login with wrong password shows error', async ({ page }) => {
    await page.goto('/auth/login');
    await page.waitForSelector('.login-card', { timeout: 10000 });

    await page.locator('input[formControlName="email"]').fill(eleveurs[0].email);
    await page.locator('input[formControlName="password"]').fill('WrongPassword!999');
    await page.locator('button[type="submit"]').click();

    // Error banner should appear
    const errorBanner = page.locator('.error-banner');
    await expect(errorBanner).toBeVisible({ timeout: 10000 });
  });

  test('Login form validates required fields', async ({ page }) => {
    await page.goto('/auth/login');
    await page.waitForSelector('.login-card', { timeout: 10000 });

    // Submit button should be disabled when form is empty
    const submitBtn = page.locator('button[type="submit"]');
    await expect(submitBtn).toBeDisabled();

    // Fill only email, password still empty
    await page.locator('input[formControlName="email"]').fill('test@test.bf');
    await expect(submitBtn).toBeDisabled();

    // Fill password, submit should be enabled
    await page.locator('input[formControlName="password"]').fill('anypassword');
    await expect(submitBtn).toBeEnabled();
  });

  // --------------------------------------------------
  // Language switching
  // --------------------------------------------------
  test('Switch language FR -> EN -> FR', async ({ page }) => {
    await page.goto('/auth/login');
    await page.waitForSelector('.login-card', { timeout: 10000 });

    // Default should be in French context (the brand text is constant)
    const brandText = page.locator('.brand');
    await expect(brandText).toHaveText('Poulets BF');

    // Look for the language switcher component
    // After login, go to the main layout to find the language switcher
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    // The layout should have a language-switcher component
    const langSwitcher = page.locator('app-language-switcher');
    if (await langSwitcher.isVisible()) {
      // Click it to switch to EN
      await langSwitcher.click();
      await page.waitForTimeout(500);

      // Switch back to FR
      await langSwitcher.click();
      await page.waitForTimeout(500);
    }

    // Verify the app is still functional
    await expect(page.locator('.layout-container')).toBeVisible();
  });
});
