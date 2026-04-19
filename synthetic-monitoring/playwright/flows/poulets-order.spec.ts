// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Synthetic flow: plateforme Poulets.
// browse catalog → add to cart → checkout (synthetic payment mode).
// Target frontend port 4801, BFF port 4800, poulets-api port 8901.

import { test, expect } from '@playwright/test';
import { syntheticUser, FLOWS } from '../fixtures/synthetic-user';
import { collectPerfMetrics, measureStep, trackHttp5xx } from '../helpers/timing';
import {
  pushSyntheticReport,
  type StepTiming,
} from '../helpers/prometheus-push';

test.describe('poulets order flow', () => {
  test('browse → add-to-cart → checkout', async ({ page }) => {
    const user = syntheticUser();
    const http5xx = trackHttp5xx(page);
    const steps: StepTiming[] = [];
    const started = Date.now();
    let success = false;

    try {
      steps.push(
        await measureStep('goto_catalog', async () => {
          await page.goto('/poulets');
          await expect(
            page.getByRole('heading', { name: /catalogue|poulets/i }),
          ).toBeVisible();
        }),
      );

      steps.push(
        await measureStep('login', async () => {
          await page.getByRole('button', { name: /connexion|login/i }).click();
          await page.getByLabel(/email|courriel/i).fill(user.email);
          await page.getByLabel(/mot de passe|password/i).fill(user.password);
          await page.getByRole('button', { name: /se connecter|sign in/i }).click();
          await expect(page.getByTestId('user-badge')).toBeVisible({
            timeout: 10_000,
          });
        }),
      );

      steps.push(
        await measureStep('add_to_cart', async () => {
          const firstCard = page.getByTestId('poulet-card').first();
          await firstCard.getByRole('button', { name: /ajouter|add/i }).click();
          await expect(page.getByTestId('cart-count')).toHaveText(/[1-9]/);
        }),
      );

      steps.push(
        await measureStep('checkout', async () => {
          await page.getByRole('link', { name: /panier|cart/i }).click();
          await page
            .getByRole('button', { name: /commander|checkout/i })
            .click();
          // synthetic payment — a marker param tells BFF to skip real PSP.
          await page.getByLabel(/mode de paiement/i).selectOption('synthetic');
          await page
            .getByRole('button', { name: /confirmer|confirm/i })
            .click();
          await expect(
            page.getByText(/commande confirmée|order confirmed/i),
          ).toBeVisible({ timeout: 15_000 });
        }),
      );

      success = true;
    } finally {
      const totalDurationMs = Date.now() - started;
      const perf = await collectPerfMetrics(page, http5xx.count()).catch(() => undefined);
      await pushSyntheticReport({
        flow: FLOWS.POULETS_ORDER,
        success,
        totalDurationMs,
        steps,
        perf,
      });
    }
  });
});
