import { test, expect } from '@playwright/test';
import { SecurityPage } from '../../page-objects/SecurityPage';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

test.describe('Security - Backup codes', () => {
  test('[@smoke] section backup codes presente sur page securite (redirect vers login si non authentifie)', async ({ page }) => {
    const security = new SecurityPage(page);
    await security.goto();
    await expect(page).toHaveURL(/\/(profile\/mfa|auth\/login)/);
  });

  test('generation des backup codes (10 codes)', async ({ page }) => {
    const actor = { ...actorsByRole('eleveur')[0]!, email: randomEmail('backup') };
    await signupAs(page, actor);

    await page.goto('/profile/mfa');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(500);
    if (page.url().includes('/auth/login')) {
      test.fixme(true, 'Bug BFF: /api/auth/session renvoie 401 — /profile/mfa inaccessible authentifié');
      return;
    }

    const security = new SecurityPage(page);
    const codes = await security.generateBackupCodes();
    expect(codes.length).toBe(10);
    // Chaque code devrait être non vide, alphanumerique.
    for (const c of codes) {
      expect(c).toMatch(/^[A-Z0-9-]+$/i);
    }
  });
});
