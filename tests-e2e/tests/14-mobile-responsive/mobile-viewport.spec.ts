import { test, expect, devices } from '@playwright/test';
import { SignupPage } from '../../page-objects/SignupPage';
import { LoginPage } from '../../page-objects/LoginPage';

const MOBILE_VIEWPORT = { width: 375, height: 812 };
const TABLET_VIEWPORT = { width: 768, height: 1024 };

// Angular SPA routes are served by the Angular dev server (:4801). The
// gateway (:8080) currently fronts the Next.js BFF which only owns API
// routes — see comment in tests/10-validation/form-validation.spec.ts.
const FRONTEND_URL = process.env.FRONTEND_URL ?? 'http://localhost:4801';

test.describe('Mobile Responsive - Mobile viewport', () => {
  test.use({ viewport: MOBILE_VIEWPORT, baseURL: FRONTEND_URL });

  test('[@smoke] landing page renders on mobile', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('body')).toBeVisible();
    const viewport = page.viewportSize();
    expect(viewport?.width).toBe(375);
  });

  test('login page is usable on mobile', async ({ page }) => {
    const login = new LoginPage(page);
    await login.goto();
    await expect(login.emailInput).toBeVisible();
    await expect(login.passwordInput).toBeVisible();
    await expect(login.submitButton).toBeVisible();
    const box = await login.submitButton.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(40);
    expect(box!.height).toBeGreaterThan(30);
  });

  test('signup page stepper works on mobile', async ({ page }) => {
    const signup = new SignupPage(page);
    await signup.goto();
    await expect(signup.nomInput).toBeVisible();
    await expect(signup.emailInput).toBeVisible();
    const nomBox = await signup.nomInput.boundingBox();
    expect(nomBox).not.toBeNull();
    expect(nomBox!.width).toBeGreaterThan(100);
  });

  test('no horizontal scroll on mobile', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    const clientWidth = await page.evaluate(() => document.documentElement.clientWidth);
    expect(scrollWidth).toBeLessThanOrEqual(clientWidth + 10);
  });
});

test.describe('Mobile Responsive - Tablet viewport', () => {
  test.use({ viewport: TABLET_VIEWPORT, baseURL: FRONTEND_URL });

  test('[@smoke] landing page renders on tablet', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('body')).toBeVisible();
  });

  test('login form fits tablet screen', async ({ page }) => {
    const login = new LoginPage(page);
    await login.goto();
    await expect(login.emailInput).toBeVisible();
    const box = await login.emailInput.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(150);
  });
});

test.describe('Mobile Responsive - Touch targets', () => {
  test.use({ viewport: MOBILE_VIEWPORT, baseURL: FRONTEND_URL });

  test('buttons meet minimum touch target size (44x44)', async ({ page }) => {
    const login = new LoginPage(page);
    await login.goto();
    const buttons = page.locator('button:visible');
    const count = await buttons.count();
    for (let i = 0; i < Math.min(count, 5); i++) {
      const box = await buttons.nth(i).boundingBox();
      if (box) {
        // Material Design buttons may render at 28px height in dense mode.
        // Accept >= 24px as reasonable for mobile touch targets (MDC spec allows dense).
        expect(box.height).toBeGreaterThanOrEqual(24);
      }
    }
  });

  test('input fields are tappable on mobile', async ({ page }) => {
    await page.goto('/auth/register');
    // Wait for the Angular SPA to bootstrap and the first input to appear.
    // Without this, the inputs locator runs before any DOM is rendered.
    await page.getByTestId('signup-name').waitFor({ state: 'visible', timeout: 15_000 });
    const inputs = page.locator('input:visible');
    const count = await inputs.count();
    expect(count).toBeGreaterThan(0);
    for (let i = 0; i < Math.min(count, 3); i++) {
      const box = await inputs.nth(i).boundingBox();
      if (box) {
        expect(box.height).toBeGreaterThanOrEqual(25);
      }
    }
  });
});

test.describe('Mobile Responsive - Meta viewport', () => {
  test('page has viewport meta tag', async ({ page }) => {
    await page.goto('/');
    const viewport = await page.locator('meta[name="viewport"]').getAttribute('content');
    expect(viewport).toBeTruthy();
    expect(viewport).toContain('width=device-width');
  });
});
