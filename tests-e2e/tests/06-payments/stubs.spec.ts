// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { test, expect } from '@playwright/test';
import { signupAs } from '../../fixtures/session';
import { actorsByRole } from '../../fixtures/actors';
import { randomEmail } from '../../fixtures/data-factory';

// Chaque stub MVP (F4..F10) est une route Angular standalone.
// Les 6 routes protégées nécessitent un login (authGuard) ; /pwa-info est publique.
// On sérialise l'exécution pour partager un seul compte via storageState
// (robustesse + vitesse : 1 signup au lieu de 6).
test.describe.configure({ mode: 'serial' });

const stubs = [
  { path: '/messaging/chat/test',       heading: /chat temps réel/i,         guarded: true  },
  { path: '/pwa-info',                  heading: /mode hors-ligne/i,         guarded: false },
  { path: '/profile/kyc',               heading: /vérification d'identité/i, guarded: true  },
  { path: '/payments/escrow/test',      heading: /paiement sécurisé/i,       guarded: true  },
  { path: '/profile/notifications-push', heading: /notifications push/i,     guarded: true  },
  { path: '/marketplace/near-me',       heading: /à proximité/i,             guarded: true  },
  { path: '/dashboard/analytics',       heading: /analytique vendeur/i,      guarded: true  },
];

let storedState: Awaited<ReturnType<import('@playwright/test').BrowserContext['storageState']>> | undefined;

test.beforeAll(async ({ browser }) => {
  const ctx = await browser.newContext();
  const page = await ctx.newPage();
  const actor = { ...actorsByRole('client')[0]!, email: randomEmail('stub') };
  await signupAs(page, actor);
  storedState = await ctx.storageState();
  await ctx.close();
});

for (const { path, heading, guarded } of stubs) {
  test(`[@smoke] stub ${path} rendu avec heading`, async ({ browser }) => {
    const ctx = guarded && storedState
      ? await browser.newContext({ storageState: storedState })
      : await browser.newContext();
    const page = await ctx.newPage();
    await page.goto(path);
    await expect(page.getByRole('heading', { name: heading }).first())
      .toBeVisible({ timeout: 10_000 });
    await ctx.close();
  });
}
