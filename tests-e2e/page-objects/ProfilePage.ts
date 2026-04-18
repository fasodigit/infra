import type { Page, Locator } from '@playwright/test';

/**
 * Route `/profile/edit` (ProfileEditComponent).
 *
 * Champs réellement exposés: nom, phone, localisation, description.
 * Les champs SIRET / AMM / licence / ville / région attendus par le test
 * initial n'existent PAS dans ce composant — les tests associés sont
 * marqués `test.fixme()` avec TODO.
 */
export class ProfilePage {
  readonly page: Page;
  readonly heading: Locator;
  readonly nomInput: Locator;
  readonly phoneInput: Locator;
  readonly localisationInput: Locator;
  readonly descriptionInput: Locator;
  readonly avatarUpload: Locator;
  readonly saveButton: Locator;
  readonly cancelButton: Locator;

  // Placeholders pour champs attendus mais absents — gardés pour compat API.
  readonly firstNameInput: Locator;
  readonly lastNameInput: Locator;
  readonly siretInput: Locator;
  readonly ammInput: Locator;
  readonly licenceInput: Locator;
  readonly licenceUpload: Locator;
  readonly cityInput: Locator;
  readonly regionInput: Locator;

  constructor(page: Page) {
    this.page = page;
    this.heading = page.getByRole('heading', { name: /profil|profile/i });
    this.nomInput = page.locator('input[formcontrolname="nom"]');
    this.phoneInput = page.locator('input[formcontrolname="phone"]');
    this.localisationInput = page.locator('input[formcontrolname="localisation"]');
    this.descriptionInput = page.locator('textarea[formcontrolname="description"]');
    this.avatarUpload = page.locator('input[type="file"][accept*="image"]');
    this.saveButton = page.locator('button[type="submit"]');
    this.cancelButton = page.getByRole('button', { name: /annuler|cancel/i });

    // Champs absents — resteront never-visible.
    this.firstNameInput = page.locator('input[formcontrolname="firstName"]');
    this.lastNameInput = page.locator('input[formcontrolname="lastName"]');
    this.siretInput = page.locator('input[formcontrolname="siret"]');
    this.ammInput = page.locator('input[formcontrolname="amm"]');
    this.licenceInput = page.locator('input[formcontrolname="licence"]');
    this.licenceUpload = page.locator('input[type="file"][name*="licence"]');
    this.cityInput = page.locator('input[formcontrolname="ville"]');
    this.regionInput = page.locator('input[formcontrolname="region"]');
  }

  async goto(): Promise<void> {
    await this.page.goto('/profile/edit');
  }

  async fillBasicInfo(opts: { nom?: string; phone?: string; localisation?: string; description?: string }): Promise<void> {
    if (opts.nom) await this.nomInput.fill(opts.nom);
    if (opts.phone) await this.phoneInput.fill(opts.phone);
    if (opts.localisation) await this.localisationInput.fill(opts.localisation);
    if (opts.description) await this.descriptionInput.fill(opts.description);
  }

  async save(): Promise<void> {
    await this.saveButton.click();
  }

  async uploadLicence(filePath: string): Promise<void> {
    await this.licenceUpload.setInputFiles(filePath);
  }

  async uploadAvatar(filePath: string): Promise<void> {
    await this.avatarUpload.setInputFiles(filePath);
  }
}
