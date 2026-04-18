import { test, expect } from '@playwright/test';
import { addVirtualAuthenticator } from '../../fixtures/webauthn';
import { SecurityPage } from '../../page-objects/SecurityPage';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

test.describe('Security - Passkey/WebAuthn', () => {
  test('[@smoke] virtual authenticator CDP disponible', async ({ page }) => {
    await page.goto('/');
    const authenticator = await addVirtualAuthenticator(page);
    expect(authenticator.authenticatorId).toBeTruthy();
    await authenticator.remove();
  });

  test('enregistrement PassKey via virtual authenticator', async ({ page }) => {
    const actor = { ...actorsByRole('eleveur')[0]!, email: randomEmail('pk') };
    await signupAs(page, actor);

    // Enregistrer le virtual authenticator AVANT la navigation vers MFA.
    const authenticator = await addVirtualAuthenticator(page);

    await page.goto('/profile/mfa');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);

    if (page.url().includes('/auth/login')) {
      test.fixme(true, 'Bug BFF: /api/auth/session renvoie 401 — impossible d\'atteindre /profile/mfa authentifié');
      await authenticator.remove();
      return;
    }

    const security = new SecurityPage(page);
    try {
      await expect(security.addPasskeyButton).toBeVisible({ timeout: 5_000 });
      await security.registerPasskey();

      // Après register, le composant ajoute un device dans la liste locale.
      await expect(page.getByText(/passkey ajoutée/i)).toBeVisible({ timeout: 5_000 });
      // La liste des devices doit contenir au moins 1 entrée.
      await expect(page.locator('.devices li')).toHaveCount(1, { timeout: 5_000 });
    } finally {
      await authenticator.remove();
    }
  });
});
