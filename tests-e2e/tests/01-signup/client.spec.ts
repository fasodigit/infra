import { test, expect } from '@playwright/test';
import { MailpitClient } from '../../fixtures/mailpit';
import { actorsByRole } from '../../fixtures/actors';
import { SignupPage } from '../../page-objects/SignupPage';

const clients = actorsByRole('client');
const mailpit = new MailpitClient();

test.describe('Signup - Clients finaux', () => {
  test('[@smoke] page signup accessible pour clients', async ({ page }) => {
    const signup = new SignupPage(page);
    await signup.goto();
    await expect(page).toHaveURL(/\/auth\/register/);
  });

  for (const actor of clients) {
    test(`inscription client ${actor.id} (${actor.email})`, async ({ page }) => {
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
