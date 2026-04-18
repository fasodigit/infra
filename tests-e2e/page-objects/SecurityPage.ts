import type { Page, Locator } from '@playwright/test';

/**
 * Page MFA / sécurité du compte. Route: `/profile/mfa`
 * (pas `/settings/security` qui n'existe pas dans le frontend).
 *
 * Composant: `MfaSettingsComponent`. Boutons ciblés par texte FR.
 */
export class SecurityPage {
  readonly page: Page;
  readonly heading: Locator;
  readonly addPasskeyButton: Locator;
  readonly configureTotpButton: Locator;
  readonly disableTotpButton: Locator;
  readonly generateBackupCodesButton: Locator;
  readonly regenerateBackupCodesButton: Locator;

  // TOTP dialog
  readonly totpSecretField: Locator;
  readonly totpCodeInput: Locator;
  readonly totpActivateButton: Locator;
  readonly totpCancelButton: Locator;

  // Backup codes dialog
  readonly backupCodesList: Locator;
  readonly backupCodesConfirmButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.heading = page.getByRole('heading', { name: /sécurité/i });
    this.addPasskeyButton = page.getByRole('button', { name: /ajouter.*clé.*sécurité|add.*security.*key/i });
    this.configureTotpButton = page.getByRole('button', { name: /^configurer$/i });
    this.disableTotpButton = page.getByRole('button', { name: /désactiver/i });
    this.generateBackupCodesButton = page.getByRole('button', {
      name: /générer 10 codes/i,
    });
    this.regenerateBackupCodesButton = page.getByRole('button', {
      name: /régénérer.*codes/i,
    });

    this.totpSecretField = page.locator('.totp-dialog code').first();
    this.totpCodeInput = page.locator('.totp-dialog input[type="text"]');
    this.totpActivateButton = page.getByRole('button', { name: /activer totp/i });
    this.totpCancelButton = page.getByRole('button', { name: /^annuler$/i });

    this.backupCodesList = page.locator('.codes-dialog code');
    this.backupCodesConfirmButton = page.getByRole('button', {
      name: /j'ai conservé mes codes/i,
    });
  }

  async goto(): Promise<void> {
    await this.page.goto('/profile/mfa');
  }

  async openTotpDialog(): Promise<void> {
    await this.configureTotpButton.first().click();
    await this.totpCodeInput.waitFor({ state: 'visible', timeout: 5_000 });
  }

  async readTotpSecret(): Promise<string> {
    await this.totpSecretField.waitFor({ state: 'visible', timeout: 5_000 });
    const raw = (await this.totpSecretField.textContent()) ?? '';
    return raw.replace(/\s+/g, '').toUpperCase();
  }

  async submitTotpCode(code: string): Promise<void> {
    await this.totpCodeInput.fill(code);
    await this.totpActivateButton.click();
  }

  async registerPasskey(): Promise<void> {
    await this.addPasskeyButton.click();
  }

  async generateBackupCodes(): Promise<string[]> {
    await this.generateBackupCodesButton.first().click();
    await this.backupCodesList.first().waitFor({ state: 'visible', timeout: 5_000 });
    const codes = await this.backupCodesList.allTextContents();
    return codes.map((c) => c.trim()).filter(Boolean);
  }
}
