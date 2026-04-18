import { test, expect } from '@playwright/test';
import { MailpitClient } from '../../fixtures/mailpit';
import { actorsByRole } from '../../fixtures/actors';
import { SignupPage } from '../../page-objects/SignupPage';

const eleveurs = actorsByRole('eleveur');
const mailpit = new MailpitClient();

test.describe('Signup - Eleveurs', () => {
  test('[@smoke] page signup accessible pour eleveurs', async ({ page }) => {
    const signup = new SignupPage(page);
    await signup.goto();
    await expect(page).toHaveURL(/\/auth\/register/);
    await expect(signup.nomInput).toBeVisible();
    await expect(signup.emailInput).toBeVisible();
  });

  for (const actor of eleveurs) {
    test(`inscription eleveur ${actor.id} (${actor.email})`, async ({ page }) => {
      test.info().annotations.push({ type: 'actor', description: actor.id });

      const signup = new SignupPage(page);
      await signup.goto();
      await signup.completeRegistration(actor);

      // Succès : redirect vers le dashboard éleveur.
      await expect(page).toHaveURL(/\/dashboard\/(eleveur|client|producteur|admin)/, {
        timeout: 15_000,
      });

      // Mailpit doit avoir reçu le mail de vérification Kratos.
      // On tolère un délai : l'email arrive en général en < 2 s.
      await expect
        .poll(async () => mailpit.countForEmail(actor.email), { timeout: 10_000 })
        .toBeGreaterThan(0);
    });
  }
});
