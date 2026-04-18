import type { Page, Locator } from '@playwright/test';

export class LoginPage {
  readonly page: Page;
  readonly heading: Locator;
  readonly emailInput: Locator;
  readonly passwordInput: Locator;
  readonly submitButton: Locator;
  readonly forgotPasswordLink: Locator;
  readonly registerLink: Locator;
  readonly errorAlert: Locator;

  constructor(page: Page) {
    this.page = page;
    this.heading = page.getByRole('heading', { name: /connexion/i });
    this.emailInput = page.locator('input[formcontrolname="email"]');
    this.passwordInput = page.locator('input[formcontrolname="password"]');
    this.submitButton = page.locator('button[type="submit"]');
    this.forgotPasswordLink = page.getByRole('link', { name: /mot de passe oubli/i });
    this.registerLink = page.getByRole('link', { name: /créer.*compte/i });
    this.errorAlert = page.locator('.error[role="alert"]');
  }

  async goto(): Promise<void> {
    await this.page.goto('/auth/login');
    await this.emailInput.waitFor({ state: 'visible', timeout: 10_000 });
  }

  async loginWith(email: string, password: string): Promise<void> {
    await this.emailInput.fill(email);
    await this.passwordInput.fill(password);
    await this.submitButton.click();
  }
}
