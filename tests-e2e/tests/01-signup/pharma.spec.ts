import { test, expect } from '@playwright/test';
import { MailpitClient } from '../../fixtures/mailpit';
import { actorsByRole } from '../../fixtures/actors';
import { SignupPage } from '../../page-objects/SignupPage';

/**
 * Pharmacies vétérinaires : le frontend les regroupe dans la carte
 * « Producteur » (role `producteur_aliment`). Le champ AMM n'est pas
 * exposé au signup — il devrait être édité dans le profil ensuite.
 */
const pharmacies = actorsByRole('pharmacie');
const mailpit = new MailpitClient();

test.describe('Signup - Pharmacies veterinaires', () => {
  test('[@smoke] page signup accessible pour pharmacies', async ({ page }) => {
    const signup = new SignupPage(page);
    await signup.goto();
    await expect(page).toHaveURL(/\/auth\/register/);
  });

  for (const actor of pharmacies) {
    test(`inscription pharmacie ${actor.id} avec AMM ${actor.amm ?? 'n/a'}`, async ({ page }) => {
      test.info().annotations.push({ type: 'actor', description: actor.id });
      test.info().annotations.push({
        type: 'note',
        description: 'AMM saisi comme zone de distribution (champ AMM non exposé au signup)',
      });

      const signup = new SignupPage(page);
      await signup.goto();
      await signup.completeRegistration(actor);

      await expect(page).toHaveURL(/\/dashboard\/(eleveur|client|producteur|admin)/, {
        timeout: 15_000,
      });

      await expect
        .poll(async () => mailpit.countForEmail(actor.email), { timeout: 10_000 })
        .toBeGreaterThan(0);
    });
  }
});
