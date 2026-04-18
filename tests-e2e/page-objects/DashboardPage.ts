import type { Page, Locator } from '@playwright/test';

/**
 * Dashboard après login. Route: `/dashboard/{eleveur|client|producteur|admin}`
 * avec redirect root `/dashboard` via DashboardRedirectComponent.
 */
export class DashboardPage {
  readonly page: Page;
  readonly heading: Locator;
  readonly kpiCards: Locator;
  readonly navigation: Locator;
  readonly userMenu: Locator;
  readonly logoutButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.heading = page.locator('h1').first();
    this.kpiCards = page.locator('.kpi-card, mat-card.kpi-card');
    this.navigation = page.locator('nav, aside.sidenav, mat-sidenav').first();
    this.userMenu = page.getByRole('button', { name: /compte|profil|menu utilisateur/i });
    this.logoutButton = page.getByRole('button', { name: /déconnexion|logout/i });
  }

  async goto(): Promise<void> {
    await this.page.goto('/dashboard');
  }

  async gotoEleveur(): Promise<void> {
    await this.page.goto('/dashboard/eleveur');
  }

  async gotoClient(): Promise<void> {
    await this.page.goto('/dashboard/client');
  }

  async logout(): Promise<void> {
    if (await this.userMenu.isVisible().catch(() => false)) {
      await this.userMenu.click();
    }
    await this.logoutButton.click();
  }
}
