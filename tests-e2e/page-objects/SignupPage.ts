import type { Page, Locator } from '@playwright/test';
import type { Actor, ActorRole } from '../fixtures/actors';

/**
 * Angular Material stepper à 4 étapes.
 * Les champs sont ciblés via `formcontrolname` (stable, indépendant i18n).
 * Les boutons « Continuer » existent dans chaque étape : utiliser
 * le filtre Playwright `:visible` qui isole le bouton de l'étape active.
 */
export class SignupPage {
  readonly page: Page;
  readonly nomInput: Locator;
  readonly emailInput: Locator;
  readonly phoneInput: Locator;
  readonly passwordInput: Locator;
  readonly confirmPasswordInput: Locator;
  readonly localisationInput: Locator;
  readonly capaciteInput: Locator;
  readonly clientTypeSelect: Locator;
  readonly zoneDistributionInput: Locator;
  readonly groupementNomInput: Locator;
  readonly submitButton: Locator;
  readonly errorAlert: Locator;

  constructor(page: Page) {
    this.page = page;
    // Use data-testid (stable across UI refactors). Each input also keeps its
    // formcontrolname for accessibility / Angular Reactive Forms wiring.
    this.nomInput = page.getByTestId('signup-name');
    this.emailInput = page.getByTestId('signup-email');
    this.phoneInput = page.getByTestId('signup-phone');
    this.passwordInput = page.getByTestId('signup-password');
    this.confirmPasswordInput = page.getByTestId('signup-password-confirm');
    this.localisationInput = page.getByTestId('signup-localisation');
    this.capaciteInput = page.getByTestId('signup-capacite');
    this.clientTypeSelect = page.getByTestId('signup-client-type');
    this.zoneDistributionInput = page.getByTestId('signup-zone-distribution');
    this.groupementNomInput = page.getByTestId('signup-groupement-nom');
    this.submitButton = page.getByTestId('signup-submit');
    this.errorAlert = page.getByTestId('signup-error');
  }

  async goto(): Promise<void> {
    await this.page.goto('/auth/register', { waitUntil: 'domcontentloaded' });
    // Angular bootstrap + lazy route peut prendre plusieurs secondes sous charge.
    await this.nomInput.first().waitFor({ state: 'visible', timeout: 45_000 });
  }

  /** Étape 1 : informations de compte. */
  async fillAccount(actor: Actor): Promise<void> {
    const fullName = `${actor.firstName} ${actor.lastName}`.trim();
    await this.nomInput.fill(fullName);
    await this.emailInput.fill(actor.email);
    if (actor.phone) {
      await this.phoneInput.fill(actor.phone);
    }
    await this.passwordInput.fill(actor.password);
    await this.confirmPasswordInput.fill(actor.password);
  }

  /**
   * Avance à l'étape suivante en cliquant sur « Continuer ».
   * Plusieurs boutons portent data-testid="signup-stepper-next" (un par step) ;
   * `:visible` + `.first()` cible celui de l'étape active.
   */
  async next(): Promise<void> {
    const btn = this.page.getByTestId('signup-stepper-next').locator('visible=true').first();
    await btn.waitFor({ state: 'visible', timeout: 5_000 });
    await btn.click();
    // Laisser l'animation du stepper se terminer (CSS transition ~300ms).
    await this.page.waitForTimeout(450);
  }

  /**
   * Étape 2 : sélectionne le rôle. Le frontend n'expose que 3 rôles :
   * eleveur / client / producteur_aliment. Les rôles fixture
   * pharmacie/vaccins/aliments sont regroupés sur producteur_aliment.
   */
  async selectRole(role: ActorRole): Promise<void> {
    const uiRole = mapRoleToUi(role);
    const radio = this.page.locator(`input[formcontrolname="role"][value="${uiRole}"]`);
    await radio.check({ force: true });
  }

  /** Étape 3 : détails selon le rôle. */
  async fillDetails(actor: Actor): Promise<void> {
    // Toutes étapes ont la localisation.
    await this.localisationInput.fill(`${actor.city}, ${actor.region}`);
    const uiRole = mapRoleToUi(actor.role);
    if (uiRole === 'eleveur') {
      // capacité optionnelle
      await this.capaciteInput.fill('500').catch(() => undefined);
    } else if (uiRole === 'client') {
      // Select natif HTML — pas mat-select — dans ce cas.
      await this.clientTypeSelect.selectOption('PARTICULIER').catch(() => undefined);
    } else if (uiRole === 'producteur_aliment' && this.zoneDistributionInput) {
      await this.zoneDistributionInput.fill(actor.region).catch(() => undefined);
    }
  }

  /** Étape 4 : groupement (facultatif) + submit. */
  async fillGroupementAndSubmit(groupementNom?: string): Promise<void> {
    if (groupementNom) {
      await this.groupementNomInput.fill(groupementNom).catch(() => undefined);
    }
    await this.submitButton.click();
  }

  /** Flow complet : étapes 1 → 2 → 3 → 4 → submit. */
  async completeRegistration(actor: Actor, opts: { groupement?: string } = {}): Promise<void> {
    await this.fillAccount(actor);
    await this.next();
    await this.selectRole(actor.role);
    await this.next();
    await this.fillDetails(actor);
    await this.next();
    await this.fillGroupementAndSubmit(opts.groupement);

    // Si une erreur serveur apparaît (ex: email dupliqué), lever explicitement.
    await this.page.waitForTimeout(500);
    if (await this.errorAlert.isVisible().catch(() => false)) {
      const msg = (await this.errorAlert.textContent()) ?? 'unknown';
      throw new Error(`Erreur serveur à l'inscription: ${msg.trim()}`);
    }
  }
}

/** Mappe les rôles fixture vers les 3 valeurs exposées par le register UI. */
export function mapRoleToUi(role: ActorRole): 'eleveur' | 'client' | 'producteur_aliment' {
  switch (role) {
    case 'eleveur':
      return 'eleveur';
    case 'client':
      return 'client';
    case 'pharmacie':
    case 'vaccins':
    case 'aliments':
    case 'veterinaire':
    case 'transporteur':
    case 'admin':
      return 'producteur_aliment';
    default:
      return 'client';
  }
}
