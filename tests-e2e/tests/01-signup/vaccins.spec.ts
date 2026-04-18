import { test, expect } from '@playwright/test';
import { MailpitClient } from '../../fixtures/mailpit';
import { actorsByRole } from '../../fixtures/actors';
import { SignupPage } from '../../page-objects/SignupPage';

/**
 * Fournisseurs de vaccins : mappés sur `producteur_aliment` dans le UI.
 * AMM et licence non exposés au signup.
 */
const vaccins = actorsByRole('vaccins');
const mailpit = new MailpitClient();

test.describe('Signup - Fournisseurs vaccins', () => {
  test('[@smoke] page signup accessible pour fournisseurs vaccins', async ({ page }) => {
    const signup = new SignupPage(page);
    await signup.goto();
    await expect(page).toHaveURL(/\/auth\/register/);
  });

  for (const actor of vaccins) {
    test(`inscription fournisseur vaccins ${actor.id} (AMM ${actor.amm ?? 'n/a'})`, async ({ page }) => {
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
