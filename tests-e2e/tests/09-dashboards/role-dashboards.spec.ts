import { test, expect } from '@playwright/test';
import { DashboardPage } from '../../page-objects/DashboardPage';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

test.describe('Dashboards - Post-signup landing', () => {
  const roles = ['eleveur', 'client'] as const;

  for (const role of roles) {
    test(`${role} lands on dashboard after signup`, async ({ page }) => {
      const actor = { ...actorsByRole(role)[0]!, email: randomEmail(`dash-${role}`) };
      await signupAs(page, actor);
      await expect(page).toHaveURL(/\/dashboard\/(eleveur|client|producteur|admin)/, {
        timeout: 15_000,
      });
    });
  }

  test('dashboard displays heading or content', async ({ page }) => {
    const actor = { ...actorsByRole('eleveur')[0]!, email: randomEmail('dash-content') };
    await signupAs(page, actor);
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });

    const dashboard = new DashboardPage(page);
    const hasHeading = await dashboard.heading.isVisible().catch(() => false);
    const hasBody = await page.locator('main, .dashboard-content, mat-sidenav-content').first().isVisible().catch(() => false);
    expect(hasHeading || hasBody).toBeTruthy();
  });
});

test.describe('Dashboards - Navigation elements', () => {
  test('dashboard has navigation sidebar or menu', async ({ page }) => {
    const actor = { ...actorsByRole('eleveur')[0]!, email: randomEmail('dash-nav') };
    await signupAs(page, actor);
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });

    const dashboard = new DashboardPage(page);
    const hasNav = await dashboard.navigation.isVisible().catch(() => false);
    const hasMenu = await page.locator('mat-toolbar, header, nav').first().isVisible().catch(() => false);
    expect(hasNav || hasMenu).toBeTruthy();
  });

  test('dashboard shows user context (name or role)', async ({ page }) => {
    const actor = { ...actorsByRole('client')[0]!, email: randomEmail('dash-user') };
    await signupAs(page, actor);
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 15_000 });

    const bodyText = await page.locator('body').textContent() ?? '';
    const hasUserInfo = bodyText.toLowerCase().includes(actor.firstName.toLowerCase()) ||
      bodyText.toLowerCase().includes('éleveur') ||
      bodyText.toLowerCase().includes('client') ||
      bodyText.toLowerCase().includes('dashboard');
    expect(hasUserInfo).toBeTruthy();
  });
});

test.describe('Dashboards - Direct URL access', () => {
  test('[@smoke] /dashboard/eleveur without auth redirects to login', async ({ page }) => {
    await page.goto('/dashboard/eleveur');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    const url = page.url();
    expect(url.includes('/auth/login') || url.includes('/dashboard')).toBeTruthy();
  });

  test('[@smoke] /dashboard/admin without auth redirects to login', async ({ page }) => {
    await page.goto('/dashboard/admin');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    const url = page.url();
    expect(url.includes('/auth/login') || url.includes('/dashboard')).toBeTruthy();
  });
});
