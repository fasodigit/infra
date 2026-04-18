import type { Page, Locator } from '@playwright/test';
import type { OfferDraft, DemandDraft } from '../fixtures/data-factory';

/**
 * Marketplace pages.
 *   - `/marketplace/annonces` (AnnoncesListComponent)
 *   - `/marketplace/annonces/new` (CreateAnnonceComponent — offre éleveur)
 *   - `/marketplace/besoins` (BesoinsListComponent)
 *   - `/marketplace/besoins/new` (CreateBesoinComponent — demande client)
 *
 * Les formulaires utilisent Angular Material avec `formControlName`.
 */
export class MarketplacePage {
  readonly page: Page;
  readonly heading: Locator;

  // Create annonce (offre) fields
  readonly raceSelect: Locator;
  readonly quantityInput: Locator;
  readonly currentWeightInput: Locator;
  readonly estimatedWeightInput: Locator;
  readonly targetDateInput: Locator;
  readonly pricePerKgInput: Locator;
  readonly pricePerUnitInput: Locator;
  readonly locationInput: Locator;
  readonly availabilityStartInput: Locator;
  readonly availabilityEndInput: Locator;
  readonly descriptionInput: Locator;
  readonly ficheSanitaireInput: Locator;
  readonly halalCheckbox: Locator;

  // Create besoin (demande) fields
  readonly racesMultiSelect: Locator;
  readonly minimumWeightInput: Locator;
  readonly deliveryDateInput: Locator;
  readonly maxBudgetPerKgInput: Locator;
  readonly frequencySelect: Locator;

  readonly submitButton: Locator;
  readonly cancelButton: Locator;

  constructor(page: Page) {
    this.page = page;
    this.heading = page.getByRole('heading', { name: /marketplace|annonces|besoins|créer|nouvelle/i });

    // Annonce (offre)
    this.raceSelect = page.locator('mat-select[formcontrolname="race"]');
    this.quantityInput = page.locator('input[formcontrolname="quantity"]');
    this.currentWeightInput = page.locator('input[formcontrolname="currentWeight"]');
    this.estimatedWeightInput = page.locator('input[formcontrolname="estimatedWeight"]');
    this.targetDateInput = page.locator('input[formcontrolname="targetDate"]');
    this.pricePerKgInput = page.locator('input[formcontrolname="pricePerKg"]');
    this.pricePerUnitInput = page.locator('input[formcontrolname="pricePerUnit"]');
    this.locationInput = page.locator('input[formcontrolname="location"]');
    this.availabilityStartInput = page.locator('input[formcontrolname="availabilityStart"]');
    this.availabilityEndInput = page.locator('input[formcontrolname="availabilityEnd"]');
    this.descriptionInput = page.locator('textarea[formcontrolname="description"]');
    this.ficheSanitaireInput = page.locator('input[formcontrolname="ficheSanitaireId"]');
    this.halalCheckbox = page.locator('mat-checkbox[formcontrolname="halalCertified"]');

    // Besoin (demande)
    this.racesMultiSelect = page.locator('mat-select[formcontrolname="races"]');
    this.minimumWeightInput = page.locator('input[formcontrolname="minimumWeight"]');
    this.deliveryDateInput = page.locator('input[formcontrolname="deliveryDate"]');
    this.maxBudgetPerKgInput = page.locator('input[formcontrolname="maxBudgetPerKg"]');
    this.frequencySelect = page.locator('mat-select[formcontrolname="frequency"]');

    this.submitButton = page.locator('button[type="submit"]');
    this.cancelButton = page.getByRole('button', { name: /annuler|cancel/i });
  }

  async gotoOffers(): Promise<void> {
    await this.page.goto('/marketplace/annonces');
  }

  async gotoNewOffer(): Promise<void> {
    await this.page.goto('/marketplace/annonces/new');
  }

  async gotoDemands(): Promise<void> {
    await this.page.goto('/marketplace/besoins');
  }

  async gotoNewDemand(): Promise<void> {
    await this.page.goto('/marketplace/besoins/new');
  }

  /** Remplit un formulaire d'annonce (offre éleveur). */
  async fillOfferForm(offer: OfferDraft): Promise<void> {
    // Race mat-select
    await this.raceSelect.click();
    await this.page.locator('mat-option').first().click();

    await this.quantityInput.fill(String(offer.quantity));
    await this.currentWeightInput.fill('1.5');
    await this.estimatedWeightInput.fill('2.0');
    // Dates: set via JS pour skip datepicker
    const futureDate = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);
    await this.targetDateInput.fill(futureDate).catch(() => undefined);
    await this.pricePerKgInput.fill(String(Math.round(offer.priceXof / 2)));
    await this.pricePerUnitInput.fill(String(offer.priceXof));
    await this.locationInput.fill('Ouagadougou');
    await this.availabilityStartInput.fill(new Date().toISOString().slice(0, 10)).catch(() => undefined);
    await this.availabilityEndInput.fill(futureDate).catch(() => undefined);
    await this.descriptionInput.fill(offer.description);
    await this.ficheSanitaireInput.fill('FS-BF-001');
  }

  async postOffer(offer: OfferDraft): Promise<void> {
    await this.fillOfferForm(offer);
    await this.submitButton.click();
  }

  /** Remplit un formulaire de besoin (demande client). */
  async fillDemandForm(demand: DemandDraft): Promise<void> {
    // Races multi-select
    await this.racesMultiSelect.click();
    await this.page.locator('mat-option').first().click();
    // Fermer le panneau avec Escape
    await this.page.keyboard.press('Escape');

    await this.quantityInput.fill(String(demand.quantity));
    await this.minimumWeightInput.fill('1.5');
    const futureDate = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);
    await this.deliveryDateInput.fill(futureDate).catch(() => undefined);
    await this.maxBudgetPerKgInput.fill(String(demand.maxPriceXof));
    await this.locationInput.fill(demand.location);
    await this.frequencySelect.click();
    await this.page.locator('mat-option').first().click();
  }

  async postDemand(demand: DemandDraft): Promise<void> {
    await this.fillDemandForm(demand);
    await this.submitButton.click();
  }
}
