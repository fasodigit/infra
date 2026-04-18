// Helpers de scénario pour tests de charge Phase 3.
import { type Page, expect } from '@playwright/test';
import type { Actor } from './actors';
import { SignupPage } from '../page-objects/SignupPage';

/**
 * Signup rapide via UI Angular 4-steps → redirect /dashboard.
 */
export async function quickSignup(page: Page, actor: Actor): Promise<void> {
  const signup = new SignupPage(page);
  await signup.goto();
  await signup.completeRegistration(actor);
  await expect(page).toHaveURL(/\/dashboard\/(eleveur|client|producteur|admin)/, {
    timeout: 15_000,
  });
}

/**
 * "Transaction" post-signup minimaliste pour Phase 3 :
 * on frappe le frontend public (pas le shell authentifié qui est cassé
 * par le bug BFF /api/auth/session 401). Ça suffit à mesurer la latence
 * réseau/render et générer du trafic concurrent sur le stack.
 */
export async function postRandomDemand(page: Page, actor: Actor): Promise<void> {
  void actor;
  const routes = ['/', '/auth/login', '/auth/register'];
  const target = routes[Math.floor(Math.random() * routes.length)];
  await page.goto(target, { waitUntil: 'domcontentloaded' });
}
