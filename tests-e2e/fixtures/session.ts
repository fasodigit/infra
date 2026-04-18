import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';
import type { Actor } from './actors';
import { SignupPage } from '../page-objects/SignupPage';

export interface StoredSession {
  actor: Actor;
  cookies?: unknown[];
  storageState?: string;
  createdAt: number;
}

export const actorStore: Map<string, StoredSession> = new Map();

/**
 * Login via `/auth/login` et attend la redirection vers `/dashboard/*`.
 * Utilise les sélecteurs `formcontrolname` du LoginComponent.
 */
export async function loginAs(page: Page, actor: Actor): Promise<void> {
  await page.goto('/auth/login');
  await page.locator('input[formcontrolname="email"]').fill(actor.email);
  await page.locator('input[formcontrolname="password"]').fill(actor.password);
  await page.locator('button[type="submit"]').click();
  await page.waitForURL((url) => url.pathname.startsWith('/dashboard'), { timeout: 15_000 });
  actorStore.set(actor.id, { actor, createdAt: Date.now() });
}

/**
 * Inscrit un nouvel acteur via le stepper complet du RegisterComponent
 * puis attend la redirection vers `/dashboard/*`. Renvoie une fois
 * l'inscription validée.
 */
export async function signupAs(page: Page, actor: Actor): Promise<void> {
  const signup = new SignupPage(page);
  await signup.goto();
  await signup.completeRegistration(actor);
  await page.waitForURL((url) => url.pathname.startsWith('/dashboard'), { timeout: 30_000 });
  actorStore.set(actor.id, { actor, createdAt: Date.now() });
}

export async function logout(page: Page): Promise<void> {
  await page.goto('/auth/logout').catch(() => undefined);
}

export function clearSessions(): void {
  actorStore.clear();
}

/**
 * Vérifie que la session BFF côté serveur est valide (workaround du bug
 * `/api/auth/session` 401 — si la session check échoue, on ré-login).
 */
export async function ensureAuthenticated(page: Page, actor: Actor): Promise<void> {
  const url = page.url();
  if (!/\/dashboard\//.test(url)) {
    await loginAs(page, actor);
  }
}
