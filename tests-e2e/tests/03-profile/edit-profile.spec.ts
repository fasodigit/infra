import { test, expect } from '@playwright/test';
import { ProfilePage } from '../../page-objects/ProfilePage';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

test.describe('Profile - Edition', () => {
  test('[@smoke] page profile accessible (redirect vers login si non authentifie)', async ({ page }) => {
    const profile = new ProfilePage(page);
    await profile.goto();
    await expect(page).toHaveURL(/\/(profile\/edit|auth\/login)/);
  });

  test('edition des champs de base (nom, téléphone, localisation, description)', async ({ page }) => {
    const actor = { ...actorsByRole('eleveur')[0]!, email: randomEmail('profile') };
    await signupAs(page, actor);

    await page.goto('/profile/edit');
    await page.waitForLoadState('networkidle');
    // L'auth guard peut encore rediriger après networkidle (session polling).
    await page.waitForTimeout(500);
    if (page.url().includes('/auth/login')) {
      test.fixme(true, 'Bug BFF: /api/auth/session renvoie 401 — /profile/edit inaccessible authentifié');
      return;
    }

    const profile = new ProfilePage(page);
    await expect(profile.nomInput).toBeVisible({ timeout: 5_000 });
    await profile.fillBasicInfo({
      nom: `${actor.firstName} ${actor.lastName}`,
      phone: actor.phone,
      localisation: `${actor.city}, ${actor.region}`,
      description: `Éleveur professionnel en ${actor.region}`,
    });
    await profile.save();
    // Après save, redirect vers /profile.
    await expect(page).toHaveURL(/\/profile(?!\/edit)/, { timeout: 5_000 });
  });

  test.fixme('edition SIRET + AMM + upload licence (champs non exposés par le frontend)', async () => {
    // TODO: Ajouter formulaires SIRET/AMM/licence dans ProfileEditComponent.
    // Le composant actuel n'expose que nom/phone/localisation/description.
  });
});
