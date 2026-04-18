import { test, expect } from '@playwright/test';
import { MailpitClient } from '../../fixtures/mailpit';
import { actorsByRole } from '../../fixtures/actors';
import { SignupPage } from '../../page-objects/SignupPage';

/**
 * Fournisseurs d'aliments : mappés sur `producteur_aliment` dans le UI.
 * Le SIRET n'est pas saisi au signup — à entrer dans profile/edit.
 */
const aliments = actorsByRole('aliments');
const mailpit = new MailpitClient();

test.describe('Signup - Fournisseurs aliments', () => {
  test('[@smoke] page signup accessible pour fournisseurs aliments', async ({ page }) => {
    const signup = new SignupPage(page);
    await signup.goto();
    await expect(page).toHaveURL(/\/auth\/register/);
  });

  for (const actor of aliments) {
    test(`inscription fournisseur aliments ${actor.id} (SIRET ${actor.siret ?? 'n/a'})`, async ({ page }) => {
      test.info().annotations.push({ type: 'actor', description: actor.id });

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
