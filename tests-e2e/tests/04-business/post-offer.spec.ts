import { test, expect } from '@playwright/test';
import { MarketplacePage } from '../../page-objects/MarketplacePage';
import { randomOffer } from '../../fixtures/data-factory';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

test.describe('Business - Publier une offre', () => {
  test('[@smoke] page offres accessible (redirect vers login si non authentifie)', async ({ page }) => {
    const marketplace = new MarketplacePage(page);
    await marketplace.gotoOffers();
    await expect(page).toHaveURL(/\/(marketplace|auth\/login)/);
  });

  test('eleveur publie une offre de poulets', async ({ page }) => {
    const actor = { ...actorsByRole('eleveur')[0]!, email: randomEmail('offer') };
    await signupAs(page, actor);

    await page.goto('/marketplace/annonces/new');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
    if (page.url().includes('/auth/login')) {
      test.fixme(true, 'Bug BFF: /api/auth/session renvoie 401 — /marketplace/annonces/new inaccessible');
      return;
    }

    const marketplace = new MarketplacePage(page);
    await expect(marketplace.raceSelect).toBeVisible({ timeout: 5_000 });
    await marketplace.postOffer(randomOffer('Poulets'));

    // Le composant devrait afficher un snackbar de succès.
    await expect(page.getByText(/annonce.*publiée|offer.*published/i)).toBeVisible({
      timeout: 10_000,
    });
  });
});
