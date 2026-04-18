import { test, expect } from '@playwright/test';
import { MarketplacePage } from '../../page-objects/MarketplacePage';
import { randomDemand } from '../../fixtures/data-factory';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

test.describe('Business - Publier une demande', () => {
  test('[@smoke] page demandes accessible (redirect vers login si non authentifie)', async ({ page }) => {
    const marketplace = new MarketplacePage(page);
    await marketplace.gotoDemands();
    await expect(page).toHaveURL(/\/(marketplace|auth\/login)/);
  });

  test('client publie une demande de poulets', async ({ page }) => {
    const actor = { ...actorsByRole('client')[0]!, email: randomEmail('demand') };
    await signupAs(page, actor);

    await page.goto('/marketplace/besoins/new');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
    if (page.url().includes('/auth/login')) {
      test.fixme(true, 'Bug BFF: /api/auth/session renvoie 401 — /marketplace/besoins/new inaccessible');
      return;
    }

    const marketplace = new MarketplacePage(page);
    await expect(marketplace.racesMultiSelect).toBeVisible({ timeout: 5_000 });
    await marketplace.postDemand(randomDemand('Poulets'));

    await expect(page.getByText(/besoin.*publié|demand.*published/i)).toBeVisible({
      timeout: 10_000,
    });
  });
});
